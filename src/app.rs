use eframe::egui;
use egui::{CentralPanel, Context, Key, SidePanel, TopBottomPanel};

use crate::audio::{AudioEngine, SoundEffect};
use crate::emulator::Machine;
use crate::gui::{ActiveModal, GuiWidgets, ShellColor};
use crate::hw_bridge::FlashInspector;
use crate::i18n::{I18n, Language};
use crate::ui::{ConsolePanel, CpuPanel, DisasmPanel, LcdPanel, MemoryPanel};

pub struct TamagotchiApp {
    pub machine: Machine,
    pub audio: AudioEngine,
    pub i18n: I18n,
    pub shell_color: ShellColor,
    pub show_debugger: bool,
    pub hex_base_addr: u32,
    pub last_frame_time: std::time::Instant,
    pub load_path_input: String,
    pub status_msg: Option<String>,
    pub flash_inspector: FlashInspector,
    pub active_modal: ActiveModal,
    pub disasm_view_addr: u32,
    /// Temps accorde a l'emulation a chaque image de l'interface.
    pub budget_ms: u64,
    /// Broches maintenues basses, avec le nombre d'images restantes.
    ///
    /// Un appui doit durer plus qu'une image : le firmware scrute ses boutons a
    /// sa propre cadence, et un appui d'une seule image lui echappe.
    pub appuis: std::collections::HashMap<u32, u32>,
    /// Phases de l'encodeur restant a jouer, une par image.
    pub phases_encodeur: std::collections::VecDeque<(bool, bool)>,
    /// Texture de l'ecran, refaite seulement quand une trame arrive.
    pub ecran: Option<egui::TextureHandle>,
    /// Anneau d'instantanes automatiques, pour revenir avant un blocage.
    pub historique: crate::emulator::etat::Historique,
    /// Etat publie au serveur local et commandes qui en reviennent.
    pub partage: std::sync::Arc<std::sync::Mutex<crate::web::Partage>>,
    /// Port du serveur local, quand il a pu demarrer.
    pub port_web: Option<u16>,
    /// Debit atteint, en pas par seconde. C'est le seul chiffre qui dit si
    /// l'interface etouffe l'emulation.
    pub debit: f64,
    /// Point de depart de la mesure de debit en cours.
    pub debit_depart: (u64, std::time::Instant),
}

impl TamagotchiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let audio = AudioEngine::new();
        let i18n = I18n::default();
        let machine = Machine::new();

        // Le serveur local permet de suivre l'emulation depuis un navigateur,
        // et d'envoyer les memes commandes que la fenetre.
        let partage = std::sync::Arc::new(std::sync::Mutex::new(crate::web::Partage::default()));
        let port_web = crate::web::demarrer(std::sync::Arc::clone(&partage), 7340).ok();

        Self {
            machine,
            audio,
            i18n,
            shell_color: ShellColor::OceanBlue,
            show_debugger: false,
            hex_base_addr: 0x6001_1000,
            last_frame_time: std::time::Instant::now(),
            load_path_input: String::new(),
            status_msg: Some("Tamagotchi Paradise Hardware Emulation Ready.".to_string()),
            flash_inspector: FlashInspector::new(),
            active_modal: ActiveModal::None,
            disasm_view_addr: 0x6001_1000,
            budget_ms: 12,
            appuis: std::collections::HashMap::new(),
            phases_encodeur: std::collections::VecDeque::new(),
            ecran: None,
            historique: crate::emulator::etat::Historique::default(),
            partage,
            port_web,
            debit: 0.0,
            debit_depart: (0, std::time::Instant::now()),
        }
    }
}

