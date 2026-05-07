// Top-level eframe application. Owns config, runtime, audio capture,
// QR scanner, chat state, and dispatches between the setup wizard and
// the main shell. Also opens a transparent QR-overlay viewport whenever
// codes are visible on screen, and a floating "wake chat" viewport
// when the wake word fires.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::ai::tools::AgentContext;
use crate::ai::ChatMessage;
use crate::automation::system::{SystemMonitor, SystemSnapshot};
use crate::config::{Config, WakeTemplate};
use crate::qr::Scanner;
use crate::theme;
use crate::views::chat::ChatState;
use crate::views::settings::AudioDevices;
use crate::views::wake_overlay::WakeOverlay;
use crate::views::{self, Tab};
use crate::voice::{extract_features, list_input_devices, list_output_devices, Recorder, WakeDetector};
use crate::wizard::{self, Wizard, WizardOutcome};

const WAKE_WINDOW_SECS: f32 = 1.4;
const WAKE_CHECK_INTERVAL: Duration = Duration::from_millis(400);
const SYSTEM_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const NEGATIVE_SAMPLE_SECS: f32 = 1.6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NegativeRecording {
    Idle,
    Capturing { started_at: Instant },
}

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
    pub wake_overlay: WakeOverlay,

    pub audio_devices: AudioDevices,
    negative_recording: NegativeRecording,
    pub status_flash: Option<(Instant, String)>,
}

