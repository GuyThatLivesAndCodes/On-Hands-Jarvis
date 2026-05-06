// Top-level eframe application: owns config, runtime, recorder, scanner,
// and dispatches between the setup wizard and the main tabbed view.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::ai::{ChatMessage, Role};
use crate::automation::system::{SystemMonitor, SystemSnapshot};
use crate::config::Config;
use crate::qr::{ScannedCode, Scanner};
use crate::theme;
use crate::views::chat::ChatState;
use crate::views::{self, Tab};
use crate::voice::{Recorder, WakeDetector};
use crate::wizard::{self, Wizard, WizardOutcome};

const WAKE_WINDOW_SECS: f32 = 1.4;
const WAKE_CHECK_INTERVAL: Duration = Duration::from_millis(400);
const SYSTEM_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

pub struct JarvisApp {
    pub config: Config,
    pub runtime: Arc<tokio::runtime::Runtime>,

    pub recorder: Option<Recorder>,
    pub mic_error: Option<String>,

    pub scanner: Scanner,
    pub system_monitor: SystemMonitor,
    pub system_snapshot: SystemSnapshot,
    pub last_system_refresh: Instant,
    pub last_wake_check: Instant,
    pub last_wake_score: f32,
    pub last_wake_event: Option<Instant>,

    pub tab: Tab,
    pub wizard: Wizard,
    pub chat: ChatState,
}

impl JarvisApp {
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: Arc<tokio::runtime::Runtime>) -> Self {
        theme::apply(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let config = Config::load().unwrap_or_else(|e| {
            log::warn!("config load failed, falling back to defaults: {e}");
            Config::default()
        });

        let (recorder, mic_error) = match Recorder::start(WAKE_WINDOW_SECS.max(2.0)) {
            Ok(r) => (Some(r), None),
            Err(e) => {
                log::warn!("microphone unavailable: {e:#}");
                (None, Some(e.to_string()))
            }
        };

        let mut system_monitor = SystemMonitor::new();
        let system_snapshot = system_monitor.snapshot();

        let tab = if config.setup_complete { Tab::Chat } else { Tab::Chat };
        let wizard = Wizard {
            mic_unavailable: mic_error.clone(),
            ..Default::default()
        };

        Self {
            config,
            runtime,
            recorder,
            mic_error,
            scanner: Scanner::default(),
            system_monitor,
            system_snapshot,
            last_system_refresh: Instant::now(),
            last_wake_check: Instant::now(),
            last_wake_score: 0.0,
            last_wake_event: None,
            tab,
            wizard,
            chat: ChatState::default(),
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        // Periodic system snapshot.
        if self.last_system_refresh.elapsed() >= SYSTEM_REFRESH_INTERVAL {
            self.system_snapshot = self.system_monitor.snapshot();
            self.last_system_refresh = Instant::now();
        }

        // QR scanning if enabled.
        if self.config.qr_scanning_enabled && self.config.setup_complete {
            if let Err(e) = self.scanner.tick() {
                log::debug!("qr scan failed: {e}");
            }
        }

        // Wake-word detection if mic + templates available.
        if let (Some(rec), true) = (
            &self.recorder,
            self.config.setup_complete && !self.config.wake_templates.is_empty(),
        ) {
            if self.last_wake_check.elapsed() >= WAKE_CHECK_INTERVAL {
                self.last_wake_check = Instant::now();
                let samples = rec.last_seconds(WAKE_WINDOW_SECS);
                if samples.len() as f32 >= WAKE_WINDOW_SECS * 16_000.0 * 0.5 {
                    let detector = WakeDetector::new(
                        &self.config.wake_templates,
                        self.config.wake_threshold,
                    );
                    let (score, hit) = detector.score(&samples);
                    self.last_wake_score = score;
                    if hit.is_some()
                        && self
                            .last_wake_event
                            .map(|t| t.elapsed() > Duration::from_secs(2))
                            .unwrap_or(true)
                    {
                        self.last_wake_event = Some(Instant::now());
                        log::info!("wake word matched (score={score:.2})");
                        self.tab = Tab::Chat;
                    }
                }
            }
        }

        self.chat.drain_pending();

        // Drive the UI at ~10Hz so timers/scans/wake checks tick smoothly.
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn show_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("●").color(if self.recorder.is_some() {
                theme::ACCENT
            } else {
                theme::TEXT_MUTED
            }));
            ui.label(if self.recorder.is_some() {
                format!(
                    "Listening for \"{}\" (score {:.2})",
                    self.config.wake_word_label, self.last_wake_score
                )
            } else {
                format!(
                    "Microphone unavailable: {}",
                    self.mic_error.clone().unwrap_or_else(|| "no input device".into())
                )
            });

            if let Some(t) = self.last_wake_event {
                if t.elapsed() < Duration::from_secs(3) {
                    ui.separator();
                    ui.label(egui::RichText::new("Wake word!").color(theme::ACCENT).strong());
                }
            }

            ui.separator();
            ui.label(format!(
                "{:.0}% CPU · {} / {} MB",
                self.system_snapshot.cpu_usage_percent,
                self.system_snapshot.mem_used_mb,
                self.system_snapshot.mem_total_mb
            ));

            if !self.scanner.codes.is_empty() {
                ui.separator();
                ui.label(format!("{} QR code(s) on screen", self.scanner.codes.len()));
            }
        });
    }
}