impl TamagotchiApp {
    /// Charge un dump de flash et rend compte de ce qui s'est reellement passe.
    fn load_firmware(&mut self, path: std::path::PathBuf) {
        self.load_path_input = path.to_string_lossy().to_string();
        self.machine.console.clear();
        self.appuis.clear();
        self.phases_encodeur.clear();
        self.historique.vider();
        match self.machine.load_firmware_file(&path) {
            Ok(report) => {
                // Les dumps Earth et Land ont ete extraits pile faible : sans
                // cela le firmware affiche son message et s'eteint aussitot.
                self.machine.remplacer_la_pile();
                let _ = self.flash_inspector.inspect_file(&path);
                // Un firmware demarrable s'execute depuis la PRAM mappee a 0,
                // sinon on laisse la vue sur le code XIP, lui toujours en clair.
                self.hex_base_addr = if report.bootable { 0 } else { 0x6001_1000 };
                self.disasm_view_addr = self.machine.cpu.regs.pc;
                self.status_msg = Some(self.describe_load(&report));
            }
            Err(e) => {
                self.status_msg = Some(self.i18n.t_args("emu_load_error", &[("error", &e)]));
            }
        }
    }

    fn describe_load(&self, r: &crate::emulator::LoadReport) -> String {
        let bytes = r.bytes.to_string();
        if r.bootable {
            self.i18n.t_args(
                "emu_load_bootable",
                &[
                    ("bytes", &bytes),
                    ("pc", &format!("0x{:08X}", r.entry_pc)),
                    ("sp", &format!("0x{:08X}", r.entry_sp)),
                ],
            )
        } else if r.encrypted {
            self.i18n.t("emu_load_need_key")
        } else {
            self.i18n.t_args("emu_load_not_bootable", &[("bytes", &bytes)])
        }
    }
}

impl TamagotchiApp {
    /// Maintient une broche basse pendant quelques images.
    fn presser(&mut self, broche: u32) {
        // Six images couvrent largement la periode a laquelle le firmware
        // relit ses boutons, sans rendre l'appui collant a l'usage.
        self.appuis.insert(broche, 6);
    }

    /// Programme un cran d'encodeur, en quadrature.
    ///
    /// Les deux voies sont hautes au repos. Un cran les fait passer par la
    /// sequence de Gray, dans un sens ou dans l'autre selon le signe. Une phase
    /// par image de l'interface suffit : le firmware echantillonne l'encodeur
    /// dans son interruption de base de temps.
    fn tourner_molette(&mut self, sens: i32) {
        const AVANT: [(bool, bool); 4] = [(false, true), (false, false), (true, false), (true, true)];
        let mut phases: Vec<(bool, bool)> = AVANT.to_vec();
        if sens < 0 {
            phases = phases.iter().map(|&(a, b)| (b, a)).collect();
        }
        for phase in phases {
            self.phases_encodeur.push_back(phase);
        }
    }

    /// Applique l'etat des entrees pour l'image en cours.
    fn appliquer_entrees(&mut self) {
        let broches: Vec<u32> = self.appuis.keys().copied().collect();
        for broche in broches {
            match self.appuis.get_mut(&broche) {
                Some(restant) if *restant > 0 => {
                    self.machine.appuyer(broche);
                    *restant -= 1;
                }
                _ => {
                    self.machine.relacher(broche);
                    self.appuis.remove(&broche);
                }
            }
        }
        if let Some((voie1, voie2)) = self.phases_encodeur.pop_front() {
            if voie1 {
                self.machine.relacher(Machine::ENCODEUR_1);
            } else {
                self.machine.appuyer(Machine::ENCODEUR_1);
            }
            if voie2 {
                self.machine.relacher(Machine::ENCODEUR_2);
            } else {
                self.machine.appuyer(Machine::ENCODEUR_2);
            }
        }
    }

    /// Revient a l'instantane precedent.
    fn reculer(&mut self) {
        match self.historique.reculer() {
            Some(etat) => {
                let cycles = etat.cycles;
                self.machine.restaurer(&etat);
                self.appuis.clear();
                self.phases_encodeur.clear();
                self.status_msg = Some(format!("Retour a {} pas executes.", cycles));
            }
            None => {
                self.status_msg = Some("Aucun instantane a restaurer.".to_string());
            }
        }
    }

    /// Publie l'image et le diagnostic pour le serveur local.
    fn publier(&mut self) {
        let rapport = self.diagnostic();
        let mut partage = self.partage.lock().unwrap();
        partage.ecran.clear();
        partage.ecran.extend_from_slice(&self.machine.periph.display.vram);
        partage.largeur = self.machine.periph.display.width;
        partage.hauteur = self.machine.periph.display.height;
        partage.trames = self.machine.periph.display.trames;
        partage.diagnostic = rapport;
    }

