// Initial setup wizard: collect a wake-word label, record 10 voice
// samples, optionally collect an xAI API key, and persist everything.

use std::time::{Duration, Instant};

use crate::config::{Config, WakeTemplate};
use crate::theme::{ACCENT, TEXT_MUTED};
use crate::voice::{extract_features, Recorder};

pub const SAMPLES_NEEDED: usize = 10;
const SAMPLE_SECONDS: f32 = 1.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Welcome,
    WakeWord,
    Voice,
    ApiKey,
    Autonomy,
    Done,
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
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.heading(egui::RichText::new("On-Hands Jarvis").size(28.0).color(ACCENT));
        ui.label(egui::RichText::new("Let's set things up.").color(TEXT_MUTED));
        ui.add_space(20.0);
    });

    match wizard.step {
        Step::Welcome => welcome(ui, wizard),
        Step::WakeWord => wake_word(ui, cfg, wizard),
        Step::Voice => voice(ui, cfg, wizard, recorder),
        Step::ApiKey => api_key(ui, cfg, wizard),
        Step::Autonomy => autonomy(ui, cfg, wizard),
        Step::Done => {
            cfg.setup_complete = true;
            let _ = cfg.save();
            return WizardOutcome::Finished;
        }
    }

    WizardOutcome::Continue
}

fn welcome(ui: &mut egui::Ui, wizard: &mut Wizard) {
    ui.vertical_centered(|ui| {
        ui.label(
            "On-Hands Jarvis is a voice-activated assistant that can help with \
             everyday tasks, scan QR codes from your screen, and (with your \
             permission) automate parts of your computer.",
        );
        ui.add_space(20.0);
        if ui
            .add_sized(egui::vec2(160.0, 36.0), egui::Button::new("Get started"))
            .clicked()
        {
            wizard.step = Step::WakeWord;
        }
    });
}

fn wake_word(ui: &mut egui::Ui, cfg: &mut Config, wizard: &mut Wizard) {
    ui.label(egui::RichText::new("1 / 5  Choose a wake word").strong());
    ui.label(
        "Pick the word or phrase you'll use to summon Jarvis. \"Jarvis\", \
         \"Garvis\", or \"Assistant\" all work — you can pick anything.",
    );
    ui.add_space(8.0);
    ui.add(egui::TextEdit::singleline(&mut cfg.wake_word_label).hint_text("Wake word"));
    ui.add_space(16.0);
    nav_buttons(ui, wizard, Some(Step::Welcome), Some(Step::Voice), !cfg.wake_word_label.trim().is_empty());
}

fn voice(ui: &mut egui::Ui, cfg: &mut Config, wizard: &mut Wizard, recorder: Option<&Recorder>) {
    ui.label(egui::RichText::new("2 / 5  Train your voice").strong());
    ui.label(format!(
        "Record \"{}\" {} times so Jarvis learns your voice.",
        cfg.wake_word_label, SAMPLES_NEEDED
    ));
    ui.add_space(8.0);

    if let Some(err) = &wizard.mic_unavailable {
        ui.colored_label(
            egui::Color32::LIGHT_RED,
            format!("Microphone unavailable: {err}"),
        );
        ui.add_space(8.0);
    }

    ui.label(format!("Captured: {} / {}", wizard.samples.len(), SAMPLES_NEEDED));
    let progress = wizard.samples.len() as f32 / SAMPLES_NEEDED as f32;
    ui.add(egui::ProgressBar::new(progress).fill(ACCENT));
    ui.add_space(12.0);

    let mic_ready = recorder.is_some();

    if wizard.recording {
        if let Some(t0) = wizard.record_started_at {
            let elapsed = t0.elapsed().as_secs_f32();
            let remaining = (SAMPLE_SECONDS - elapsed).max(0.0);
            ui.label(
                egui::RichText::new(format!("Recording… speak now ({:.1}s)", remaining))
                    .color(ACCENT),
            );
            ui.ctx().request_repaint_after(Duration::from_millis(80));
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
            let btn = egui::Button::new(
                egui::RichText::new(format!("● Record sample {}", wizard.samples.len() + 1)).strong(),
            )
            .min_size(egui::vec2(200.0, 36.0));
            if ui.add_enabled(can_record, btn).clicked() {
                wizard.recording = true;
                wizard.record_started_at = Some(Instant::now());
            }
            if ui.button("Discard last").clicked() && !wizard.samples.is_empty() {
                wizard.samples.pop();
            }
        });
    }

    if let Some(s) = &wizard.status {
        ui.label(egui::RichText::new(s).color(TEXT_MUTED));
    }

    ui.add_space(16.0);
    let advance = wizard.samples.len() >= SAMPLES_NEEDED;
    if advance {
        cfg.wake_templates = wizard.samples.clone();
        let _ = cfg.save();
    }
    nav_buttons(ui, wizard, Some(Step::WakeWord), Some(Step::ApiKey), advance);
}

