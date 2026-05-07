// Settings view: each grouping is its own subcard. Saves on the spot
// when any field changes.

use crate::config::Config;
use crate::theme;

#[derive(Default)]
pub struct SettingsResult {
    pub retrain_wake_word: bool,
    pub clear_wake_templates: bool,
    pub record_negative_sample: bool,
    pub clear_negative_templates: bool,
    pub refresh_audio_devices: bool,
    pub mic_changed: bool,
}

#[derive(Default, Clone)]
pub struct AudioDevices {
    pub input: Vec<String>,
    pub output: Vec<String>,
}

pub fn show(ui: &mut egui::Ui, cfg: &mut Config, devices: &AudioDevices) -> SettingsResult {
    let mut result = SettingsResult::default();

    theme::section_header(ui, "Settings", Some("Changes save automatically."));

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            xai_section(ui, cfg);
            ui.add_space(10.0);
            audio_section(ui, cfg, devices, &mut result);
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

fn audio_section(
    ui: &mut egui::Ui,
    cfg: &mut Config,
    devices: &AudioDevices,
    result: &mut SettingsResult,
) {
    theme::subcard(ui, |ui| {
        ui.horizontal(|ui| {
            group_label_inline(ui, "Audio devices");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::ghost_button(ui, "Refresh").clicked() {
                    result.refresh_audio_devices = true;
                }
            });
        });

        ui.add_space(6.0);
        ui.label(egui::RichText::new("Input (microphone)").color(theme::TEXT_MUTED).small());
        device_picker(
            ui,
            "input_device",
            &devices.input,
            &mut cfg.mic_device,
            "(System default)",
            || result.mic_changed = true,
        );

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Output (speakers)").color(theme::TEXT_MUTED).small());
        device_picker(
            ui,
            "output_device",
            &devices.output,
            &mut cfg.output_device,
            "(System default)",
            || {},
        );

        let _ = cfg.save();
    });
}

fn device_picker(
    ui: &mut egui::Ui,
    id: &str,
    devices: &[String],
    selected: &mut Option<String>,
    default_label: &str,
    mut on_change: impl FnMut(),
) {
    let current = selected
        .clone()
        .unwrap_or_else(|| default_label.to_string());
    egui::ComboBox::from_id_source(id)
        .selected_text(current.clone())
        .width(ui.available_width().min(420.0))
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(selected.is_none(), default_label)
                .clicked()
            {
                if selected.is_some() {
                    *selected = None;
                    on_change();
                }
            }
            for name in devices {
                let is_sel = selected.as_deref() == Some(name.as_str());
                if ui.selectable_label(is_sel, name).clicked() && !is_sel {
                    *selected = Some(name.clone());
                    on_change();
                }
            }
        });
}

fn wake_section(ui: &mut egui::Ui, cfg: &mut Config, result: &mut SettingsResult) {
    theme::subcard(ui, |ui| {
        group_label(ui, "Wake word");
        ui.horizontal(|ui| {
            theme::badge(ui, &format!("\"{}\"", cfg.wake_word_label), true);
            theme::badge(ui, &format!("{} positive", cfg.wake_templates.len()), false);
            theme::badge(
                ui,
                &format!("{} negative", cfg.wake_negative_templates.len()),
                false,
            );
        });
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Detection threshold").color(theme::TEXT_MUTED).small());
        if ui.add(egui::Slider::new(&mut cfg.wake_threshold, 0.30..=0.95)
            .show_value(true))
            .changed()
        {
            let _ = cfg.save();
        }

        ui.add_space(4.0);
        ui.label(egui::RichText::new("Cooldown after a trigger (seconds)").color(theme::TEXT_MUTED).small());
        let mut cooldown = cfg.wake_cooldown_secs as i32;
        if ui.add(egui::Slider::new(&mut cooldown, 1..=60)).changed() {
            cfg.wake_cooldown_secs = cooldown.max(1) as u32;
            let _ = cfg.save();
        }

        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(
                "Add more positive samples (you saying the wake word) and negative \
                 samples (background speech, near-misses, ambient noise) to reduce \
                 false triggers.",
            )
            .color(theme::TEXT_MUTED)
            .small(),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if theme::primary_button(ui, "Add positive sample", true).clicked() {
                result.retrain_wake_word = true;
            }
            if theme::ghost_button(ui, "Clear positives").clicked() {
                result.clear_wake_templates = true;
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if theme::primary_button(ui, "Add negative sample", true).clicked() {
                result.record_negative_sample = true;
            }
            if theme::ghost_button(ui, "Clear negatives").clicked() {
                result.clear_negative_templates = true;
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
        changed |= ui.checkbox(&mut cfg.autonomy.allow_app_launch, "Launch and close applications").changed();
        changed |= ui.checkbox(&mut cfg.autonomy.allow_input_control, "Control mouse, keyboard, and clipboard").changed();
        changed |= ui.checkbox(&mut cfg.autonomy.allow_file_writes, "Modify files on disk").changed();
        changed |= ui.checkbox(&mut cfg.autonomy.allow_web_browsing, "Browse and interact with the web").changed();
        changed |= ui.checkbox(&mut cfg.autonomy.allow_screen_capture, "Capture screenshots").changed();
        changed |= ui.checkbox(&mut cfg.autonomy.allow_shell_commands, "Run shell commands  (high blast radius)").changed();
        if changed {
            let _ = cfg.save();
        }
    });
}

fn qr_section(ui: &mut egui::Ui, cfg: &mut Config) {
    theme::subcard(ui, |ui| {
        group_label(ui, "QR scanning");
        let mut changed = false;
        changed |= ui
            .checkbox(
                &mut cfg.qr_scanning_enabled,
                "Continuously scan the screen for QR codes",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut cfg.qr_overlay_enabled,
                "Draw outlines on the screen with Open / Copy buttons",
            )
            .changed();
        if changed {
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

fn group_label_inline(ui: &mut egui::Ui, s: &str) {
    ui.label(
        egui::RichText::new(s)
            .color(theme::TEXT)
            .strong()
            .size(15.0),
    );
}
