// Visual theme: stark black-to-white gradient with a light-blue accent.
// Applied once during app startup via `apply` and exposed as constants for
// places that paint directly (gradients, panel backgrounds, etc.).

use egui::{Color32, Stroke, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(140, 200, 255); // light blue
pub const ACCENT_DIM: Color32 = Color32::from_rgb(80, 130, 180);
pub const BG_TOP: Color32 = Color32::from_rgb(8, 8, 10);
pub const BG_BOTTOM: Color32 = Color32::from_rgb(245, 247, 250);
pub const PANEL: Color32 = Color32::from_rgba_premultiplied(18, 18, 22, 235);
pub const TEXT: Color32 = Color32::from_rgb(235, 240, 245);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(150, 156, 165);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = PANEL;
    v.extreme_bg_color = Color32::from_rgb(12, 12, 15);
    v.faint_bg_color = Color32::from_rgb(22, 22, 26);
    v.selection.bg_fill = ACCENT_DIM;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(40, 42, 48));
    v.widgets.inactive.bg_fill = Color32::from_rgb(28, 30, 34);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(22, 24, 28);
    v.widgets.hovered.bg_fill = Color32::from_rgb(40, 44, 52);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
    v.widgets.active.bg_fill = ACCENT_DIM;
    v.widgets.active.bg_stroke = Stroke::new(1.5, ACCENT);
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(12.0);
    ctx.set_style(style);
}

/// Paint a vertical black-to-white gradient as a background for the given rect.
pub fn paint_gradient(ui: &mut egui::Ui, rect: egui::Rect) {
    // Cheap gradient: stack thin horizontal bars interpolating BG_TOP -> BG_BOTTOM.
    let painter = ui.painter_at(rect);
    let bands = 64usize;
    let h = rect.height() / bands as f32;
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        let c = lerp_color(BG_TOP, BG_BOTTOM, t);
        let band = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.min.y + i as f32 * h),
            egui::vec2(rect.width(), h + 1.0),
        );
        painter.rect_filled(band, 0.0, c);
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp = |x: u8, y: u8| -> u8 {
        (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}