    /// Rapport d'etat copiable, pour signaler un blocage sans capture d'ecran.
    fn diagnostic(&self) -> String {
        let n = &self.machine.cpu.nvic;
        let mode = match self.machine.cpu.regs.mode {
            crate::emulator::cpu::registers::Mode::Thread => "Thread",
            _ => "Handler",
        };
        let etat = |a: u32| -> u32 {
            let o = (a - 0x1800_0000) as usize;
            let b = |i: usize| self.machine.bus.sram.read_u8(o + i) as u32;
            b(0) | (b(1) << 8)
        };
        let console: String = self.machine.console.chars().rev().take(600).collect::<Vec<_>>()
            .into_iter().rev().collect();
        format!(
            "== diagnostic emulateur Tamagotchi Paradise\n\
             firmware      {}\n\
             pas executes  {}   debit {:.1} millions par seconde\n\
             PC            {:#010x}   mode {}   PRIMASK {}\n\
             trames ecran  {}   instantanes {}\n\
             etat du jeu   courant {}   transition demandee {}\n\
             IRQ 0..31     activees {:#010x}  en attente {:#010x}\n\
             IRQ 32..63    activees {:#010x}  en attente {:#010x}\n\
             dernier transfert vers l'ecran : {}\n\
             console du firmware (fin) :\n\
             {}\n",
            self.load_path_input,
            self.machine.cpu.cycles,
            self.debit / 1e6,
            self.machine.cpu.regs.pc,
            mode,
            self.machine.cpu.regs.primask,
            self.machine.periph.display.trames,
            self.historique.len(),
            etat(0x1800_1BF4),
            etat(0x1800_1BF6),
            n.iser[0],
            n.ispr[0],
            n.iser[1],
            n.ispr[1],
            match self.machine.periph.dma.canaux.first() {
                Some(c) => format!(
                    "source {:#010x}  destination {:#010x}  unites {}",
                    c.source,
                    c.destination,
                    c.compte & crate::emulator::peripherals::dma::MASQUE_COMPTE
                ),
                None => "aucun".to_string(),
            },
            console.trim_end()
        )
    }
}

