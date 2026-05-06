// QR codes view: list of recently detected codes with a clickable link
// for each. The detection itself runs in `JarvisApp::tick` via
// `qr::Scanner` so we just render the latest results here.

use crate::qr::ScannedCode;
use crate::theme::{ACCENT, TEXT_MUTED};

pub fn show(ui: &mut egui::Ui, codes: &[ScannedCode], enabled: bool) {
    ui.heading("QR Codes On Screen");
    ui.label(
        egui::RichText::new(if enabled {
            "Continuously scanning all monitors. Click any code to open or copy."
        } else {
            "QR scanning is currently disabled (see Settings)."
        })
        .color(TEXT_MUTED),
    );
    ui.add_space(8.0);

    if codes.is_empty() {
        ui.label(egui::RichText::new("No QR codes detected.").color(TEXT_MUTED));
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for code in codes {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Monitor {}", code.monitor_index))
                                .color(TEXT_MUTED)
                                .small(),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "Quad: ({}, {}) → ({}, {})",
                                code.corners[0].0, code.corners[0].1, code.corners[2].0, code.corners[2].1
                            ))
                            .color(TEXT_MUTED)
                            .small(),
                        );
                    });
                    ui.label(egui::RichText::new(&code.content).color(ACCENT));
                    ui.horizontal(|ui| {
                        if ui.button("Open").clicked() {
                            let _ = crate::automation::apps::open_url(&code.content);
                        }
                        if ui.button("Copy").clicked() {
                            ui.ctx().copy_text(code.content.clone());
                        }
                    });
                });
                ui.add_space(4.0);
            }
        });
}
