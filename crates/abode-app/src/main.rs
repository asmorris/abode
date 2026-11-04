#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod document;
mod textures;
mod theme;
mod ui;
mod updater;

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // macOS used to pass `-psn_…` when launched from Finder; ignore flags like that.
    let initial = std::env::args_os()
        .skip(1)
        .find(|a| !a.to_string_lossy().starts_with("-psn"))
        .map(PathBuf::from);
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon/icon.png")).ok();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Abode")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0])
            .with_icon(icon.unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "Abode",
        options,
        Box::new(move |cc| Ok(Box::new(app::AbodeApp::new(cc, initial)))),
    )
}
