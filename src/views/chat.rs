// Chat view: speech-bubble transcript, rounded composer, and an agent
// loop that lets Grok call local tools (system info, QR codes, app
// launch, …) before producing a final reply.

use std::sync::mpsc::{Receiver, Sender};

use crate::ai::{
    available_tools, catalog_summary, execute_tool, AgentContext, ChatMessage, ChatRequest,
    Client, Role,
};
use crate::theme;

const AGENT_LOOP_LIMIT: usize = 6;

pub struct ChatState {
    pub history: Vec<ChatMessage>,
    pub draft: String,
    pub awaiting: bool,
    pub error: Option<String>,
    sender: Sender<ChatEvent>,
    receiver: Receiver<ChatEvent>,
}

pub enum ChatEvent {
    /// One assistant message (possibly with tool_calls) appended.
    Assistant(ChatMessage),
    /// One tool result message appended.
    ToolResult(ChatMessage),
    /// Agent loop finished (with or without a final assistant message).
    Done,
    Error(String),
}

impl Default for ChatState {
    fn default() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            history: vec![ChatMessage::system(
                "You are Jarvis, the user's on-device assistant. Be concise.",
            )],
            draft: String::new(),
            awaiting: false,
            error: None,
            sender,
            receiver,
        }
    }
}

impl ChatState {
    /// Replace the system prompt at the top of `history` so it always
    /// reflects the current autonomy / wake-word / tool catalog. Called
    /// every frame so the agent sees fresh state.
    pub fn refresh_system_prompt(&mut self, agent: &AgentContext) {
        let prompt = format!(
            "You are Jarvis, the user's on-device voice-activated desktop assistant.\n\
             The wake word is \"{wake}\".\n\
             You can call the following tools to help the user. \
             Prefer tools over guessing when the user asks about live state.\n\n{tools}\n\n\
             When you have enough information, reply naturally in plain text. \
             Be concise and friendly.",
            wake = agent.wake_word,
            tools = catalog_summary(&agent.autonomy),
        );
        if let Some(first) = self.history.first_mut() {
            if first.role == Role::System {
                first.content = Some(prompt);
                return;
            }
        }
        self.history.insert(0, ChatMessage::system(prompt));
    }

    pub fn drain_pending(&mut self) {
        while let Ok(ev) = self.receiver.try_recv() {
            match ev {
                ChatEvent::Assistant(msg) => self.history.push(msg),
                ChatEvent::ToolResult(msg) => self.history.push(msg),
                ChatEvent::Done => self.awaiting = false,
                ChatEvent::Error(e) => {
                    self.error = Some(e);
                    self.awaiting = false;
                }
            }
        }
    }

    pub fn submit(
        &mut self,
        runtime: &tokio::runtime::Runtime,
        ctx: egui::Context,
        client: Client,
        model: String,
        agent: AgentContext,
    ) {
        let prompt = self.draft.trim().to_string();
        if prompt.is_empty() || self.awaiting {
            return;
        }
        self.history.push(ChatMessage::user(prompt));
        self.draft.clear();
        self.awaiting = true;
        self.error = None;

        let messages = self.history.clone();
        let tx = self.sender.clone();
        let tools = available_tools(&agent.autonomy);

        runtime.spawn(async move {
            let mut working = messages;

            for _ in 0..AGENT_LOOP_LIMIT {
                let req = ChatRequest {
                    model: model.clone(),
                    messages: working.clone(),
                    temperature: Some(0.4),
                    max_tokens: Some(800),
                    tools: if tools.is_empty() { None } else { Some(tools.clone()) },
                    tool_choice: Some("auto".into()),
                };

                let resp = match client.chat(req).await {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(ChatEvent::Error(format!("{e:#}")));
                        ctx.request_repaint();
                        return;
                    }
                };

                let Some(choice) = resp.choices.into_iter().next() else {
                    let _ = tx.send(ChatEvent::Error("empty response".into()));
                    ctx.request_repaint();
                    return;
                };

                let assistant = choice.message.clone();
                working.push(assistant.clone());
                let _ = tx.send(ChatEvent::Assistant(assistant.clone()));
                ctx.request_repaint();

                let Some(tool_calls) = assistant.tool_calls.clone() else {
                    let _ = tx.send(ChatEvent::Done);
                    ctx.request_repaint();
                    return;
                };

                if tool_calls.is_empty() {
                    let _ = tx.send(ChatEvent::Done);
                    ctx.request_repaint();
                    return;
                }

                for call in tool_calls {
                    let args = serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                        .unwrap_or(serde_json::Value::Object(Default::default()));
                    let result = execute_tool(&agent, &call.function.name, args);
                    let payload = serde_json::to_string(&result).unwrap_or_else(|_| "null".into());
                    let tool_msg = ChatMessage::tool_result(&call.id, &call.function.name, payload);
                    working.push(tool_msg.clone());
                    let _ = tx.send(ChatEvent::ToolResult(tool_msg));
                    ctx.request_repaint();
                }
            }

            // Loop hit the safety limit without converging.
            let _ = tx.send(ChatEvent::Error(format!(
                "agent loop hit the {AGENT_LOOP_LIMIT}-call limit without finishing"
            )));
            ctx.request_repaint();
        });
    }
}

