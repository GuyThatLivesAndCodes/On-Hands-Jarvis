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
//
// We deliberately stay close to right angles — small radii give the UI a
// crisp, instrument-panel feel rather than a "soft glassmorphism" look.

pub const R_CARD:   f32 = 4.0;
pub const R_PILL:   f32 = 4.0;
pub const R_FIELD:  f32 = 3.0;

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

    // Accent is rationed: text selections + hyperlinks. Selection backgrounds
    // stay grey so the UI doesn't look "tinted".
    v.selection.bg_fill = SURFACE_HOV;
    v.selection.stroke  = Stroke::new(1.0, BORDER_STR);
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

    // "Active" (pressed) widgets stay monochrome with a brighter border
    // so we don't paint big swaths of blue on every click.
    v.widgets.active.bg_fill      = SURFACE_HOV;
    v.widgets.active.weak_bg_fill = SURFACE_HOV;
    v.widgets.active.bg_stroke    = Stroke::new(1.5, TEXT);
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

/// Sidebar tab row. Selected = filled grey background + a bright accent
/// stripe on the left edge (the only place blue appears). Not selected =
/// transparent until hover.
pub fn pill_tab(ui: &mut egui::Ui, label: &str, selected: bool) -> Response {
    let desired = egui::vec2(ui.available_width(), 36.0);
    let (rect, resp) = ui.allocate_at_least(desired, Sense::click());

    let bg = if selected {
        SURFACE_HOV
    } else if resp.hovered() {
        SURFACE_2
    } else {
        Color32::TRANSPARENT
    };
    let fg = if selected || resp.hovered() { TEXT } else { TEXT_MUTED };

    let painter = ui.painter();
    painter.rect(rect, rounding(R_PILL), bg, Stroke::NONE);

    if selected {
        // Vertical accent stripe — the one bit of color in the rail.
        let stripe = egui::Rect::from_min_size(
            rect.left_top() + egui::vec2(0.0, 4.0),
            egui::vec2(3.0, rect.height() - 8.0),
        );
        painter.rect_filled(stripe, rounding(2.0), ACCENT);
    }

    let label_pos = egui::pos2(rect.left() + 16.0, rect.center().y);
    painter.text(
        label_pos,
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        fg,
    );
    resp
}

/// Primary CTA. Monochrome by default — the only bit of color is a thin
/// accent underline beneath the label, so blue is reserved for "this is
/// the next thing to click", not "this is a button".
pub fn primary_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> Response {
    let desired = egui::vec2(140.0, 36.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let interact = resp.hovered() && enabled;
    let bg = if !enabled {
        SURFACE_1
    } else if interact {
        SURFACE_HOV
    } else {
        SURFACE_2
    };
    let border = if !enabled {
        BORDER
    } else if interact {
        TEXT
    } else {
        BORDER_STR
    };
    let fg = if enabled { TEXT } else { TEXT_DIM };

    let painter = ui.painter();
    painter.rect(rect, rounding(R_PILL), bg, Stroke::new(1.0, border));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        fg,
    );
    if enabled {
        let underline_y = rect.bottom() - 6.0;
        let underline = egui::Rect::from_min_max(
            egui::pos2(rect.center().x - 18.0, underline_y),
            egui::pos2(rect.center().x + 18.0, underline_y + 1.5),
        );
        painter.rect_filled(underline, rounding(1.0), ACCENT);
    }
    if !enabled {
        return resp.on_hover_text("disabled");
    }
    resp
}

/// Secondary monochrome button. Border-only.
pub fn ghost_button(ui: &mut egui::Ui, label: &str) -> Response {
    let desired = egui::vec2(96.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let bg = if resp.hovered() { SURFACE_2 } else { Color32::TRANSPARENT };
    let stroke = Stroke::new(1.0, if resp.hovered() { BORDER_STR } else { BORDER });
    let painter = ui.painter();
    painter.rect(rect, rounding(R_PILL), bg, stroke);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.5),
        if resp.hovered() { TEXT } else { TEXT_MUTED },
    );
    resp
}

/// Compact inline button used in toolbars / chat headers / close
/// buttons. Square corners, no underline.
pub fn icon_button(ui: &mut egui::Ui, label: &str) -> Response {
    let desired = egui::vec2(32.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let bg = if resp.hovered() { SURFACE_2 } else { Color32::TRANSPARENT };
    let painter = ui.painter();
    painter.rect(rect, rounding(R_FIELD), bg, Stroke::new(1.0, if resp.hovered() { BORDER_STR } else { BORDER }));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.0),
        if resp.hovered() { TEXT } else { TEXT_MUTED },
    );
    resp
}

/// Square-cornered status badge. Border-only by default; `accent`
/// switches the leading dot + label to the accent color but keeps the
/// background grey.
pub fn badge(ui: &mut egui::Ui, label: &str, accent: bool) {
    egui::Frame::none()
        .fill(SURFACE_2)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(rounding(R_PILL))
        .inner_margin(Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                ui.painter().circle_filled(
                    rect.center(),
                    3.0,
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
