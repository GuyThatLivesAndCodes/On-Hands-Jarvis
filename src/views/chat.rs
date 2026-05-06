// Chat view: scrollable transcript + input box. Submitting fires an
// async xAI chat completion on the shared tokio runtime; replies arrive
// over an mpsc channel and we request a repaint when one shows up.

use std::sync::mpsc::{Receiver, Sender};

use crate::ai::{ChatMessage, Client, Role};
use crate::theme::{ACCENT, ACCENT_DIM, TEXT_MUTED};

pub struct ChatState {
    pub history: Vec<ChatMessage>,
    pub draft: String,
    pub awaiting: bool,
    pub error: Option<String>,
    sender: Sender<ChatEvent>,
    receiver: Receiver<ChatEvent>,
}

pub enum ChatEvent {
    Reply(String),
    Error(String),
}

impl Default for ChatState {
    fn default() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            history: vec![ChatMessage {
                role: Role::System,
                content: "You are Jarvis, a concise on-device assistant.".to_string(),
            }],
            draft: String::new(),
            awaiting: false,
            error: None,
            sender,
            receiver,
        }
    }
}

impl ChatState {
    pub fn drain_pending(&mut self) {
        while let Ok(ev) = self.receiver.try_recv() {
            match ev {
                ChatEvent::Reply(text) => {
                    self.history.push(ChatMessage { role: Role::Assistant, content: text });
                    self.awaiting = false;
                }
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
    ) {
        let prompt = self.draft.trim().to_string();
        if prompt.is_empty() || self.awaiting {
            return;
        }
        self.history.push(ChatMessage { role: Role::User, content: prompt });
        self.draft.clear();
        self.awaiting = true;
        self.error = None;

        let messages = self.history.clone();
        let tx = self.sender.clone();
        let model = model;
        runtime.spawn(async move {
            let req = crate::ai::ChatRequest {
                model,
                messages,
                temperature: Some(0.4),
                max_tokens: Some(800),
            };
            let result = client.chat(req).await;
            let event = match result {
                Ok(resp) => match resp.choices.into_iter().next() {
                    Some(c) => ChatEvent::Reply(c.message.content),
                    None => ChatEvent::Error("empty response".to_string()),
                },
                Err(e) => ChatEvent::Error(format!("{e:#}")),
            };
            let _ = tx.send(event);
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
) {
    let api_key = api_key.map(|s| s.to_string());
    ui.heading("Chat with Jarvis");
    ui.label(egui::RichText::new("Powered by xAI Grok").color(TEXT_MUTED));
    ui.add_space(8.0);

    let avail = ui.available_size();
    let log_height = (avail.y - 110.0).max(150.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .max_height(log_height)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for msg in &chat.history {
                if msg.role == Role::System {
                    continue;
                }
                let (label, color) = match msg.role {
                    Role::User => ("You", ACCENT),
                    Role::Assistant => ("Jarvis", ACCENT_DIM),
                    Role::System => ("System", TEXT_MUTED),
                };
                ui.label(egui::RichText::new(label).strong().color(color));
                ui.label(&msg.content);
                ui.add_space(6.0);
                ui.separator();
            }
            if chat.awaiting {
                ui.label(egui::RichText::new("Jarvis is thinking…").italics().color(TEXT_MUTED));
            }
            if let Some(err) = &chat.error {
                ui.label(egui::RichText::new(format!("Error: {err}")).color(egui::Color32::LIGHT_RED));
            }
        });

    ui.separator();
    ui.add_space(4.0);

    let key_present = api_key.as_deref().map(|k| !k.trim().is_empty()).unwrap_or(false);

    ui.horizontal(|ui| {
        let send_btn = egui::Button::new(egui::RichText::new("Send").strong()).min_size(egui::vec2(80.0, 32.0));
        let response = ui.add_sized(
            egui::vec2(ui.available_width() - 96.0, 32.0),
            egui::TextEdit::multiline(&mut chat.draft)
                .desired_rows(1)
                .hint_text(if key_present {
                    "Ask Jarvis anything…"
                } else {
                    "Set your xAI API key in Settings to chat."
                }),
        );
        let submit = ui.add_enabled(key_present && !chat.awaiting, send_btn).clicked();
        let enter = response.has_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
        if submit || enter {
            if let Some(key) = api_key {
                if let Ok(client) = Client::new(key) {
                    chat.submit(runtime, ui.ctx().clone(), client, model.to_string());
                }
            }
        }
    });
}