impl JarvisApp {
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: Arc<tokio::runtime::Runtime>) -> Self {
        theme::apply(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let config = Config::load().unwrap_or_else(|e| {
            log::warn!("config load failed, falling back to defaults: {e}");
            Config::default()
        });

        let (recorder, mic_error) = start_recorder(&config);
        let mut system_monitor = SystemMonitor::new();
        let system_snapshot = system_monitor.snapshot();

        let wizard = Wizard {
            mic_unavailable: mic_error.clone(),
            ..Default::default()
        };

        let mut chat = ChatState::default();
        chat.refresh_chat_list();

        let audio_devices = AudioDevices {
            input: list_input_devices(),
            output: list_output_devices(),
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
            tab: Tab::Chat,
            wizard,
            chat,
            wake_overlay: WakeOverlay::default(),
            audio_devices,
            negative_recording: NegativeRecording::Idle,
            status_flash: None,
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        if self.last_system_refresh.elapsed() >= SYSTEM_REFRESH_INTERVAL {
            self.system_snapshot = self.system_monitor.snapshot();
            self.last_system_refresh = Instant::now();
        }

        if self.config.qr_scanning_enabled && self.config.setup_complete {
            if let Err(e) = self.scanner.tick() {
                log::debug!("qr scan failed: {e}");
            }
        }

        // Negative-sample capture finalize.
        if let NegativeRecording::Capturing { started_at } = self.negative_recording {
            if started_at.elapsed().as_secs_f32() >= NEGATIVE_SAMPLE_SECS {
                if let Some(rec) = &self.recorder {
                    let samples = rec.last_seconds(NEGATIVE_SAMPLE_SECS);
                    let feats = extract_features(&samples);
                    if !feats.is_empty() {
                        self.config.wake_negative_templates.push(WakeTemplate {
                            features: feats.data,
                            frames: feats.frames,
                            bins: feats.bins,
                        });
                        let _ = self.config.save();
                        self.flash(format!(
                            "Captured negative sample ({} total).",
                            self.config.wake_negative_templates.len()
                        ));
                    }
                }
                self.negative_recording = NegativeRecording::Idle;
            } else {
                ctx.request_repaint_after(Duration::from_millis(80));
            }
        }

        // Wake-word detection (only when not in cooldown).
        let in_cooldown = self
            .last_wake_event
            .map(|t| t.elapsed() < Duration::from_secs(self.config.wake_cooldown_secs as u64))
            .unwrap_or(false);

        if let (Some(rec), true) = (
            &self.recorder,
            self.config.setup_complete
                && !self.config.wake_templates.is_empty()
                && !in_cooldown
                && matches!(self.negative_recording, NegativeRecording::Idle),
        ) {
            if self.last_wake_check.elapsed() >= WAKE_CHECK_INTERVAL {
                self.last_wake_check = Instant::now();
                let samples = rec.last_seconds(WAKE_WINDOW_SECS);
                if samples.len() as f32 >= WAKE_WINDOW_SECS * 16_000.0 * 0.5 {
                    let detector = WakeDetector::new(
                        &self.config.wake_templates,
                        &self.config.wake_negative_templates,
                        self.config.wake_threshold,
                    );
                    let decision = detector.score(&samples);
                    self.last_wake_score = decision.positive_score;
                    if decision.hit {
                        self.last_wake_event = Some(Instant::now());
                        log::info!(
                            "wake matched (pos={:.2} neg={:.2})",
                            decision.positive_score, decision.negative_score
                        );
                        self.wake_overlay.trigger();
                    }
                }
            }
        }

        self.chat.drain_pending();
        self.chat.drain_ui_commands(ctx);

        ctx.request_repaint_after(Duration::from_millis(120));
    }

    fn agent_context(&self, ctx: &egui::Context) -> AgentContext {
        let clipboard = read_clipboard_text(ctx);
        AgentContext {
            system: self.system_snapshot.clone(),
            qr_codes: self.scanner.codes.clone(),
            autonomy: self.config.autonomy.clone(),
            wake_word: self.config.wake_word_label.clone(),
            clipboard,
            ui_tx: self.chat.ui_tx.clone(),
        }
    }

    fn flash(&mut self, msg: impl Into<String>) {
        self.status_flash = Some((Instant::now(), msg.into()));
    }

    fn refresh_audio_devices(&mut self) {
        self.audio_devices.input = list_input_devices();
        self.audio_devices.output = list_output_devices();
    }

    fn restart_audio(&mut self) {
        let (rec, err) = start_recorder(&self.config);
        self.recorder = rec;
        self.mic_error = err;
    }
}

fn start_recorder(config: &Config) -> (Option<Recorder>, Option<String>) {
    match Recorder::start(WAKE_WINDOW_SECS.max(2.0), config.mic_device.as_deref()) {
        Ok(r) => (Some(r), None),
        Err(e) => {
            log::warn!("microphone unavailable: {e:#}");
            (None, Some(e.to_string()))
        }
    }
}

fn read_clipboard_text(_ctx: &egui::Context) -> Option<String> {
    // egui doesn't expose synchronous clipboard read; the agent loop
    // ignores `None` and returns an empty string for `read_clipboard`.
    // (We could plug an `arboard` instance here later if needed.)
    None
}

impl eframe::App for JarvisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme::SURFACE_0))
            .show(ctx, |ui| {
                if !self.config.setup_complete {
                    self.show_wizard(ui);
                } else {
                    self.show_main(ui);
                }
            });

        // Wake overlay (ephemeral floating chat).
        let agent = self.agent_context(ctx);
        let api_key = self.config.xai_api_key.clone();
        let model = self.config.xai_model.clone();
        let wake_word = self.config.wake_word_label.clone();
        views::wake_overlay::show(
            ctx,
            &mut self.wake_overlay,
            &self.runtime,
            api_key.as_deref(),
            &model,
            agent,
            &wake_word,
        );

        // QR overlay.
        if self.config.setup_complete {
            views::qr_overlay::show(ctx, &self.scanner.codes, self.config.qr_overlay_enabled);
        }
    }
}

