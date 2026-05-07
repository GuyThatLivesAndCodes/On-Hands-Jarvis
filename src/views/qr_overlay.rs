// Always-on-top, transparent, borderless overlay window that draws an
// outline around every detected QR code on the user's primary monitor
// and floats Open / Copy buttons just below each. Closes itself when
// no codes are detected.

use crate::qr::ScannedCode;
use crate::theme;

/// Render the overlay if `codes` is non-empty. Caller controls the
/// `enabled` flag (Settings → QR scanning).
pub fn show(ctx: &egui::Context, codes: &[ScannedCode], enabled: bool) {
    if !enabled || codes.is_empty() {
        return;
    }

    // We position the overlay over the primary monitor only — getting
    // multi-monitor right requires per-monitor viewports and platform
    // quirks aren't worth it for a v1.
    let Some(primary) = primary_monitor() else {
        return;
    };
    let codes: Vec<ScannedCode> = codes
        .iter()
        .filter(|c| c.monitor_index == primary.index)
        .cloned()
        .collect();
    if codes.is_empty() {
        return;
    }

    let viewport_id = egui::ViewportId::from_hash_of("qr-overlay");
    let viewport = egui::ViewportBuilder::default()
        .with_title("Jarvis QR overlay")
        .with_inner_size([primary.w as f32, primary.h as f32])
        .with_position([primary.x as f32, primary.y as f32])
        .with_decorations(false)
        .with_always_on_top()
        .with_resizable(false)
        .with_transparent(true)
        .with_taskbar(false)
        .with_mouse_passthrough(false);

    ctx.show_viewport_immediate(viewport_id, viewport, |ctx, _class| {
        // Esc dismisses by hiding the viewport on next frame; we don't
        // own the open/closed state, so we just let the user close it
        // and the parent app will re-open it next tick when codes are
        // still present. (User can also disable via Settings.)
        let close = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let painter = ui.painter();
                for code in &codes {
                    let (min, max) = bounding_box(&code.corners);
                    let rect = egui::Rect::from_min_max(
                        egui::pos2(min.0 as f32, min.1 as f32),
                        egui::pos2(max.0 as f32, max.1 as f32),
                    );
                    // Outline + corner ticks.
                    painter.rect_stroke(
                        rect.expand(4.0),
                        theme::rounding(theme::R_FIELD),
                        egui::Stroke::new(2.5, theme::ACCENT),
                    );
                    let tick = 12.0;
                    let r = rect.expand(4.0);
                    for (a, b) in [
                        (r.left_top(), r.left_top() + egui::vec2(tick, 0.0)),
                        (r.left_top(), r.left_top() + egui::vec2(0.0, tick)),
                        (r.right_top(), r.right_top() - egui::vec2(tick, 0.0)),
                        (r.right_top(), r.right_top() + egui::vec2(0.0, tick)),
                        (r.left_bottom(), r.left_bottom() + egui::vec2(tick, 0.0)),
                        (r.left_bottom(), r.left_bottom() - egui::vec2(0.0, tick)),
                        (r.right_bottom(), r.right_bottom() - egui::vec2(tick, 0.0)),
                        (r.right_bottom(), r.right_bottom() - egui::vec2(0.0, tick)),
                    ] {
                        painter.line_segment([a, b], egui::Stroke::new(3.0, theme::ACCENT));
                    }
                }

                // Action chips beneath each code.
                for code in &codes {
                    let (_, max) = bounding_box(&code.corners);
                    let chip_pos = egui::pos2(max.0 as f32, max.1 as f32 + 8.0);
                    actions_chip(ui, chip_pos, code);
                }

                // A subtle "Esc to dismiss" hint at the top center.
                let center_top = egui::pos2(ui.max_rect().center().x, 14.0);
                ui.painter().text(
                    center_top,
                    egui::Align2::CENTER_TOP,
                    "QR overlay  ·  press Esc to hide",
                    egui::FontId::proportional(12.0),
                    theme::TEXT_MUTED,
                );
            });

        if close || ctx.input(|i| i.viewport().close_requested()) {
            // We can't reach back to the app to flip a flag from here,
            // so we just paint nothing this frame and let the parent
            // tick decide whether to open us again.
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
}

fn actions_chip(ui: &mut egui::Ui, pos: egui::Pos2, code: &ScannedCode) {
    let chip_w = 220.0_f32;
    let chip_h = 64.0;
    let rect = egui::Rect::from_min_size(
        egui::pos2(pos.x - chip_w, pos.y),
        egui::vec2(chip_w, chip_h),
    );
    // Don't render off-screen chips below the viewport.
    let viewport = ui.max_rect();
    let rect = if rect.bottom() > viewport.bottom() {
        // Flip above the QR.
        let above = egui::Rect::from_min_size(
            egui::pos2(pos.x - chip_w, pos.y - chip_h - 16.0),
            egui::vec2(chip_w, chip_h),
        );
        above
    } else {
        rect
    };

    let painter = ui.painter();
    painter.rect(
        rect,
        theme::rounding(theme::R_FIELD),
        theme::SURFACE_1,
        egui::Stroke::new(1.0, theme::BORDER_STR),
    );

    // Truncate URL into the chip.
    let preview: String = code.content.chars().take(32).collect();
    painter.text(
        rect.left_top() + egui::vec2(10.0, 6.0),
        egui::Align2::LEFT_TOP,
        preview,
        egui::FontId::proportional(12.0),
        theme::TEXT,
    );

    // Open / Copy buttons inside the chip.
    let btn_y = rect.bottom() - 26.0;
    let open_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 10.0, btn_y),
        egui::vec2(80.0, 22.0),
    );
    let copy_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 100.0, btn_y),
        egui::vec2(80.0, 22.0),
    );
    let id = ui.id().with(("qr-chip", &code.content, code.monitor_index));
    let open_resp = ui.interact(open_rect, id.with("open"), egui::Sense::click());
    let copy_resp = ui.interact(copy_rect, id.with("copy"), egui::Sense::click());

    paint_btn(ui, open_rect, "Open", open_resp.hovered(), true);
    paint_btn(ui, copy_rect, "Copy", copy_resp.hovered(), false);

    if open_resp.clicked() {
        let _ = crate::automation::apps::open_url(&code.content);
    }
    if copy_resp.clicked() {
        ui.ctx().copy_text(code.content.clone());
    }
}

