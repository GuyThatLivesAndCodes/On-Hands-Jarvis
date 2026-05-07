// View modules for the post-setup main window.

pub mod chat;
pub mod qr_view;
pub mod settings;
pub mod system_view;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Chat,
    Qr,
    System,
    Settings,
}

impl Tab {
    pub fn label(self) -> &'static str {
        match self {
            Tab::Chat => "Chat",
            Tab::Qr => "QR Codes",
            Tab::System => "System",
            Tab::Settings => "Settings",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Chat => "Chat",
            Tab::Qr => "QR Codes",
            Tab::System => "System",
            Tab::Settings => "Settings",
        }
    }
}
