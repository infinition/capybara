//! Dit si la fenetre transparente est reellement obtenue, et pourquoi non.
//!
//! Usage : cargo run --release --example transparence_probe
//!
//! eframe signale par le journal qu'il n'a pas pu obtenir une configuration
//! graphique avec couche alpha, mais l'application n'ecoute pas ce journal et
//! le message se perd. Cette sonde ouvre une fenetre aux memes reglages que le
//! mode jeu, branche un journal sur la sortie d'erreur, et se ferme seule.

use eframe::egui;

struct Journal;

impl log::Log for Journal {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, entree: &log::Record) {
        let cible = entree.target();
        if cible.starts_with("eframe") || cible.starts_with("egui") || cible.starts_with("glutin") {
            eprintln!("[{}] {} : {}", entree.level(), cible, entree.args());
        }
    }
    fn flush(&self) {}
}

static JOURNAL: Journal = Journal;

struct Sonde {
    depart: std::time::Instant,
}

impl eframe::App for Sonde {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let zone = ui.available_rect_before_wrap();
                ui.painter().circle_filled(
                    zone.center(),
                    zone.width().min(zone.height()) * 0.3,
                    egui::Color32::from_rgb(240, 140, 180),
                );
            });
        ctx.request_repaint();
        if self.depart.elapsed().as_secs_f32() > 3.0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn main() -> eframe::Result<()> {
    log::set_logger(&JOURNAL).ok();
    log::set_max_level(log::LevelFilter::Debug);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 320.0])
            .with_transparent(true)
            .with_decorations(false),
        ..Default::default()
    };
    eframe::run_native(
        "sonde de transparence",
        options,
        Box::new(|_| Ok(Box::new(Sonde { depart: std::time::Instant::now() }))),
    )
}
