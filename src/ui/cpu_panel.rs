use egui::{Color32, RichText, Ui};

use crate::emulator::cpu::Registers;
use crate::i18n::I18n;

pub struct CpuPanel;

impl CpuPanel {
    pub fn render(ui: &mut Ui, regs: &Registers, cycles: u64, is_running: bool, i18n: &I18n) {
        ui.heading(RichText::new("ARM Cortex-M3 (SNC73410)").size(15.0));

        let status_color = if is_running { Color32::GREEN } else { Color32::YELLOW };
        let status_text = if is_running {
            i18n.choisir("En marche (48 MHz)", "Running (48 MHz)")
        } else {
            i18n.choisir("En pause / Point d'arret", "Paused / Breakpoint")
        };
        ui.horizontal(|ui| {
            ui.label(i18n.choisir("Etat :", "Status:"));
            ui.colored_label(status_color, status_text);
            ui.label(format!("{}: {}", i18n.choisir("Cycles", "Cycles"), cycles));
        });

        ui.separator();

        // Register Grid
        egui::Grid::new("reg_grid")
            .num_columns(4)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for i in 0..13 {
                    ui.label(RichText::new(format!("R{:02}", i)).strong());
                    ui.label(format!("0x{:08X}", regs.r[i]));

                    if i % 2 == 1 || i == 12 {
                        ui.end_row();
                    }
                }

                ui.label(RichText::new("MSP").strong());
                ui.label(format!("0x{:08X}", regs.msp));
                ui.label(RichText::new("PSP").strong());
                ui.label(format!("0x{:08X}", regs.psp));
                ui.end_row();

                ui.label(RichText::new("LR").strong());
                ui.label(format!("0x{:08X}", regs.lr));
                ui.label(RichText::new("PC").strong());
                ui.label(RichText::new(format!("0x{:08X}", regs.pc)).color(Color32::from_rgb(255, 220, 80)));
                ui.end_row();
            });

        ui.separator();

        // APSR Condition Flags
        ui.horizontal(|ui| {
            ui.label(RichText::new(i18n.choisir("Drapeaux :", "Flags:")).strong());
            Self::flag_label(ui, "N", regs.flag_n());
            Self::flag_label(ui, "Z", regs.flag_z());
            Self::flag_label(ui, "C", regs.flag_c());
            Self::flag_label(ui, "V", regs.flag_v());
            ui.label(format!("PRIMASK: {}", regs.primask));
            ui.label(format!("{}: {:?}", i18n.choisir("Mode", "Mode"), regs.mode));
        });
    }

    fn flag_label(ui: &mut Ui, name: &str, active: bool) {
        let col = if active { Color32::GREEN } else { Color32::GRAY };
        ui.colored_label(col, name);
    }
}
