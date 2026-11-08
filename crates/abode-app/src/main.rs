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
    // Debug aid: ABODE_EXPORT_DOCS=<folder> [ABODE_EXPORT_PRIVATE=1] exports the
    // file (or --sample) as documents into that folder and exits.
    if let Ok(parent) = std::env::var("ABODE_EXPORT_DOCS") {
        let doc = match initial.as_deref() {
            Some(p) if p != std::path::Path::new("--sample") => document::Document::open(p),
            _ => document::Document::new_with(abode_core::sample::sample_house()),
        };
        let everything = std::env::var("ABODE_EXPORT_PRIVATE").is_ok();
        match doc.and_then(|mut d| d.export_documents(std::path::Path::new(&parent), everything, "debug")) {
            Ok(dir) => println!("{}", dir.display()),
            Err(e) => eprintln!("Couldn't export: {e:#}"),
        }
        return Ok(());
    }
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon/icon.png")).ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Abode")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 600.0])
        .with_icon(icon.unwrap_or_default());
    // Linux desktops match the window to abode.desktop by this id, for the
    // dock/taskbar icon (Wayland ignores the window icon). Only set on Linux:
    // eframe also names its settings folder after it.
    if cfg!(target_os = "linux") {
        viewport = viewport.with_app_id("abode");
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "Abode",
        options,
        Box::new(move |cc| Ok(Box::new(app::AbodeApp::new(cc, initial)))),
    )
}
