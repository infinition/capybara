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

/// L'icone de la fenetre et de la barre des taches, cuite dans l'executable.
///
/// Elle est embarquee plutot que lue a cote : un executable deplace garde son
/// icone, et il n'y a pas de chemin a faire suivre. Un decodage qui echoue
/// laisse l'icone par defaut plutot que d'empecher le demarrage.
pub fn icone() -> Option<egui::IconData> {
    let image = image::load_from_memory(include_bytes!("../assets/icone.png")).ok()?;
    let image = image.into_rgba8();
    let (largeur, hauteur) = image.dimensions();
    Some(egui::IconData { rgba: image.into_raw(), width: largeur, height: hauteur })
}

fn main() -> eframe::Result<()> {
    let mut fenetre = egui::ViewportBuilder::default()
        .with_inner_size([560.0, 600.0])
        .with_min_inner_size([300.0, 380.0])
        .with_title("Capybara")
        // La transparence se demande a la creation : sur la plupart des
        // systemes elle ne s'allume pas apres coup. Elle ne se voit qu'en
        // mode jeu, ou le fond n'est pas peint.
        .with_transparent(true)
        // Sans decor a la creation. Sur Windows une fenetre transparente
        // ne s'obtient a peu pres jamais autrement : la barre de titre
        // force un fond opaque. Les modes accueil et inspection remettent
        // le decor a l'execution, ce sens la marchant bien.
        .with_decorations(false)
        .with_resizable(true);
    if let Some(icone) = icone() {
        fenetre = fenetre.with_icon(icone);
    }
    let native_options = eframe::NativeOptions { viewport: fenetre, ..Default::default() };

    eframe::run_native(
        "Capybara",
        native_options,
        Box::new(|cc| Ok(Box::new(TamagotchiApp::new(cc)))),
    )
}
