// Initial setup wizard. Centered card with progress dots; collects a
// wake-word label, records 10 voice samples, optionally takes an xAI
// API key, and persists everything.

use std::time::{Duration, Instant};

use crate::config::{Config, WakeTemplate};
use crate::theme;
use crate::voice::{extract_features, Recorder};

pub const SAMPLES_NEEDED: usize = 10;
const SAMPLE_SECONDS: f32 = 1.4;
const TOTAL_STEPS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Welcome,
    WakeWord,
    Voice,
    ApiKey,
    Autonomy,
    Done,
}

impl Step {
    fn index(self) -> usize {
        match self {
            Step::Welcome => 0,
            Step::WakeWord => 1,
            Step::Voice => 2,
            Step::ApiKey => 3,
            Step::Autonomy => 4,
            Step::Done => 5,
        }
    }
}

pub struct Wizard {
    pub step: Step,
    pub recording: bool,
    pub record_started_at: Option<Instant>,
    pub status: Option<String>,
    pub samples: Vec<WakeTemplate>,
    pub mic_unavailable: Option<String>,
}

impl Default for Wizard {
    fn default() -> Self {
        Self {
            step: Step::Welcome,
            recording: false,
            record_started_at: None,
            status: None,
            samples: Vec::new(),
            mic_unavailable: None,
        }
    }
}

pub enum WizardOutcome {
    Continue,
    Finished,
}

pub fn show(
    ui: &mut egui::Ui,
    cfg: &mut Config,
    wizard: &mut Wizard,
    recorder: Option<&Recorder>,
) -> WizardOutcome {
    if wizard.step == Step::Done {
        cfg.setup_complete = true;
        let _ = cfg.save();
        return WizardOutcome::Finished;
    }

    let avail = ui.available_rect_before_wrap();
    let card_w = 560.0_f32.min(avail.width() - 40.0);
    let card_x = avail.center().x - card_w / 2.0;
    let card_rect = egui::Rect::from_min_size(
        egui::pos2(card_x, avail.min.y + 60.0),
        egui::vec2(card_w, avail.height() - 120.0),
    );

    ui.allocate_ui_at_rect(card_rect, |ui| {
        // Brand: rounded accent square with a "J" wordmark.
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
            ui.painter().rect(
                rect,
                theme::rounding(12.0),
                theme::ACCENT,
                egui::Stroke::new(1.0, theme::ACCENT_HOV),
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "J",
                egui::FontId::proportional(26.0),
                theme::BLACK,
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("On-Hands Jarvis")
                    .color(theme::TEXT)
                    .strong()
                    .size(26.0),
            );
            ui.label(
                egui::RichText::new("Voice-activated desktop assistant")
                    .color(theme::TEXT_MUTED),
            );
            ui.add_space(14.0);
            theme::step_dots(ui, TOTAL_STEPS, wizard.step.index());
            ui.add_space(18.0);
        });

        theme::card(ui, |ui| {
            match wizard.step {
                Step::Welcome => welcome(ui, wizard),
                Step::WakeWord => wake_word(ui, cfg, wizard),
                Step::Voice => voice(ui, cfg, wizard, recorder),
                Step::ApiKey => api_key(ui, cfg, wizard),
                Step::Autonomy => autonomy(ui, cfg, wizard),
                Step::Done => {}
            }
        });
    });

    WizardOutcome::Continue
}

fn welcome(ui: &mut egui::Ui, wizard: &mut Wizard) {
    ui.label(
        egui::RichText::new("Hi.")
            .color(theme::TEXT)
            .strong()
            .size(20.0),
    );
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(
            "On-Hands Jarvis is a voice-activated assistant that can help with everyday \
             tasks, scan QR codes from your screen, and (with your permission) automate \
             parts of your computer. Setup takes about a minute.",
        )
        .color(theme::TEXT_MUTED),
    );
    ui.add_space(18.0);
    ui.vertical_centered(|ui| {
        if theme::primary_button(ui, "Get started", true).clicked() {
            wizard.step = Step::WakeWord;
        }
    });
}