impl JarvisApp {
    fn show_wizard(&mut self, ui: &mut egui::Ui) {
        let outcome = wizard::show(ui, &mut self.config, &mut self.wizard, self.recorder.as_ref());
        if let WizardOutcome::Finished = outcome {
            self.tab = Tab::Chat;
            self.chat = ChatState::default();
            self.chat.history = vec![ChatMessage::system(format!(
                "You are Jarvis, the user's on-device assistant. They activated you with the wake word \"{}\". Be concise.",
                self.config.wake_word_label
            ))];
            self.chat.refresh_chat_list();
        }
    }

    fn show_main(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_rect_before_wrap();
        let nav_w = 220.0_f32;

        let nav_rect = egui::Rect::from_min_size(avail.min, egui::vec2(nav_w, avail.height()));
        ui.allocate_ui_at_rect(nav_rect, |ui| self.draw_sidebar(ui));

        let content_rect = egui::Rect::from_min_max(
            egui::pos2(avail.min.x + nav_w, avail.min.y),
            avail.max,
        );
        ui.allocate_ui_at_rect(content_rect, |ui| {
            ui.set_clip_rect(content_rect);
            ui.add_space(20.0);
            ui.scope(|ui| {
                ui.style_mut().spacing.item_spacing.y = 14.0;
                self.draw_content(ui);
            });
        });
    }

    fn draw_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(22.0);