impl eframe::App for TamagotchiApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let now = std::time::Instant::now();
        let _dt = (now - self.last_frame_time).as_secs_f32().min(0.1);
        self.last_frame_time = now;

        // Auto repaint for 60 FPS emulator loop
        ctx.request_repaint();

        // 1. Entrees : les broches sont maintenues basses plusieurs images, sans
        //    quoi le firmware, qui scrute a sa propre cadence, rate l'appui.
        let key_a = ctx.input(|i| i.key_down(Key::A) || i.key_down(Key::ArrowLeft));
        let key_b = ctx.input(|i| i.key_down(Key::B) || i.key_down(Key::ArrowDown));
        let key_c = ctx.input(|i| i.key_down(Key::C) || i.key_down(Key::ArrowRight));
        let key_ok = ctx.input(|i| i.key_down(Key::Space) || i.key_down(Key::Enter));
        let key_f10 = ctx.input(|i| i.key_pressed(Key::F10));
        let molette = ctx.input(|i| {
            (i.key_pressed(Key::ArrowUp) as i32) - (i.key_pressed(Key::PageDown) as i32)
        });

        if key_a {
            self.presser(Machine::BOUTON_A);
        }
        if key_b {
            self.presser(Machine::BOUTON_B);
        }
        if key_c {
            self.presser(Machine::BOUTON_C);
        }
        if key_ok {
            self.presser(Machine::BOUTON_MOLETTE);
        }
        if molette != 0 {
            self.tourner_molette(molette);
        }

        if key_f10 {
            self.machine.is_running = false;
            self.machine.step();
        }

        // 2. Avance de l'emulation, bornee en temps pour que l'interface reste
        //    reactive. Le coeur tourne a environ dix-neuf millions de pas par
        //    seconde, soit un cinquieme de la console.
        // Commandes venues du navigateur : elles passent par les memes broches
        // que la fenetre.
        let recues: Vec<crate::web::Commande> = {
            let mut partage = self.partage.lock().unwrap();
            std::mem::take(&mut partage.commandes)
        };
        for commande in recues {
            match commande {
                crate::web::Commande::BoutonA => self.presser(Machine::BOUTON_A),
                crate::web::Commande::BoutonB => self.presser(Machine::BOUTON_B),
                crate::web::Commande::BoutonC => self.presser(Machine::BOUTON_C),
                crate::web::Commande::Molette => self.presser(Machine::BOUTON_MOLETTE),
                crate::web::Commande::Tourner(sens) => self.tourner_molette(sens),
                crate::web::Commande::Reculer => self.reculer(),
            }
        }

        self.appliquer_entrees();
        if self.machine.is_running && self.budget_ms > 0 {
            let debut = std::time::Instant::now();
            let limite = std::time::Duration::from_millis(self.budget_ms);
            while debut.elapsed() < limite {
                if !matches!(self.machine.run_frame(), crate::emulator::StepResult::Ok(_)) {
                    break;
                }
            }
            self.historique.suivre(&self.machine);
        }

        // Debit reel, mesure sur une demi-seconde.
        let ecoule = self.debit_depart.1.elapsed().as_secs_f64();
        if ecoule >= 0.5 {
            let faits = self.machine.cpu.cycles.saturating_sub(self.debit_depart.0);
            self.debit = faits as f64 / ecoule;
            self.debit_depart = (self.machine.cpu.cycles, std::time::Instant::now());
        }

        // Support Drag and Drop of firmware files onto the emulator
        ctx.input(|i| {
            if let Some(dropped) = i.raw.dropped_files.first() {
                if let Some(path) = &dropped.path {
                    self.load_firmware(path.clone());
                }
            }
        });

        // 3. Top Status & Menu Bar
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tamagotchi Paradise SNC73410 Emulator").strong());

                ui.separator();

                if ui.button(if self.show_debugger { "Masquer l'inspection" } else { "Afficher l'inspection" }).clicked() {
                    self.show_debugger = !self.show_debugger;
                }

                if ui.button("💾 Inspecteur Flash").clicked() {
                    self.active_modal = ActiveModal::FlashInspector;
                }

                ui.separator();

                ui.label("Shell Color:");
                if ui.selectable_label(self.shell_color == ShellColor::OceanBlue, "Ocean").clicked() {
                    self.shell_color = ShellColor::OceanBlue;
                }
                if ui.selectable_label(self.shell_color == ShellColor::JungleGreen, "Jungle").clicked() {
                    self.shell_color = ShellColor::JungleGreen;
                }
                if ui.selectable_label(self.shell_color == ShellColor::SunsetPink, "Sunset").clicked() {
                    self.shell_color = ShellColor::SunsetPink;
                }
                if ui.selectable_label(self.shell_color == ShellColor::CyberGrey, "Cyber").clicked() {
                    self.shell_color = ShellColor::CyberGrey;
                }

                ui.separator();

                ui.label("Language:");
                if ui.selectable_label(self.i18n.language() == Language::Fr, "FR").clicked() {
                    self.i18n.set_language(Language::Fr);
                }
                if ui.selectable_label(self.i18n.language() == Language::En, "EN").clicked() {
                    self.i18n.set_language(Language::En);
                }
            });
        });

        // 4. Panneau lateral : l'essentiel toujours, l'inspection sur demande.
        {
            SidePanel::right("debug_panel")
                .min_width(420.0)
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.add_space(4.0);

                    // Firmware File Loader Box
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Firmware / Flash Dump (.bin):").strong());
                        ui.horizontal(|ui| {
                            if ui.button(egui::RichText::new("📂 Parcourir / Browse...").strong()).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Firmware Binary (*.bin, *.rom, *.hex, *.elf)", &["bin", "rom", "hex", "elf", "raw", "dump"])
                                    .set_title("Sélectionner un dump de firmware Tamagotchi")
                                    .pick_file()
                                {
                                    self.load_firmware(path.clone());
                                }
                            }

                            ui.text_edit_singleline(&mut self.load_path_input);
                            if ui.button("Load").clicked() && !self.load_path_input.is_empty() {
                                self.load_firmware(std::path::PathBuf::from(self.load_path_input.clone()));
                            }
                        });
                        if let Some(msg) = &self.status_msg {
                            ui.label(egui::RichText::new(msg).small().color(egui::Color32::from_rgb(255, 230, 80)));
                        }
                    });

                    ui.separator();

                    // Vitesse et diagnostic : de quoi jouer, puis rapporter un
                    // blocage sans avoir a relire la trace.
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Vitesse :").strong());
                            for (nom, ms) in
                                [("Pause", 0u64), ("Normale", 12), ("Rapide", 40), ("Turbo", 150)]
                            {
                                if ui.selectable_label(self.budget_ms == ms, nom).clicked() {
                                    self.budget_ms = ms;
                                }
                            }
                            if ui.button("Revenir en arriere").clicked() {
                                self.reculer();
                            }
                            if ui.button("Rejouer depuis le debut").clicked() {
                                let chemin = self.load_path_input.clone();
                                if !chemin.is_empty() {
                                    self.load_firmware(std::path::PathBuf::from(chemin));
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Sauver l'etat...").clicked() {
                                if let Some(chemin) = rfd::FileDialog::new()
                                    .add_filter("Etat de l'emulateur (*.tamastate)", &["tamastate"])
                                    .set_file_name("tamagotchi.tamastate")
                                    .save_file()
                                {
                                    let etat = self.machine.instantane();
                                    self.status_msg = Some(match etat.ecrire(&chemin) {
                                        Ok(()) => format!("Etat ecrit dans {}", chemin.display()),
                                        Err(e) => format!("Ecriture impossible : {}", e),
                                    });
                                }
                            }
                            if ui.button("Charger un etat...").clicked() {
                                if let Some(chemin) = rfd::FileDialog::new()
                                    .add_filter("Etat de l'emulateur (*.tamastate)", &["tamastate"])
                                    .pick_file()
                                {
                                    self.status_msg = Some(
                                        match crate::emulator::etat::Instantane::lire(&chemin) {
                                            Ok(etat) => {
                                                self.machine.restaurer(&etat);
                                                self.appuis.clear();
                                                self.phases_encodeur.clear();
                                                format!("Etat restaure, {} pas executes.", etat.cycles)
                                            }
                                            Err(e) => format!("Lecture impossible : {}", e),
                                        },
                                    );
                                }
                            }
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} instantanes automatiques",
                                    self.historique.len()
                                ))
                                .small(),
                            );
                        });
                        match self.port_web {
                            Some(port) => {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Suivi dans le navigateur : http://127.0.0.1:{}/",
                                        port
                                    ))
                                    .small()
                                    .color(egui::Color32::from_rgb(140, 220, 160)),
                                );
                            }
                            None => {
                                ui.label(
                                    egui::RichText::new("Serveur local indisponible.").small(),
                                );
                            }
                        }
                        ui.label(
                            egui::RichText::new(
                                "Clavier : A, B, C pour les boutons, Espace ou Entree pour la                                  molette, Fleche haut et Page suivante pour la tourner.",
                            )
                            .small(),
                        );

                        let rapport = self.diagnostic();
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Diagnostic").strong());
                            if ui.button("Copier").clicked() {
                                ui.output_mut(|o| o.copied_text = rapport.clone());
                                self.status_msg =
                                    Some("Diagnostic copie dans le presse-papiers.".to_string());
                            }
                        });
                        egui::ScrollArea::vertical()
                            .max_height(170.0)
                            .id_source("diagnostic")
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(egui::RichText::new(&rapport).monospace().small())
                                        .wrap(),
                                );
                            });
                    });

                    // Les panneaux d'inspection coutent plus cher a dessiner que
                    // l'emulation n'en gagne a tourner : ils restent replies par
                    // defaut, et le debit affiche dit tout de suite ce qu'ils
                    // prennent.
                    if !self.show_debugger {
                        return;
                    }

                    ui.separator();

                    // CPU Registers Inspector
                    CpuPanel::render(
                        ui,
                        &self.machine.cpu.regs,
                        self.machine.cpu.cycles,
                        self.machine.is_running,
                        &self.i18n,
                    );

                    ui.separator();

                    // Disassembly Stream
                    let instructions = self.machine.get_disassembly_at(self.disasm_view_addr, 12);
                    let current_pc = self.machine.cpu.regs.pc;
                    let is_running_ref = &mut self.machine.is_running;
                    let view_addr_ref = &mut self.disasm_view_addr;

                    let mut step_requested = false;
                    let mut reset_requested = false;
                    let mut new_pc_target = None;
                    DisasmPanel::render(
                        ui,
                        &instructions,
                        current_pc,
                        is_running_ref,
                        view_addr_ref,
                        || {
                            step_requested = true;
                        },
                        || {
                            reset_requested = true;
                        },
                        |target| {
                            new_pc_target = Some(target);
                        },
                    );

                    if let Some(target) = new_pc_target {
                        self.machine.cpu.regs.pc = target & !1;
                        self.disasm_view_addr = target & !1;
                    }
                    if step_requested {
                        self.machine.step();
                        self.disasm_view_addr = self.machine.cpu.regs.pc;
                    }
                    if reset_requested {
                        self.machine.reset();
                        self.disasm_view_addr = self.machine.cpu.regs.pc;
                    }

                    ui.separator();

                    // Hex Memory Viewer
                    MemoryPanel::render(
                        ui,
                        &mut self.machine.bus,
                        &mut self.machine.periph,
                        &self.machine.cpu.nvic,
                        &mut self.hex_base_addr,
                    );

                    ui.separator();

                    // UART Console
                    ConsolePanel::render(ui, &mut self.machine.periph.uart);
                });
        }

        // 5. Central Virtual Device Display & Controls
        CentralPanel::default().show(ctx, |ui| {
            let available_rect = ui.available_rect_before_wrap();

            // L'ecran est une texture : la retesseler en seize mille rectangles a
            // chaque image mangeait le temps qui doit aller a l'emulation.
            if self.machine.periph.display.dirty || self.ecran.is_none() {
                let d = &self.machine.periph.display;
                let mut pixels = Vec::with_capacity(d.width * d.height);
                for &brut in &d.vram {
                    let r = (((brut >> 11) & 0x1F) * 255 / 31) as u8;
                    let v = (((brut >> 5) & 0x3F) * 255 / 63) as u8;
                    let b = ((brut & 0x1F) * 255 / 31) as u8;
                    pixels.push(egui::Color32::from_rgb(r, v, b));
                }
                let image = egui::ColorImage { size: [d.width, d.height], pixels };
                let options = egui::TextureOptions::NEAREST;
                match &mut self.ecran {
                    Some(texture) => texture.set(image, options),
                    None => {
                        self.ecran = Some(ctx.load_texture("ecran_console", image, options));
                    }
                }
                self.machine.periph.display.dirty = false;
                self.publier();
            }

            let commandes = LcdPanel::render(
                ui,
                available_rect,
                &self.machine.periph.display,
                self.ecran.as_ref(),
                self.shell_color,
            );

            // Les commandes vont sur les vraies broches : bouton A en P0.9, B en
            // P0.11, C en P0.10, appui de molette en P0.8, encodeur sur P2.0 et
            // P2.1.
            if commandes.bouton_a {
                self.presser(Machine::BOUTON_A);
            }
            if commandes.bouton_b {
                self.presser(Machine::BOUTON_B);
            }
            if commandes.bouton_c {
                self.presser(Machine::BOUTON_C);
            }
            if commandes.molette_appuyee {
                self.presser(Machine::BOUTON_MOLETTE);
            }
            if commandes.molette_tournee != 0 {
                self.tourner_molette(commandes.molette_tournee);
                self.audio.play(SoundEffect::DialTick);
            }
        });

        // 6. Modals
        GuiWidgets::render_flash_inspector_modal(
            ctx,
            &self.i18n,
            &mut self.active_modal,
            &self.flash_inspector,
        );
    }
}