fn api_key(ui: &mut egui::Ui, cfg: &mut Config, wizard: &mut Wizard) {
    ui.label(egui::RichText::new("3 / 5  Connect to Grok / xAI").strong());
    ui.label(
        "Paste your xAI API key to enable the AI assistant. You can skip this \
         and add it later from Settings.",
    );
    ui.add_space(8.0);
    let mut key = cfg.xai_api_key.clone().unwrap_or_default();
    if ui
        .add(
            egui::TextEdit::singleline(&mut key)
                .password(true)
                .hint_text("xai-…"),
        )
        .changed()
    {
        cfg.xai_api_key = if key.trim().is_empty() { None } else { Some(key) };
        let _ = cfg.save();
    }
    ui.add_space(8.0);
    ui.add(egui::TextEdit::singleline(&mut cfg.xai_model).hint_text("Model (e.g. grok-2-latest)"));
    ui.add_space(16.0);
    nav_buttons(ui, wizard, Some(Step::Voice), Some(Step::Autonomy), true);
}

fn autonomy(ui: &mut egui::Ui, cfg: &mut Config, wizard: &mut Wizard) {
    ui.label(egui::RichText::new("4 / 5  Autonomy safeguards").strong());
    ui.label(
        "Choose what Jarvis is allowed to do on your computer. You can change \
         these any time from Settings.",
    );
    ui.add_space(8.0);
    ui.checkbox(&mut cfg.autonomy.allow_app_launch, "Launch applications");
    ui.checkbox(&mut cfg.autonomy.allow_input_control, "Control mouse and keyboard");
    ui.checkbox(&mut cfg.autonomy.allow_file_writes, "Modify files on disk");
    ui.checkbox(&mut cfg.autonomy.allow_web_browsing, "Browse and interact with the web");
    ui.add_space(8.0);
    ui.checkbox(&mut cfg.qr_scanning_enabled, "Continuously scan the screen for QR codes");
    ui.add_space(16.0);
    if ui
        .add_sized(egui::vec2(160.0, 36.0), egui::Button::new("Finish setup"))
        .clicked()
    {
        let _ = cfg.save();
        wizard.step = Step::Done;
    }
    if ui.button("Back").clicked() {
        wizard.step = Step::ApiKey;
    }
}

fn nav_buttons(
    ui: &mut egui::Ui,
    wizard: &mut Wizard,
    back: Option<Step>,
    next: Option<Step>,
    can_advance: bool,
) {
    ui.horizontal(|ui| {
        if let Some(b) = back {
            if ui.button("Back").clicked() {
                wizard.step = b;
            }
        }
        if let Some(n) = next {
            if ui
                .add_enabled(
                    can_advance,
                    egui::Button::new(egui::RichText::new("Next").strong())
                        .min_size(egui::vec2(120.0, 32.0)),
                )
                .clicked()
            {
                wizard.step = n;
            }
        }
    });
}