        ui.horizontal(|ui| {
            ui.add_space(20.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
            ui.painter().rect(
                rect,
                theme::rounding(theme::R_FIELD),
                theme::ACCENT,
                egui::Stroke::new(1.0, theme::ACCENT_HOV),
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "J",
                egui::FontId::proportional(16.0),
                theme::BLACK,
            );
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Jarvis")
                        .color(theme::TEXT)
                        .strong()
                        .size(18.0),
                );
                ui.label(
                    egui::RichText::new("On-Hands")
                        .color(theme::TEXT_MUTED)
                        .small(),
                );
            });
        });

        ui.add_space(26.0);

        ui.scope(|ui| {
            ui.style_mut().spacing.item_spacing.y = 4.0;
            for (tab, label) in [
                (Tab::Chat,     "Chat"),
                (Tab::Qr,       "QR Codes"),
                (Tab::System,   "System"),
                (Tab::Settings, "Settings"),
            ] {
                let pad = egui::Frame::none().inner_margin(egui::Margin::symmetric(14.0, 0.0));
                pad.show(ui, |ui| {
                    if theme::pill_tab(ui, label, self.tab == tab).clicked() {
                        self.tab = tab;
                    }
                });
            }
        });

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(20.0);
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(14.0, 0.0))
                .show(ui, |ui| {
                    theme::subcard(ui, |ui| {
                        ui.horizontal(|ui| {
                            let live = self.recorder.is_some();
                            let in_cool = self
                                .last_wake_event
                                .map(|t| t.elapsed() < Duration::from_secs(self.config.wake_cooldown_secs as u64))
                                .unwrap_or(false);
                            let text = if !live {
                                "Mic offline".to_string()
                            } else if in_cool {
                                let remaining = self
                                    .config
                                    .wake_cooldown_secs as u64
                                    - self
                                        .last_wake_event
                                        .map(|t| t.elapsed().as_secs())
                                        .unwrap_or(0);
                                format!("Cooldown · {remaining}s")
                            } else {
                                format!("Listening · {:.0}%", self.last_wake_score * 100.0)
                            };
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            let core_color = if !live {
                                theme::TEXT_DIM
                            } else if in_cool {
                                theme::TEXT_MUTED
                            } else {
                                theme::ACCENT
                            };
                            let painter = ui.painter();
                            if live && !in_cool {
                                let t = ui.ctx().input(|i| i.time) as f32;
                                let phase = (t * 1.6).sin() * 0.5 + 0.5;
                                let halo_a = (40.0 + phase * 80.0) as u8;
                                painter.circle_filled(
                                    rect.center(),
                                    4.0 + phase * 4.0,
                                    egui::Color32::from_rgba_unmultiplied(
                                        theme::ACCENT.r(), theme::ACCENT.g(), theme::ACCENT.b(), halo_a,
                                    ),
                                );
                                ui.ctx().request_repaint_after(Duration::from_millis(60));
                            }
                            painter.circle_filled(rect.center(), 3.5, core_color);
                            ui.label(egui::RichText::new(text).color(theme::TEXT).small());
                        });
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(format!("Wake word: \"{}\"", self.config.wake_word_label))
                                .color(theme::TEXT_MUTED)
                                .small(),
                        );
                        if let Some(dev) = &self.config.mic_device {
                            ui.label(
                                egui::RichText::new(format!("Mic: {}", truncate(dev, 22)))
                                    .color(theme::TEXT_DIM)
                                    .small(),
                            );
                        }
                    });
                });
        });
    }

    fn draw_content(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(self.tab.title())
                    .color(theme::TEXT)
                    .size(24.0)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(20.0);
                if !self.scanner.codes.is_empty() {
                    theme::badge(ui, &format!("{} QR codes", self.scanner.codes.len()), false);
                }
                theme::badge(
                    ui,
                    &format!(
                        "{:.0}% CPU  ·  {} MB",
                        self.system_snapshot.cpu_usage_percent,
                        self.system_snapshot.mem_used_mb
                    ),
                    false,
                );
                if let Some(t) = self.last_wake_event {
                    if t.elapsed() < Duration::from_secs(3) {
                        theme::badge(ui, "Wake!", true);
                    }
                }
            });
        });

        // Transient status flash from settings actions.
        if let Some((t, msg)) = self.status_flash.clone() {
            if t.elapsed() < Duration::from_secs(3) {
                ui.add_space(4.0);
                theme::subcard(ui, |ui| {
                    ui.label(egui::RichText::new(msg).color(theme::ACCENT));
                });
            } else {
                self.status_flash = None;
            }
        }

        ui.add_space(6.0);

        let content_rect = ui.available_rect_before_wrap().shrink2(egui::vec2(20.0, 0.0));
        ui.allocate_ui_at_rect(content_rect, |ui| {
            theme::card(ui, |ui| {
                ui.set_min_height(ui.available_height() - 20.0);
                match self.tab {
                    Tab::Chat => {
                        let agent = self.agent_context(ui.ctx());
                        views::chat::show(
                            ui,
                            &mut self.chat,
                            &self.runtime,
                            self.config.xai_api_key.as_deref(),
                            &self.config.xai_model,
                            agent,
                        );
                    }
                    Tab::Qr => {
                        views::qr_view::show(ui, &self.scanner.codes, self.config.qr_scanning_enabled);
                    }
                    Tab::System => {
                        views::system_view::show(ui, &self.system_snapshot);
                    }
                    Tab::Settings => {
                        let result = views::settings::show(
                            ui, &mut self.config, &self.audio_devices,
                        );
                        if result.refresh_audio_devices {
                            self.refresh_audio_devices();
                            self.flash("Refreshed audio device list.");
                        }
                        if result.mic_changed {
                            self.restart_audio();
                            self.flash(format!(
                                "Switched mic to {}",
                                self.config.mic_device.clone().unwrap_or_else(|| "system default".into())
                            ));
                        }
                        if result.clear_wake_templates {
                            self.config.wake_templates.clear();
                            let _ = self.config.save();
                            self.flash("Cleared positive wake samples.");
                        }
                        if result.clear_negative_templates {
                            self.config.wake_negative_templates.clear();
                            let _ = self.config.save();
                            self.flash("Cleared negative wake samples.");
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
                        if result.record_negative_sample {
                            if self.recorder.is_some() {
                                self.negative_recording = NegativeRecording::Capturing {
                                    started_at: Instant::now(),
                                };
                                self.flash("Capturing 1.6s of background sound — keep it noisy.");
                            } else {
                                self.flash("Mic unavailable — can't capture a sample.");
                            }
                        }
                    }
                }
            });
        });
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
