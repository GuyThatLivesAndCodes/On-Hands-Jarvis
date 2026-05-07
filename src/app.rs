// Top-level eframe application. Dispatches between the setup wizard and
// the main shell, which is a left side-rail of pill tabs alongside a
// card-based content area.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::ai::{AgentContext, ChatMessage};
use crate::automation::system::{SystemMonitor, SystemSnapshot};
use crate::config::Config;
use crate::qr::Scanner;
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
            tab: Tab::Chat,
            wizard,
            chat: ChatState::default(),
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

        ctx.request_repaint_after(Duration::from_millis(100));
    }
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
        }
    }

    fn show_main(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_rect_before_wrap();
        let nav_w = 220.0_f32;

        // Left side rail.
        let nav_rect = egui::Rect::from_min_size(
            avail.min,
            egui::vec2(nav_w, avail.height()),
        );
        ui.allocate_ui_at_rect(nav_rect, |ui| self.draw_sidebar(ui));

        // Right content area.
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

        // Brand: a small rounded accent square + wordmark, no special glyphs.
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
            ui.painter().rect(
                rect,
                theme::rounding(8.0),
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

        // Tab pills, padded from the rail edge.
        ui.scope(|ui| {
            ui.style_mut().spacing.item_spacing.y = 6.0;
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

        // Bottom-anchored status block.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(20.0);
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(14.0, 0.0))
                .show(ui, |ui| {
                    theme::subcard(ui, |ui| {
                        ui.horizontal(|ui| {
                            let live = self.recorder.is_some();
                            let text = if live {
                                format!("Listening · {:.0}%", self.last_wake_score * 100.0)
                            } else {
                                "Mic offline".to_string()
                            };
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            // Soft halo + solid core. The halo radius
                            // pulses slowly when the mic is live.
                            let painter = ui.painter();
                            let core_color = if live { theme::ACCENT } else { theme::TEXT_DIM };
                            if live {
                                let t = ui.ctx().input(|i| i.time) as f32;
                                let phase = (t * 1.6).sin() * 0.5 + 0.5; // 0..1
                                let halo_r = 4.0 + phase * 4.0;
                                let halo_a = (40.0 + phase * 80.0) as u8;
                                painter.circle_filled(
                                    rect.center(),
                                    halo_r,
                                    egui::Color32::from_rgba_unmultiplied(
                                        theme::ACCENT.r(),
                                        theme::ACCENT.g(),
                                        theme::ACCENT.b(),
                                        halo_a,
                                    ),
                                );
                                ui.ctx().request_repaint_after(std::time::Duration::from_millis(60));
                            }
                            painter.circle_filled(rect.center(), 3.5, core_color);
                            ui.label(egui::RichText::new(text).color(theme::TEXT).small());
                        });
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "Wake word: \"{}\"",
                                self.config.wake_word_label
                            ))
                            .color(theme::TEXT_MUTED)
                            .small(),
                        );
                    });
                });
        });
    }

    fn draw_content(&mut self, ui: &mut egui::Ui) {
        // Top app bar with title + status badges.
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
                    theme::badge(
                        ui,
                        &format!("{} QR codes", self.scanner.codes.len()),
                        false,
                    );
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

        ui.add_space(6.0);

        // The card holding the active tab's content fills the remaining space.
        let content_rect = ui.available_rect_before_wrap().shrink2(egui::vec2(20.0, 0.0));
        ui.allocate_ui_at_rect(content_rect, |ui| {
            theme::card(ui, |ui| {
                ui.set_min_height(ui.available_height() - 20.0);
                match self.tab {
                    Tab::Chat => {
                        let agent = AgentContext {
                            system: self.system_snapshot.clone(),
                            qr_codes: self.scanner.codes.clone(),
                            autonomy: self.config.autonomy.clone(),
                            wake_word: self.config.wake_word_label.clone(),
                        };
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
            });
        });
    }
}
