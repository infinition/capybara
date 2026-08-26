#![windows_subsystem = "windows"]

mod app;
mod audio;
mod core;
mod gui;
mod hw_bridge;
mod i18n;

use app::TamagotchiApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 720.0])
            .with_min_inner_size([400.0, 600.0])
            .with_title("Tamagotchi Paradise Desktop")
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Tamagotchi Paradise Desktop",
        native_options,
        Box::new(|cc| Ok(Box::new(TamagotchiApp::new(cc)))),
    )
}
