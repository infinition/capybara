#![windows_subsystem = "windows"]

mod app;
mod audio;
mod core;
mod emulator;
mod gui;
mod hw_bridge;
mod i18n;
mod ui;
mod web;

use app::TamagotchiApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 600.0])
            .with_min_inner_size([300.0, 380.0])
            .with_title("Tamagotchi Paradise")
            // La transparence se demande a la creation : sur la plupart des
            // systemes elle ne s'allume pas apres coup. Elle ne se voit qu'en
            // mode jeu, ou le fond n'est pas peint.
            .with_transparent(true)
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Tamagotchi Paradise Hardware Emulator",
        native_options,
        Box::new(|cc| Ok(Box::new(TamagotchiApp::new(cc)))),
    )
}