fn wake_word(ui: &mut egui::Ui, cfg: &mut Config, wizard: &mut Wizard) {
    step_heading(ui, "Choose a wake word");
    ui.label(
        egui::RichText::new(
            "Pick the word or phrase you'll use to summon Jarvis. \"Jarvis\", \
             \"Garvis\", or \"Assistant\" all work — pick anything you like.",
        )
        .color(theme::TEXT_MUTED),
    );
    ui.add_space(14.0);
    theme::subcard(ui, |ui| {
        ui.label(egui::RichText::new("Wake word").color(theme::TEXT_MUTED).small());
        ui.add(
            egui::TextEdit::singleline(&mut cfg.wake_word_label)
                .desired_width(f32::INFINITY)
                .hint_text("Jarvis"),
        );
    });
    ui.add_space(20.0);
    nav_row(ui, wizard, Some(Step::Welcome), Some(Step::Voice), !cfg.wake_word_label.trim().is_empty());
}

fn voice(ui: &mut egui::Ui, cfg: &mut Config, wizard: &mut Wizard, recorder: Option<&Recorder>) {
    step_heading(ui, "Train your voice");
    ui.label(
        egui::RichText::new(format!(
            "Say \"{}\" {} times so Jarvis learns your voice. Speak clearly, with a \
             short pause between samples.",
            cfg.wake_word_label, SAMPLES_NEEDED
        ))
        .color(theme::TEXT_MUTED),
    );

    if let Some(err) = &wizard.mic_unavailable {
        ui.add_space(8.0);
        theme::subcard(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("Microphone unavailable — {err}"))
                    .color(egui::Color32::from_rgb(255, 170, 170)),
            );
        });
    }

    ui.add_space(14.0);
    theme::subcard(ui, |ui| {
        ui.horizontal(|ui| {
            theme::badge(
                ui,
                &format!("{} / {}", wizard.samples.len(), SAMPLES_NEEDED),
                wizard.samples.len() >= SAMPLES_NEEDED,
            );
            ui.add_space(8.0);
            let progress = wizard.samples.len() as f32 / SAMPLES_NEEDED as f32;
            ui.add(egui::ProgressBar::new(progress).desired_width(ui.available_width()).fill(theme::ACCENT));
        });
        ui.add_space(10.0);

        let mic_ready = recorder.is_some();
        if wizard.recording {
            if let Some(t0) = wizard.record_started_at {
                let elapsed = t0.elapsed().as_secs_f32();
                let remaining = (SAMPLE_SECONDS - elapsed).max(0.0);
                ui.horizontal(|ui| {
                    // Pulsing accent dot — cheap "alive" cue while recording.
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    let phase = (ui.ctx().input(|i| i.time) as f32 * 4.0).sin() * 0.5 + 0.5;
                    let halo_a = (60.0 + phase * 100.0) as u8;
                    ui.painter().circle_filled(
                        rect.center(),
                        4.0 + phase * 4.0,
                        egui::Color32::from_rgba_unmultiplied(
                            theme::ACCENT.r(),
                            theme::ACCENT.g(),
                            theme::ACCENT.b(),
                            halo_a,
                        ),
                    );
                    ui.painter().circle_filled(rect.center(), 4.0, theme::ACCENT);
                    ui.label(
                        egui::RichText::new(format!("Recording  ·  speak now  ·  {:.1}s", remaining))
                            .color(theme::ACCENT)
                            .strong(),
                    );
                });
                ui.ctx().request_repaint_after(Duration::from_millis(60));
                if elapsed >= SAMPLE_SECONDS {
                    if let Some(rec) = recorder {
                        let samples = rec.last_seconds(SAMPLE_SECONDS);
                        let feats = extract_features(&samples);
                        if feats.is_empty() {
                            wizard.status = Some("Sample too short, try again.".to_string());
                        } else {
                            wizard.samples.push(WakeTemplate {
                                features: feats.data,
                                frames: feats.frames,
                                bins: feats.bins,
                            });
                            wizard.status = Some(format!("Captured sample {}.", wizard.samples.len()));
                        }
                    }
                    wizard.recording = false;
                    wizard.record_started_at = None;
                }
            }
        } else {
            ui.horizontal(|ui| {
                let can_record = mic_ready && wizard.samples.len() < SAMPLES_NEEDED;
                let label = format!("Record sample {}", (wizard.samples.len() + 1).min(SAMPLES_NEEDED));
                if theme::primary_button(ui, &label, can_record).clicked() {
                    wizard.recording = true;
                    wizard.record_started_at = Some(Instant::now());
                }
                if !wizard.samples.is_empty() && theme::ghost_button(ui, "Discard last").clicked() {
                    wizard.samples.pop();
                }
            });
        }

        if let Some(s) = &wizard.status {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(s).color(theme::TEXT_DIM).small());
        }
    });

    ui.add_space(20.0);
    let advance = wizard.samples.len() >= SAMPLES_NEEDED;
    if advance {
        cfg.wake_templates = wizard.samples.clone();
        let _ = cfg.save();
    }
    nav_row(ui, wizard, Some(Step::WakeWord), Some(Step::ApiKey), advance);
}

