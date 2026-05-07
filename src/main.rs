// On-Hands Jarvis: a voice-activated desktop AI assistant.
//
// Entry point. Initializes logging, builds an async runtime for the AI
// client, and hands control over to the eframe GUI.

#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]
// The automation toolset and several response/feature helpers are part of
// the public surface for future autonomous-control work but are not all
// wired into the UI yet. Allow dead code here so the surface compiles
// clean under `-Dwarnings` in CI.
#![allow(dead_code)]

mod ai;
mod app;
mod automation;
mod chat_store;
mod config;
mod qr;
mod theme;
mod views;
mod voice;
mod wizard;

use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let runtime = Arc::new(runtime);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("On-Hands Jarvis")
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([720.0, 480.0]),
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "On-Hands Jarvis",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::JarvisApp::new(cc, runtime.clone())))),
    )
    .map_err(|e| anyhow::anyhow!("eframe failed: {e}"))
}