fn paint_btn(ui: &egui::Ui, rect: egui::Rect, label: &str, hovered: bool, primary: bool) {
    let painter = ui.painter();
    let bg = if hovered { theme::SURFACE_HOV } else { theme::SURFACE_2 };
    let stroke = if hovered { theme::TEXT } else { theme::BORDER_STR };
    painter.rect(rect, theme::rounding(theme::R_FIELD), bg, egui::Stroke::new(1.0, stroke));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(12.5),
        theme::TEXT,
    );
    if primary {
        let underline_y = rect.bottom() - 4.0;
        let underline = egui::Rect::from_min_max(
            egui::pos2(rect.center().x - 14.0, underline_y),
            egui::pos2(rect.center().x + 14.0, underline_y + 1.5),
        );
        painter.rect_filled(underline, theme::rounding(1.0), theme::ACCENT);
    }
}

#[derive(Debug, Clone, Copy)]
struct PrimaryMonitor {
    index: usize,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

fn primary_monitor() -> Option<PrimaryMonitor> {
    let mons = xcap::Monitor::all().ok()?;
    let (idx, m) = mons
        .iter()
        .enumerate()
        .find(|(_, m)| m.is_primary())
        .or_else(|| mons.iter().enumerate().next())?;
    Some(PrimaryMonitor {
        index: idx,
        x: m.x(),
        y: m.y(),
        w: m.width(),
        h: m.height(),
    })
}

fn bounding_box(corners: &[(i32, i32); 4]) -> ((i32, i32), (i32, i32)) {
    let xs = corners.iter().map(|p| p.0);
    let ys = corners.iter().map(|p| p.1);
    let min_x = xs.clone().min().unwrap_or(0);
    let max_x = xs.max().unwrap_or(0);
    let min_y = ys.clone().min().unwrap_or(0);
    let max_y = ys.max().unwrap_or(0);
    ((min_x, min_y), (max_x, max_y))
}
