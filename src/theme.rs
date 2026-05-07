// Visual theme: monochrome (black → white) with a single light-blue
// accent. Surfaces are flat solids with rounded corners; widgets opt into
// the helpers below to stay visually consistent.

use egui::{Color32, FontId, Margin, Response, RichText, Rounding, Sense, Stroke, Visuals};

// -- palette ----------------------------------------------------------------

pub const BLACK:       Color32 = Color32::from_rgb(0, 0, 0);
pub const SURFACE_0:   Color32 = Color32::from_rgb(8, 8, 10);     // page background
pub const SURFACE_1:   Color32 = Color32::from_rgb(24, 24, 28);   // cards (~9% gray)
pub const SURFACE_2:   Color32 = Color32::from_rgb(40, 40, 46);   // inputs / nested
pub const SURFACE_HOV: Color32 = Color32::from_rgb(56, 56, 64);
pub const BORDER:      Color32 = Color32::from_rgb(56, 56, 66);
pub const BORDER_STR:  Color32 = Color32::from_rgb(96, 96, 110);

pub const TEXT:        Color32 = Color32::from_rgb(248, 248, 250);
pub const TEXT_MUTED:  Color32 = Color32::from_rgb(168, 172, 184);
pub const TEXT_DIM:    Color32 = Color32::from_rgb(112, 116, 128);

pub const ACCENT:      Color32 = Color32::from_rgb(124, 196, 255); // light blue
pub const ACCENT_HOV:  Color32 = Color32::from_rgb(170, 218, 255);
pub const ACCENT_DEEP: Color32 = Color32::from_rgb(36, 92, 156);

// -- shape constants --------------------------------------------------------

pub const R_CARD:   f32 = 16.0;
pub const R_PILL:   f32 = 12.0;
pub const R_FIELD:  f32 = 10.0;

pub fn rounding(r: f32) -> Rounding { Rounding::same(r) }

// -- global style -----------------------------------------------------------

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.panel_fill = SURFACE_0;
    v.window_fill = SURFACE_1;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_rounding = rounding(R_CARD);
    v.menu_rounding = rounding(R_FIELD);
    v.extreme_bg_color = SURFACE_2;
    v.faint_bg_color   = SURFACE_1;

    v.selection.bg_fill = ACCENT_DEEP;
    v.selection.stroke  = Stroke::new(1.0, ACCENT);
    v.hyperlink_color   = ACCENT;

    let pill = rounding(R_PILL);

    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
    v.widgets.noninteractive.rounding  = pill;

    v.widgets.inactive.bg_fill      = SURFACE_2;
    v.widgets.inactive.weak_bg_fill = SURFACE_1;
    v.widgets.inactive.bg_stroke    = Stroke::new(1.0, BORDER);
    v.widgets.inactive.fg_stroke    = Stroke::new(1.0, TEXT);
    v.widgets.inactive.rounding     = pill;

    v.widgets.hovered.bg_fill      = SURFACE_HOV;
    v.widgets.hovered.weak_bg_fill = SURFACE_HOV;
    v.widgets.hovered.bg_stroke    = Stroke::new(1.0, BORDER_STR);
    v.widgets.hovered.fg_stroke    = Stroke::new(1.0, TEXT);
    v.widgets.hovered.rounding     = pill;

    v.widgets.active.bg_fill      = ACCENT_DEEP;
    v.widgets.active.weak_bg_fill = ACCENT_DEEP;
    v.widgets.active.bg_stroke    = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke    = Stroke::new(1.0, TEXT);
    v.widgets.active.rounding     = pill;

    v.widgets.open.bg_fill   = SURFACE_2;
    v.widgets.open.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.open.rounding  = pill;

    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing      = egui::vec2(10.0, 10.0);
    style.spacing.button_padding    = egui::vec2(16.0, 9.0);
    style.spacing.window_margin     = Margin::same(0.0);
    style.spacing.menu_margin       = Margin::same(8.0);
    style.spacing.interact_size.y   = 32.0;
    style.spacing.scroll.bar_width  = 10.0;

    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(22.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(14.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        FontId::new(12.0, egui::FontFamily::Proportional),
    );

    ctx.set_style(style);
}

// -- composable surfaces ----------------------------------------------------

/// A flat rounded card with a hairline border. Use as the primary
/// container for grouped content.
pub fn card<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(SURFACE_1)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(rounding(R_CARD))
        .inner_margin(Margin::same(18.0))
        .show(ui, content)
        .inner
}

