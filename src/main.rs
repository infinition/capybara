#![windows_subsystem = "windows"]

mod app;
mod audio;
mod core;
mod emulator;
mod gui;
mod hw_bridge;
mod i18n;
mod ui;

use app::TamagotchiApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 720.0])
            .with_min_inner_size([720.0, 560.0])
            .with_title("Tamagotchi Paradise Hardware Emulator (Sonix SNC73410)")
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Tamagotchi Paradise Hardware Emulator",
        native_options,
        Box::new(|cc| Ok(Box::new(TamagotchiApp::new(cc)))),
    )
}
