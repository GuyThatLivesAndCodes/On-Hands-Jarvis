// System view: live host info as a row of stat cards followed by the
// top-process table.

use crate::automation::system::SystemSnapshot;
use crate::theme;

pub fn show(ui: &mut egui::Ui, snap: &SystemSnapshot) {
    theme::section_header(
        ui,
        "System",
        Some(&format!("{}  ·  {}  ·  {}", snap.host, snap.os, snap.kernel)),
    );

    ui.horizontal(|ui| {
        let stat_w = (ui.available_width() / 4.0) - 10.0;
        stat(
            ui,
            stat_w,
            "CPU",
            &format!("{:.1}%", snap.cpu_usage_percent),
            &format!("across {} cores", snap.cpu_count),
        );
        stat(
            ui,
            stat_w,
            "Memory",
            &format!("{} MB", snap.mem_used_mb),
            &format!("of {} MB", snap.mem_total_mb),
        );
        stat(
            ui,
            stat_w,
            "Load",
            &format!("{:.2}", snap.load_avg_one),
            "1-minute average",
        );
        stat(
            ui,
            stat_w,
            "Uptime",
            &format_uptime(snap.uptime_secs),
            "since boot",
        );
    });

    ui.add_space(14.0);
    ui.label(
        egui::RichText::new("Top processes")
            .color(theme::TEXT)
            .strong()
            .size(15.0),
    );
    ui.add_space(6.0);

    theme::subcard(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("proc_grid")
                    .num_columns(4)
                    .striped(true)
                    .spacing([18.0, 6.0])
                    .show(ui, |ui| {
                        for (h, _) in [("PID", 60.0), ("Name", 220.0), ("CPU%", 70.0), ("Mem MB", 80.0)] {
                            ui.label(
                                egui::RichText::new(h)
                                    .color(theme::TEXT_MUTED)
                                    .small()
                                    .strong(),
                            );
                        }
                        ui.end_row();
                        for p in &snap.top_processes {
                            ui.label(egui::RichText::new(p.pid.to_string()).color(theme::TEXT_MUTED));
                            ui.label(egui::RichText::new(&p.name).color(theme::TEXT));
                            ui.label(
                                egui::RichText::new(format!("{:.1}", p.cpu)).color(theme::ACCENT),
                            );
                            ui.label(egui::RichText::new(p.mem_mb.to_string()).color(theme::TEXT));
                            ui.end_row();
                        }
                    });
            });
    });
}

fn stat(ui: &mut egui::Ui, width: f32, label: &str, value: &str, sub: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 92.0), egui::Sense::hover());
    ui.allocate_ui_at_rect(rect, |ui| {
        egui::Frame::none()
            .fill(theme::SURFACE_2)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .rounding(theme::rounding(theme::R_CARD))
            .inner_margin(egui::Margin::symmetric(16.0, 12.0))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(label)
                        .color(theme::TEXT_MUTED)
                        .small()
                        .strong(),
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(value)
                        .color(theme::ACCENT)
                        .strong()
                        .size(22.0),
                );
                ui.label(egui::RichText::new(sub).color(theme::TEXT_DIM).small());
            });
    });
}

fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 { format!("{h}h {m}m") } else { format!("{m}m") }
}
