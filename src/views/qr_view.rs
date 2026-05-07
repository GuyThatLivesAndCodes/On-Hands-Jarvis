// QR codes view: each detected code rendered as its own card with an
// Open / Copy action row.

use crate::qr::ScannedCode;
use crate::theme;

pub fn show(ui: &mut egui::Ui, codes: &[ScannedCode], enabled: bool) {
    theme::section_header(
        ui,
        "QR Codes On Screen",
        Some(if enabled {
            "Continuously scanning all monitors. Click any code to open or copy."
        } else {
            "QR scanning is currently disabled — toggle it back on in Settings."
        }),
    );

    if codes.is_empty() {
        empty_state(ui, enabled);
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for code in codes {
                code_card(ui, code);
                ui.add_space(8.0);
            }
        });
}

fn empty_state(ui: &mut egui::Ui, enabled: bool) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        // A simple rounded outline placeholder instead of a missing-glyph box.
        let (rect, _) = ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::hover());
        ui.painter().rect(
            rect,
            theme::rounding(12.0),
            theme::SURFACE_2,
            egui::Stroke::new(1.5, theme::BORDER_STR),
        );
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(if enabled {
                "No QR codes detected on screen."
            } else {
                "Scanner is off."
            })
            .color(theme::TEXT_MUTED),
        );
    });
}

fn code_card(ui: &mut egui::Ui, code: &ScannedCode) {
    theme::subcard(ui, |ui| {
        ui.horizontal(|ui| {
            theme::badge(ui, &format!("Monitor {}", code.monitor_index), false);
            theme::badge(
                ui,
                &format!(
                    "({}, {})  to  ({}, {})",
                    code.corners[0].0, code.corners[0].1, code.corners[2].0, code.corners[2].1
                ),
                false,
            );
        });
        ui.add_space(8.0);
        ui.label(egui::RichText::new(&code.content).color(theme::ACCENT).strong());
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if theme::primary_button(ui, "Open", true).clicked() {
                let _ = crate::automation::apps::open_url(&code.content);
            }
            if theme::ghost_button(ui, "Copy").clicked() {
                ui.ctx().copy_text(code.content.clone());
            }
        });
    });
}
