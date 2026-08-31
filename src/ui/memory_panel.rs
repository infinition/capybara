use egui::{Color32, RichText, ScrollArea, Ui};

use crate::emulator::mmu::MemoryBus;
use crate::emulator::peripherals::Peripherals;
use crate::i18n::I18n;

pub struct MemoryPanel;

impl MemoryPanel {
    pub fn render(
        ui: &mut Ui,
        bus: &mut MemoryBus,
        periph: &mut Peripherals,
        nvic: &crate::emulator::cpu::Nvic,
        base_address: &mut u32,
        i18n: &I18n,
    ) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(i18n.choisir("Memoire hexadecimale :", "Memory hex viewer:")).strong());
            if ui.button("XIP (0x60000000)").clicked() {
                *base_address = 0x6000_0000;
            }
            if ui.button("Flash (0x10000000)").clicked() {
                *base_address = 0x1000_0000;
            }
            if ui.button("SRAM (0x20000000)").clicked() {
                *base_address = 0x2000_0000;
            }
            if ui.button("BootROM (0x08000000)").clicked() {
                *base_address = 0x0800_0000;
            }
            if ui.button("SYS0 (0x45000000)").clicked() {
                *base_address = 0x4500_0000;
            }
        });

        ui.separator();

        ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
            for row in 0..16 {
                let addr = *base_address + (row as u32 * 16);
                let mut bytes = Vec::new();
                for col in 0..16 {
                    bytes.push(bus.read_u8(addr + col, periph, nvic));
                }

                let hex_str = bytes
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");

                let ascii_str: String = bytes
                    .iter()
                    .map(|&b| if b >= 32 && b < 127 { b as char } else { '.' })
                    .collect();

                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("0x{:08X}:", addr)).color(Color32::YELLOW).monospace());
                    ui.label(RichText::new(hex_str).monospace().color(Color32::LIGHT_GRAY));
                    ui.label(RichText::new(ascii_str).monospace().color(Color32::LIGHT_BLUE));
                });
            }
        });
    }
}