fn api_key(ui: &mut egui::Ui, cfg: &mut Config, wizard: &mut Wizard) {
    step_heading(ui, "Connect to Grok / xAI");
    ui.label(
        egui::RichText::new(
            "Paste your xAI API key to enable the AI assistant. You can skip this and \
             add it later from Settings.",
        )
        .color(theme::TEXT_MUTED),
    );
    ui.add_space(14.0);
    theme::subcard(ui, |ui| {
        ui.label(egui::RichText::new("API key").color(theme::TEXT_MUTED).small());
        let mut key = cfg.xai_api_key.clone().unwrap_or_default();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut key)
                .password(true)
                .desired_width(f32::INFINITY)
                .hint_text("xai-…"),
        );
        if resp.changed() {
            cfg.xai_api_key = if key.trim().is_empty() { None } else { Some(key) };
            let _ = cfg.save();
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Model").color(theme::TEXT_MUTED).small());
        ui.add(
            egui::TextEdit::singleline(&mut cfg.xai_model)
                .desired_width(f32::INFINITY)
                .hint_text("grok-2-latest"),
        );
    });
    ui.add_space(20.0);
    nav_row(ui, wizard, Some(Step::Voice), Some(Step::Autonomy), true);
}

fn autonomy(ui: &mut egui::Ui, cfg: &mut Config, wizard: &mut Wizard) {
    step_heading(ui, "Autonomy safeguards");
    ui.label(
        egui::RichText::new(
            "Choose what Jarvis is allowed to do on your computer. You can change \
             these any time from Settings.",
        )
        .color(theme::TEXT_MUTED),
    );
    ui.add_space(14.0);
    theme::subcard(ui, |ui| {
        ui.checkbox(&mut cfg.autonomy.allow_app_launch, "Launch applications");
        ui.checkbox(&mut cfg.autonomy.allow_input_control, "Control mouse and keyboard");
        ui.checkbox(&mut cfg.autonomy.allow_file_writes, "Modify files on disk");
        ui.checkbox(&mut cfg.autonomy.allow_web_browsing, "Browse and interact with the web");
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);
        ui.checkbox(&mut cfg.qr_scanning_enabled, "Continuously scan the screen for QR codes");
    });
    ui.add_space(20.0);

    ui.horizontal(|ui| {
        if theme::ghost_button(ui, "Back").clicked() {
            wizard.step = Step::ApiKey;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::primary_button(ui, "Finish setup", true).clicked() {
                let _ = cfg.save();
                wizard.step = Step::Done;
            }
        });
    });
}

fn step_heading(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .color(theme::TEXT)
            .strong()
            .size(20.0),
    );
    ui.add_space(6.0);
}

fn nav_row(
    ui: &mut egui::Ui,
    wizard: &mut Wizard,
    back: Option<Step>,
    next: Option<Step>,
    can_advance: bool,
) {
    ui.horizontal(|ui| {
        if let Some(b) = back {
            if theme::ghost_button(ui, "Back").clicked() {
                wizard.step = b;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(n) = next {
                if theme::primary_button(ui, "Continue", can_advance).clicked() {
                    wizard.step = n;
                }
            }
        });
    });
}
