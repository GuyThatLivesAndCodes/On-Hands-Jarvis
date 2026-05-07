// Chat view: a left chat-list pane plus the message transcript +
// composer. Submitting fires an async xAI chat completion that drives
// an agent loop with local tools. Conversations are persisted under
// `<config_dir>/chats/`; "New chat" starts fresh, clicking a saved
// conversation loads it.

use std::sync::mpsc::{Receiver, Sender};

use chrono::Utc;

use crate::ai::tools::UiCommand;
use crate::ai::{
    available_tools, catalog_summary, execute_tool, AgentContext, ChatMessage, ChatRequest,
    Client, Role,
};
use crate::chat_store::{self, ChatSummary, SavedChat};
use crate::theme;

const AGENT_LOOP_LIMIT: usize = 6;

pub struct ChatState {
    pub history: Vec<ChatMessage>,
    pub draft: String,
    pub awaiting: bool,
    pub error: Option<String>,
    pub current_id: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub title: String,
    pub saved_chats: Vec<ChatSummary>,
    sender: Sender<ChatEvent>,
    receiver: Receiver<ChatEvent>,
    pub ui_tx: Sender<UiCommand>,
    pub ui_rx: Receiver<UiCommand>,
}

pub enum ChatEvent {
    Assistant(ChatMessage),
    ToolResult(ChatMessage),
    Done,
    Error(String),
}

impl Default for ChatState {
    fn default() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let (ui_tx, ui_rx) = std::sync::mpsc::channel();
        Self {
            history: vec![ChatMessage::system(
                "You are Jarvis, the user's on-device assistant. Be concise.",
            )],
            draft: String::new(),
            awaiting: false,
            error: None,
            current_id: None,
            created_at: Utc::now(),
            title: String::new(),
            saved_chats: Vec::new(),
            sender,
            receiver,
            ui_tx,
            ui_rx,
        }
    }
}

impl ChatState {
    pub fn refresh_chat_list(&mut self) {
        match chat_store::list() {
            Ok(list) => self.saved_chats = list,
            Err(e) => log::warn!("chat list refresh failed: {e}"),
        }
    }

    pub fn refresh_system_prompt(&mut self, agent: &AgentContext) {
        let prompt = format!(
            "You are Jarvis, the user's on-device voice-activated desktop assistant.\n\
             The wake word is \"{wake}\".\n\
             You can call the following tools to help the user. Prefer tools over \
             guessing when the user asks about live state.\n\n{tools}\n\n\
             Be concise. When you have enough information, reply naturally in plain text.",
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
                ChatEvent::Done => {
                    self.awaiting = false;
                    self.persist_current();
                    self.refresh_chat_list();
                }
                ChatEvent::Error(e) => {
                    self.error = Some(e);
                    self.awaiting = false;
                }
            }
        }
    }

    pub fn drain_ui_commands(&self, ctx: &egui::Context) {
        while let Ok(cmd) = self.ui_rx.try_recv() {
            match cmd {
                UiCommand::SetClipboard(text) => ctx.copy_text(text),
            }
        }
    }

    fn persist_current(&mut self) {
        if self.title.is_empty() {
            self.title = chat_store::derive_title(&self.history);
        }
        if self.current_id.is_none() {
            self.current_id = Some(chat_store::new_id());
            self.created_at = Utc::now();
        }
        if let Some(id) = &self.current_id {
            let chat = SavedChat {
                id: id.clone(),
                title: self.title.clone(),
                created_at: self.created_at,
                updated_at: Utc::now(),
                messages: self.history.clone(),
            };
            if let Err(e) = chat_store::save(&chat) {
                log::warn!("persist chat failed: {e}");
            }
        }
    }

    pub fn start_new_chat(&mut self) {
        // Persist whatever we already had (if anything user-facing).
        let has_user_msg = self
            .history
            .iter()
            .any(|m| m.role == Role::User || m.role == Role::Assistant);
        if has_user_msg {
            self.persist_current();
        }
        self.history.retain(|m| m.role == Role::System);
        if self.history.is_empty() {
            self.history.push(ChatMessage::system("You are Jarvis, the user's on-device assistant. Be concise."));
        }
        self.draft.clear();
        self.awaiting = false;
        self.error = None;
        self.current_id = None;
        self.title = String::new();
        self.created_at = Utc::now();
    }

    pub fn load_chat(&mut self, id: &str) {
        match chat_store::load(id) {
            Ok(chat) => {
                self.history = chat.messages;
                self.title = chat.title;
                self.created_at = chat.created_at;
                self.current_id = Some(chat.id);
                self.draft.clear();
                self.error = None;
                self.awaiting = false;
            }
            Err(e) => self.error = Some(format!("load chat: {e:#}")),
        }
    }

    pub fn delete_chat(&mut self, id: &str) {
        if let Err(e) = chat_store::delete(id) {
            log::warn!("delete chat: {e}");
        }
        if self.current_id.as_deref() == Some(id) {
            self.start_new_chat();
        }
        self.refresh_chat_list();
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

        // Persist the user turn immediately so a crash mid-reply doesn't lose it.
        self.persist_current();

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
    chat.drain_ui_commands(ui.ctx());

    let api_key_owned = api_key.map(|s| s.to_string());

    // Two-pane layout: chat list on the left, transcript + composer on the right.
    let avail = ui.available_rect_before_wrap();
    let list_w = 200.0_f32.min(avail.width() * 0.30);

    let list_rect = egui::Rect::from_min_size(avail.min, egui::vec2(list_w, avail.height()));
    let main_rect = egui::Rect::from_min_max(
        egui::pos2(avail.min.x + list_w + 10.0, avail.min.y),
        avail.max,
    );

    ui.allocate_ui_at_rect(list_rect, |ui| chat_list(ui, chat));
    ui.allocate_ui_at_rect(main_rect, |ui| {
        chat_main(ui, chat, runtime, api_key_owned.as_deref(), model, agent);
    });
}

