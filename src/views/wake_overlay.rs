// Floating, borderless chat window that pops up when the wake word
// fires. State is ephemeral — closing the window discards the
// conversation, by design.

use crate::ai::{AgentContext, Client};
use crate::theme;
use crate::views::chat::ChatState;

pub struct WakeOverlay {
    pub chat: ChatState,
    pub open: bool,
    pub focused_input: bool,
}

impl Default for WakeOverlay {
    fn default() -> Self {
        Self {
            chat: ChatState::default(),
            open: false,
            focused_input: false,
        }
    }
}

impl WakeOverlay {
    pub fn trigger(&mut self) {
        self.chat = ChatState::default();
        self.open = true;
        self.focused_input = false;
    }
}

pub fn show(
    ctx: &egui::Context,
    state: &mut WakeOverlay,
    runtime: &tokio::runtime::Runtime,
    api_key: Option<&str>,
    model: &str,
    agent: AgentContext,
    wake_word: &str,
) {
    if !state.open {
        return;
    }

    let viewport_id = egui::ViewportId::from_hash_of("wake-overlay");
    let viewport = egui::ViewportBuilder::default()
        .with_title("Jarvis")
        .with_inner_size([520.0, 420.0])
        .with_decorations(false)
        .with_always_on_top()
        .with_resizable(false)
        .with_transparent(false);

    let api_key = api_key.map(|s| s.to_string());
    let model = model.to_string();
    // Make sure all the values the inner closure captures live as long as the call.
    let agent_ref = agent.clone();
    let wake_word = wake_word.to_string();

    let mut close_requested = false;
    ctx.show_viewport_immediate(viewport_id, viewport, |ctx, _class| {
        // Esc dismisses.
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if esc {
            close_requested = true;
        }
        // Drain async chat events into history.
        state.chat.drain_pending();
        state.chat.drain_ui_commands(ctx);
        state.chat.refresh_system_prompt(&agent_ref);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::SURFACE_0)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER_STR))
                    .inner_margin(egui::Margin::same(14.0)),
            )
            .show(ctx, |ui| {
                // Title row
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                    ui.painter().rect(
                        rect,
                        theme::rounding(theme::R_FIELD),
                        theme::ACCENT,
                        egui::Stroke::NONE,
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "J",
                        egui::FontId::proportional(13.0),
                        theme::BLACK,
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!("\"{wake_word}\""))
                            .color(theme::TEXT)
                            .strong()
                            .size(15.0),
                    );
                    ui.label(
                        egui::RichText::new("ephemeral · closing discards this chat")
                            .color(theme::TEXT_MUTED)
                            .small(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::icon_button(ui, "x").clicked() {
                            close_requested = true;
                        }
                    });
                });

                ui.add_space(8.0);

                // Transcript
                let composer_h = 78.0;
                let log_h = (ui.available_height() - composer_h - 12.0).max(120.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .max_height(log_h)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in &state.chat.history {
                            use crate::ai::Role;
                            match msg.role {
                                Role::System => continue,
                                Role::User => {
                                    label_line(ui, "You", theme::TEXT_MUTED);
                                    ui.label(egui::RichText::new(msg.content_str()).color(theme::TEXT));
                                    ui.add_space(6.0);
                                }
                                Role::Assistant => {
                                    if !msg.content_str().is_empty() {
                                        label_line(ui, "Jarvis", theme::ACCENT);
                                        ui.label(egui::RichText::new(msg.content_str()).color(theme::TEXT));
                                        ui.add_space(6.0);
                                    }
                                    if let Some(calls) = &msg.tool_calls {
                                        for c in calls {
                                            theme::badge(ui, &format!("→ {}", c.function.name), false);
                                            ui.add_space(2.0);
                                        }
                                    }
                                }
                                Role::Tool => {
                                    let preview: String = msg.content_str().chars().take(160).collect();
                                    ui.label(
                                        egui::RichText::new(format!("[{}] {preview}", msg.name.as_deref().unwrap_or("tool")))
                                            .color(theme::TEXT_DIM)
                                            .small()
                                            .monospace(),
                                    );
                                }
                            }
                        }
                        if state.chat.awaiting {
                            ui.label(
                                egui::RichText::new("Jarvis is thinking…")
                                    .italics()
                                    .color(theme::TEXT_MUTED),
                            );
                            ctx.request_repaint_after(std::time::Duration::from_millis(200));
                        }
                        if let Some(err) = &state.chat.error {
                            ui.label(
                                egui::RichText::new(format!("Error: {err}"))
                                    .color(egui::Color32::from_rgb(255, 170, 170)),
                            );
                        }
                    });

                ui.add_space(8.0);

                // Composer
                let key_present = api_key.as_deref().map(|k| !k.trim().is_empty()).unwrap_or(false);
                egui::Frame::none()
                    .fill(theme::SURFACE_2)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER))
                    .rounding(theme::rounding(theme::R_FIELD))
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let resp = ui.add_sized(
                                egui::vec2(ui.available_width() - 100.0, 40.0),
                                egui::TextEdit::singleline(&mut state.chat.draft)
                                    .frame(false)
                                    .hint_text("Tell Jarvis…"),
                            );
                            if !state.focused_input {
                                resp.request_focus();
                                state.focused_input = true;
                            }
                            let enter = resp.has_focus()
                                && ctx.input(|i| i.key_pressed(egui::Key::Enter));
                            let mut submit = false;
                            if theme::primary_button(ui, "Send", key_present && !state.chat.awaiting).clicked() {
                                submit = true;
                            }
                            if enter {
                                submit = true;
                            }
                            if submit {
                                if let Some(key) = api_key.as_deref() {
                                    if let Ok(client) = Client::new(key) {
                                        state.chat.submit(
                                            runtime,
                                            ctx.clone(),
                                            client,
                                            model.clone(),
                                            agent_ref.clone(),
                                        );
                                    }
                                }
                            }
                        });
                    });
            });

        if ctx.input(|i| i.viewport().close_requested()) {
            close_requested = true;
        }
    });

    if close_requested {
        state.open = false;
    }
}

fn label_line(ui: &mut egui::Ui, who: &str, color: egui::Color32) {
    ui.label(
        egui::RichText::new(who)
            .color(color)
            .small()
            .strong(),
    );
}
