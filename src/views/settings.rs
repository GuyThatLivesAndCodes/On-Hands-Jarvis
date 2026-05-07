// Settings view: each grouping is its own subcard. Saves on the spot
// when any field changes.

use crate::config::Config;
use crate::theme;

pub struct SettingsResult {
    pub retrain_wake_word: bool,
    pub clear_wake_templates: bool,
}

pub fn show(ui: &mut egui::Ui, cfg: &mut Config) -> SettingsResult {
    let mut result = SettingsResult { retrain_wake_word: false, clear_wake_templates: false };

    theme::section_header(ui, "Settings", Some("Changes save automatically."));

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            xai_section(ui, cfg);
            ui.add_space(10.0);
            wake_section(ui, cfg, &mut result);
            ui.add_space(10.0);
            autonomy_section(ui, cfg);
            ui.add_space(10.0);
            qr_section(ui, cfg);
        });

    result
}

fn xai_section(ui: &mut egui::Ui, cfg: &mut Config) {
    theme::subcard(ui, |ui| {
        group_label(ui, "xAI / Grok");
        ui.label(egui::RichText::new("API key").color(theme::TEXT_MUTED).small());
        let mut key = cfg.xai_api_key.clone().unwrap_or_default();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut key)
                .password(true)
                .hint_text("xai-…")
                .desired_width(f32::INFINITY),
        );
        if resp.changed() {
            cfg.xai_api_key = if key.trim().is_empty() { None } else { Some(key) };
            let _ = cfg.save();
        }

        ui.add_space(6.0);
        ui.label(egui::RichText::new("Model").color(theme::TEXT_MUTED).small());
        let resp = ui.add(
            egui::TextEdit::singleline(&mut cfg.xai_model).desired_width(f32::INFINITY),
        );
        if resp.lost_focus() {
            let _ = cfg.save();
        }
    });
}

fn wake_section(ui: &mut egui::Ui, cfg: &mut Config, result: &mut SettingsResult) {
    theme::subcard(ui, |ui| {
        group_label(ui, "Wake word");
        ui.horizontal(|ui| {
            theme::badge(ui, &format!("\"{}\"", cfg.wake_word_label), true);
            theme::badge(ui, &format!("{} templates", cfg.wake_templates.len()), false);
        });
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Detection threshold").color(theme::TEXT_MUTED).small());
        ui.add(egui::Slider::new(&mut cfg.wake_threshold, 0.30..=0.95).show_value(true));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if theme::primary_button(ui, "Re-record samples", true).clicked() {
                result.retrain_wake_word = true;
            }
            if theme::ghost_button(ui, "Clear samples").clicked() {
                result.clear_wake_templates = true;
            }
            if theme::ghost_button(ui, "Save threshold").clicked() {
                let _ = cfg.save();
            }
        });
    });
}

fn autonomy_section(ui: &mut egui::Ui, cfg: &mut Config) {
    theme::subcard(ui, |ui| {
        group_label(ui, "Autonomy safeguards");
        ui.label(
            egui::RichText::new(
                "Toggle which categories of action Jarvis is permitted to take \
                 autonomously on your behalf.",
            )
            .color(theme::TEXT_MUTED)
            .small(),
        );
        ui.add_space(6.0);
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
}

fn qr_section(ui: &mut egui::Ui, cfg: &mut Config) {
    theme::subcard(ui, |ui| {
        group_label(ui, "QR scanning");
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
}

fn group_label(ui: &mut egui::Ui, s: &str) {
    ui.label(
        egui::RichText::new(s)
            .color(theme::TEXT)
            .strong()
            .size(15.0),
    );
    ui.add_space(6.0);
}