fn chat_list(ui: &mut egui::Ui, chat: &mut ChatState) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Chats")
                    .color(theme::TEXT)
                    .strong()
                    .size(14.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::icon_button(ui, "↻").clicked() {
                    chat.refresh_chat_list();
                }
            });
        });
        ui.add_space(6.0);
        if theme::primary_button(ui, "+ New chat", true).clicked() {
            chat.start_new_chat();
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .max_height(ui.available_height() - 4.0)
            .show(ui, |ui| {
                if chat.saved_chats.is_empty() {
                    ui.label(
                        egui::RichText::new("No saved chats yet.")
                            .color(theme::TEXT_DIM)
                            .small(),
                    );
                }
                let mut to_delete: Option<String> = None;
                let mut to_load: Option<String> = None;

                let summaries = chat.saved_chats.clone();
                for entry in &summaries {
                    let selected = chat.current_id.as_deref() == Some(entry.id.as_str());
                    let row_h = 38.0;
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_h),
                        egui::Sense::click(),
                    );
                    let bg = if selected {
                        theme::SURFACE_HOV
                    } else if resp.hovered() {
                        theme::SURFACE_2
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    ui.painter().rect(
                        rect,
                        theme::rounding(theme::R_FIELD),
                        bg,
                        egui::Stroke::NONE,
                    );
                    if selected {
                        let stripe = egui::Rect::from_min_size(
                            rect.left_top() + egui::vec2(0.0, 4.0),
                            egui::vec2(2.5, rect.height() - 8.0),
                        );
                        ui.painter().rect_filled(stripe, theme::rounding(1.0), theme::ACCENT);
                    }
                    // Title
                    let title = if entry.title.trim().is_empty() {
                        "(untitled)"
                    } else {
                        entry.title.as_str()
                    };
                    ui.painter().text(
                        rect.left_top() + egui::vec2(10.0, 6.0),
                        egui::Align2::LEFT_TOP,
                        truncate(title, 28),
                        egui::FontId::proportional(13.5),
                        theme::TEXT,
                    );
                    ui.painter().text(
                        rect.left_top() + egui::vec2(10.0, rect.height() - 6.0),
                        egui::Align2::LEFT_BOTTOM,
                        entry.updated_at.format("%Y-%m-%d %H:%M").to_string(),
                        egui::FontId::proportional(11.0),
                        theme::TEXT_DIM,
                    );

                    // A delete-X target on the right of the row.
                    let close_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.right() - 14.0, rect.center().y),
                        egui::vec2(20.0, 20.0),
                    );
                    let close_resp = ui.interact(close_rect, ui.id().with(("close", &entry.id)), egui::Sense::click());
                    let close_color = if close_resp.hovered() { theme::TEXT } else { theme::TEXT_DIM };
                    ui.painter().text(
                        close_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "x",
                        egui::FontId::proportional(13.0),
                        close_color,
                    );
                    if close_resp.clicked() {
                        to_delete = Some(entry.id.clone());
                    } else if resp.clicked() {
                        to_load = Some(entry.id.clone());
                    }
                }

                if let Some(id) = to_load {
                    chat.load_chat(&id);
                }
                if let Some(id) = to_delete {
                    chat.delete_chat(&id);
                }
            });
    });
}

fn chat_main(
    ui: &mut egui::Ui,
    chat: &mut ChatState,
    runtime: &tokio::runtime::Runtime,
    api_key: Option<&str>,
    model: &str,
    agent: AgentContext,
) {
    let key_present = api_key.map(|k| !k.trim().is_empty()).unwrap_or(false);

    let title_text = if chat.title.is_empty() {
        "New chat".to_string()
    } else {
        chat.title.clone()
    };
    theme::section_header(
        ui,
        &title_text,
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

    composer(ui, chat, runtime, api_key, model, key_present, agent);
}

fn bubble(ui: &mut egui::Ui, msg: &ChatMessage) {
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

        let label = if is_user { "You" } else { "Jarvis" };

        egui::Frame::none()
            .fill(theme::SURFACE_2)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .rounding(theme::rounding(theme::R_CARD))
            .inner_margin(egui::Margin::symmetric(14.0, 10.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .color(theme::TEXT_MUTED)
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
                    theme::badge(ui, &format!("{name} → result"), false);
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
        let n = ((t * 2.0) as usize) % 4;
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

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let cut: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}