pub fn show(
    ui: &mut egui::Ui,
    chat: &mut ChatState,
    runtime: &tokio::runtime::Runtime,
    api_key: Option<&str>,
    model: &str,
    agent: AgentContext,
) {
    chat.refresh_system_prompt(&agent);

    let api_key = api_key.map(|s| s.to_string());
    let key_present = api_key.as_deref().map(|k| !k.trim().is_empty()).unwrap_or(false);

    theme::section_header(
        ui,
        "Chat with Jarvis",
        Some(if key_present {
            "Powered by xAI Grok. Jarvis can call local tools — see Settings, then Autonomy."
        } else {
            "Set your xAI API key in Settings to start chatting."
        }),
    );

    let composer_h = 92.0;
    let log_h = (ui.available_height() - composer_h - 16.0).max(180.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .max_height(log_h)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.add_space(4.0);
            for msg in &chat.history {
                match msg.role {
                    Role::System => continue,
                    Role::User | Role::Assistant => bubble(ui, msg),
                    Role::Tool => tool_chip(ui, msg),
                }
            }
            if chat.awaiting {
                pending_bubble(ui);
            }
            if let Some(err) = &chat.error {
                ui.add_space(6.0);
                theme::subcard(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("Error: {err}"))
                            .color(egui::Color32::from_rgb(255, 170, 170)),
                    );
                });
            }
        });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    composer(ui, chat, runtime, api_key.as_deref(), model, key_present, agent);
}

fn bubble(ui: &mut egui::Ui, msg: &ChatMessage) {
    // Skip empty assistant messages whose only payload was a tool_calls
    // array (we render the tool_calls themselves below as chips).
    let has_text = !msg.content_str().is_empty();
    let has_calls = msg
        .tool_calls
        .as_ref()
        .map(|c| !c.is_empty())
        .unwrap_or(false);
    if !has_text && !has_calls {
        return;
    }

    let is_user = msg.role == Role::User;
    let layout = if is_user {
        egui::Layout::right_to_left(egui::Align::Min)
    } else {
        egui::Layout::left_to_right(egui::Align::Min)
    };

    ui.with_layout(layout, |ui| {
        let max_w = (ui.available_width() * 0.78).max(220.0);
        ui.set_max_width(max_w);

        let (fill, stroke, label) = if is_user {
            (theme::ACCENT_DEEP, theme::ACCENT, "You")
        } else {
            (theme::SURFACE_2, theme::BORDER, "Jarvis")
        };

        egui::Frame::none()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, stroke))
            .rounding(theme::rounding(theme::R_CARD))
            .inner_margin(egui::Margin::symmetric(14.0, 10.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .color(if is_user { theme::ACCENT_HOV } else { theme::TEXT_MUTED })
                            .small()
                            .strong(),
                    );
                    ui.add_space(2.0);
                    if has_text {
                        ui.label(egui::RichText::new(msg.content_str()).color(theme::TEXT));
                    }
                    if let Some(calls) = &msg.tool_calls {
                        for call in calls {
                            ui.add_space(4.0);
                            theme::badge(
                                ui,
                                &format!("calling {}(…)", call.function.name),
                                false,
                            );
                        }
                    }
                });
            });
    });
    ui.add_space(8.0);
}

fn tool_chip(ui: &mut egui::Ui, msg: &ChatMessage) {
    let name = msg.name.as_deref().unwrap_or("tool");
    let body = msg.content_str();
    let preview = if body.len() > 220 {
        format!("{}…", &body[..220])
    } else {
        body.to_string()
    };
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        let max_w = (ui.available_width() * 0.78).max(220.0);
        ui.set_max_width(max_w);
        egui::Frame::none()
            .fill(theme::SURFACE_1)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .rounding(theme::rounding(theme::R_FIELD))
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        theme::badge(ui, &format!("{name} → result"), false);
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(preview)
                            .color(theme::TEXT_MUTED)
                            .monospace()
                            .small(),
                    );
                });
            });
    });
    ui.add_space(8.0);
}

fn pending_bubble(ui: &mut egui::Ui) {
    let dots = {
        let t = ui.ctx().input(|i| i.time);
        let n = ((t * 2.0) as usize) % 4; // 0..=3
        ".".repeat(n)
    };
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));

    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        egui::Frame::none()
            .fill(theme::SURFACE_2)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .rounding(theme::rounding(theme::R_CARD))
            .inner_margin(egui::Margin::symmetric(14.0, 10.0))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!("Jarvis is thinking{dots}"))
                        .italics()
                        .color(theme::TEXT_MUTED),
                );
            });
    });
}

fn composer(
    ui: &mut egui::Ui,
    chat: &mut ChatState,
    runtime: &tokio::runtime::Runtime,
    api_key: Option<&str>,
    model: &str,
    key_present: bool,
    agent: AgentContext,
) {
    let mut should_submit = false;
    egui::Frame::none()
        .fill(theme::SURFACE_2)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .rounding(theme::rounding(theme::R_CARD))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                let resp = ui.add_sized(
                    egui::vec2(ui.available_width() - 124.0, 56.0),
                    egui::TextEdit::multiline(&mut chat.draft)
                        .desired_rows(2)
                        .frame(false)
                        .hint_text(if key_present {
                            "Ask Jarvis anything…  (Enter to send, Shift+Enter for newline)"
                        } else {
                            "Set your xAI API key in Settings to chat."
                        }),
                );
                let enter = resp.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                if theme::primary_button(ui, "Send", key_present && !chat.awaiting).clicked() {
                    should_submit = true;
                }
                if enter {
                    should_submit = true;
                }
            });
        });

    if should_submit {
        if let Some(key) = api_key {
            if let Ok(client) = Client::new(key) {
                chat.submit(runtime, ui.ctx().clone(), client, model.to_string(), agent);
            }
        }
    }
}
