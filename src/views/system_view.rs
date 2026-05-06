// System information view: live CPU/memory snapshot + top processes.

use crate::automation::system::SystemSnapshot;
use crate::theme::{ACCENT, TEXT_MUTED};

pub fn show(ui: &mut egui::Ui, snap: &SystemSnapshot) {
    ui.heading("System");
    ui.label(
        egui::RichText::new(format!("{} — {} ({})", snap.host, snap.os, snap.kernel))
            .color(TEXT_MUTED),
    );
    ui.add_space(8.0);

    egui::Grid::new("sys_grid").num_columns(2).spacing([20.0, 6.0]).show(ui, |ui| {
        ui.label("CPU usage");
        ui.label(egui::RichText::new(format!("{:.1}% across {} cores", snap.cpu_usage_percent, snap.cpu_count)).color(ACCENT));
        ui.end_row();

        ui.label("Memory");
        ui.label(egui::RichText::new(format!(
            "{} / {} MB",
            snap.mem_used_mb, snap.mem_total_mb
        )).color(ACCENT));
        ui.end_row();

        ui.label("Load avg (1m)");
        ui.label(format!("{:.2}", snap.load_avg_one));
        ui.end_row();

        ui.label("Uptime");
        ui.label(format!("{} s", snap.uptime_secs));
        ui.end_row();
    });

    ui.add_space(12.0);
    ui.label(egui::RichText::new("Top processes (by CPU)").strong());
    ui.separator();

    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        egui::Grid::new("proc_grid")
            .num_columns(4)
            .striped(true)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("PID").color(TEXT_MUTED));
                ui.label(egui::RichText::new("Name").color(TEXT_MUTED));
                ui.label(egui::RichText::new("CPU%").color(TEXT_MUTED));
                ui.label(egui::RichText::new("Mem MB").color(TEXT_MUTED));
                ui.end_row();
                for p in &snap.top_processes {
                    ui.label(p.pid.to_string());
                    ui.label(&p.name);
                    ui.label(format!("{:.1}", p.cpu));
                    ui.label(p.mem_mb.to_string());
                    ui.end_row();
                }
            });
    });
}