/// A subtler nested surface (e.g. for input rows or muted callouts).
pub fn subcard<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(SURFACE_2)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(rounding(R_FIELD))
        .inner_margin(Margin::symmetric(14.0, 10.0))
        .show(ui, content)
        .inner
}

/// Heading row: large title with an optional muted subtitle below.
pub fn section_header(ui: &mut egui::Ui, title: &str, subtitle: Option<&str>) {
    ui.label(RichText::new(title).heading().color(TEXT).strong());
    if let Some(s) = subtitle {
        ui.label(RichText::new(s).color(TEXT_MUTED));
    }
    ui.add_space(10.0);
}

/// Pill-shaped tab row item used in the sidebar / topbar. A small
/// rounded indicator on the left flips on when the tab is selected.
pub fn pill_tab(ui: &mut egui::Ui, label: &str, selected: bool) -> Response {
    let desired = egui::vec2(ui.available_width(), 40.0);
    let (rect, resp) = ui.allocate_at_least(desired, Sense::click());

    let bg = if selected {
        ACCENT_DEEP
    } else if resp.hovered() {
        SURFACE_HOV
    } else {
        Color32::TRANSPARENT
    };
    let fg = if selected { TEXT } else if resp.hovered() { TEXT } else { TEXT_MUTED };
    let stroke = if selected { Stroke::new(1.0, ACCENT) } else { Stroke::NONE };

    let painter = ui.painter();
    painter.rect(rect, rounding(R_PILL), bg, stroke);

    // Left-edge indicator dot.
    let dot_x = rect.left() + 14.0;
    let dot_r = 4.0;
    let dot_color = if selected { ACCENT } else if resp.hovered() { TEXT_MUTED } else { TEXT_DIM };
    painter.circle_filled(egui::pos2(dot_x, rect.center().y), dot_r, dot_color);

    let label_pos = egui::pos2(rect.left() + 30.0, rect.center().y);
    painter.text(
        label_pos,
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.5),
        fg,
    );
    resp
}

/// Big primary CTA button — accent-filled when enabled, monochrome
/// otherwise. Returns a `Response` so callers can branch on `clicked`.
pub fn primary_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> Response {
    let text = RichText::new(label)
        .color(if enabled { BLACK } else { TEXT_MUTED })
        .strong();
    let mut btn = egui::Button::new(text)
        .min_size(egui::vec2(160.0, 40.0))
        .rounding(rounding(R_PILL));
    if enabled {
        btn = btn.fill(ACCENT).stroke(Stroke::new(1.0, ACCENT_HOV));
    } else {
        btn = btn.fill(SURFACE_2).stroke(Stroke::new(1.0, BORDER));
    }
    ui.add_enabled(enabled, btn)
}

/// Secondary monochrome button.
pub fn ghost_button(ui: &mut egui::Ui, label: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(TEXT))
            .min_size(egui::vec2(96.0, 36.0))
            .fill(SURFACE_2)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(rounding(R_PILL)),
    )
}

/// Static rounded badge / status pill. A small leading dot replaces the
/// previous unicode glyph so we don't depend on extra fonts.
pub fn badge(ui: &mut egui::Ui, label: &str, accent: bool) {
    egui::Frame::none()
        .fill(if accent { ACCENT_DEEP } else { SURFACE_2 })
        .stroke(Stroke::new(
            1.0,
            if accent { ACCENT } else { BORDER },
        ))
        .rounding(rounding(999.0))
        .inner_margin(Margin::symmetric(12.0, 5.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                ui.painter().circle_filled(
                    rect.center(),
                    3.5,
                    if accent { ACCENT } else { TEXT_MUTED },
                );
                ui.label(
                    RichText::new(label)
                        .color(if accent { TEXT } else { TEXT_MUTED })
                        .small(),
                );
            });
        });
}

/// Step indicator: filled dots for completed steps, accent ring for the
/// current one, hollow for pending.
pub fn step_dots(ui: &mut egui::Ui, total: usize, current: usize) {
    ui.horizontal(|ui| {
        for i in 0..total {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
            let painter = ui.painter();
            let ring = rounding(999.0);
            if i < current {
                painter.rect_filled(rect, ring, ACCENT);
            } else if i == current {
                painter.rect(rect, ring, BLACK, Stroke::new(2.0, ACCENT));
            } else {
                painter.rect(rect, ring, BLACK, Stroke::new(1.0, BORDER_STR));
            }
            ui.add_space(2.0);
        }
    });
}
