// Settings view: API key, model, autonomy safeguards, wake-word
// retraining shortcut. Saves on the spot when any field changes.

use crate::config::Config;
use crate::theme::{ACCENT, TEXT_MUTED};

pub struct SettingsResult {
    pub retrain_wake_word: bool,
    pub clear_wake_templates: bool,
}

pub fn show(ui: &mut egui::Ui, cfg: &mut Config) -> SettingsResult {
    let mut result = SettingsResult { retrain_wake_word: false, clear_wake_templates: false };

    ui.heading("Settings");
    ui.label(egui::RichText::new("Changes save automatically.").color(TEXT_MUTED));
    ui.add_space(10.0);

    ui.collapsing("xAI / Grok", |ui| {
        ui.label("API key");
        let mut key = cfg.xai_api_key.clone().unwrap_or_default();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut key)
                .password(true)
                .hint_text("xai-…"),
        );
        if resp.changed() {
            cfg.xai_api_key = if key.trim().is_empty() { None } else { Some(key) };
            let _ = cfg.save();
        }

        ui.add_space(4.0);
        ui.label("Model");
        let resp = ui.add(egui::TextEdit::singleline(&mut cfg.xai_model));
        if resp.lost_focus() {
            let _ = cfg.save();
        }
    });

    ui.add_space(8.0);
    ui.collapsing("Wake word", |ui| {
        ui.label(egui::RichText::new(format!("Current word: {}", cfg.wake_word_label)).color(ACCENT));
        ui.label(format!("Stored templates: {}", cfg.wake_templates.len()));
        ui.add(
            egui::Slider::new(&mut cfg.wake_threshold, 0.30..=0.95)
                .text("Detection threshold"),
        );
        ui.horizontal(|ui| {
            if ui.button("Re-record templates").clicked() {
                result.retrain_wake_word = true;
            }
            if ui.button("Clear templates").clicked() {
                result.clear_wake_templates = true;
            }
        });
        if ui.button("Save threshold").clicked() {
            let _ = cfg.save();
        }
    });

    ui.add_space(8.0);
    ui.collapsing("Autonomy safeguards", |ui| {
        ui.label(
            egui::RichText::new(
                "Toggle which categories of action Jarvis is permitted to take \
                 autonomously on your behalf.",
            )
            .color(TEXT_MUTED),
        );
        let mut changed = false;
        changed |= ui.checkbox(&mut cfg.autonomy.allow_app_launch, "Launch applications").changed();
        changed |= ui
            .checkbox(&mut cfg.autonomy.allow_input_control, "Control mouse and keyboard")
            .changed();
        changed |= ui
            .checkbox(&mut cfg.autonomy.allow_file_writes, "Modify files on disk")
            .changed();
        changed |= ui
            .checkbox(&mut cfg.autonomy.allow_web_browsing, "Browse and interact with the web")
            .changed();
        if changed {
            let _ = cfg.save();
        }
    });

    ui.add_space(8.0);
    ui.collapsing("QR scanning", |ui| {
        if ui
            .checkbox(
                &mut cfg.qr_scanning_enabled,
                "Continuously scan the screen for QR codes",
            )
            .changed()
        {
            let _ = cfg.save();
        }
    });

    result
}