impl eframe::App for JarvisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);

        // Background gradient covering the full window.
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                theme::paint_gradient(ui, rect);

                // Inner padded area.
                let inner = rect.shrink2(egui::vec2(20.0, 16.0));
                let mut child = ui.child_ui(inner, *ui.layout(), None);

                if !self.config.setup_complete {
                    self.show_wizard(&mut child);
                } else {
                    self.show_main(&mut child);
                }
            });
    }
}

impl JarvisApp {
    fn show_wizard(&mut self, ui: &mut egui::Ui) {
        let outcome = wizard::show(ui, &mut self.config, &mut self.wizard, self.recorder.as_ref());
        if let WizardOutcome::Finished = outcome {
            self.tab = Tab::Chat;
            // Fresh chat state with the wake word baked into the system prompt.
            self.chat = ChatState::default();
            self.chat.history = vec![ChatMessage {
                role: Role::System,
                content: format!(
                    "You are Jarvis, the user's on-device assistant. They activated you with the wake word \"{}\". Be concise.",
                    self.config.wake_word_label
                ),
            }];
        }
    }

    fn show_main(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("On-Hands Jarvis").color(theme::ACCENT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for tab in [Tab::Settings, Tab::System, Tab::Qr, Tab::Chat] {
                    let selected = self.tab == tab;
                    let label = egui::RichText::new(tab.label())
                        .color(if selected { theme::ACCENT } else { theme::TEXT_MUTED });
                    if ui.selectable_label(selected, label).clicked() {
                        self.tab = tab;
                    }
                }
            });
        });
        ui.separator();
        ui.add_space(6.0);

        match self.tab {
            Tab::Chat => {
                views::chat::show(
                    ui,
                    &mut self.chat,
                    &self.runtime,
                    self.config.xai_api_key.as_deref(),
                    &self.config.xai_model,
                );
            }
            Tab::Qr => {
                views::qr_view::show(ui, &self.scanner.codes, self.config.qr_scanning_enabled);
            }
            Tab::System => {
                views::system_view::show(ui, &self.system_snapshot);
            }
            Tab::Settings => {
                let result = views::settings::show(ui, &mut self.config);
                if result.clear_wake_templates {
                    self.config.wake_templates.clear();
                    let _ = self.config.save();
                }
                if result.retrain_wake_word {
                    self.wizard = Wizard {
                        mic_unavailable: self.mic_error.clone(),
                        step: wizard::Step::Voice,
                        ..Default::default()
                    };
                    self.config.wake_templates.clear();
                    self.config.setup_complete = false;
                    let _ = self.config.save();
                }
            }
        }

        ui.add_space(6.0);
        ui.separator();
        self.show_status_bar(ui);
    }
}

// Silence dead-code warnings for fields read only by closures or
// platform-specific builds.
#[allow(dead_code)]
fn _used(_codes: &[ScannedCode]) {}
