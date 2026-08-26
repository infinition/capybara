use egui::{Button, Color32, RichText, ScrollArea, Ui};

use crate::emulator::cpu::DisassembledInst;

pub struct DisasmPanel;

impl DisasmPanel {
    pub fn render(
        ui: &mut Ui,
        instructions: &[DisassembledInst],
        current_pc: u32,
        is_running: &mut bool,
        on_step_into: impl FnOnce(),
        on_reset: impl FnOnce(),
    ) {
        ui.horizontal(|ui| {
            if *is_running {
                if ui.button(RichText::new("⏸ Pause").strong()).clicked() {
                    *is_running = false;
                }
            } else if ui.button(RichText::new("▶ Run").strong().color(Color32::GREEN)).clicked() {
                *is_running = true;
            }

            if ui.add_enabled(!*is_running, Button::new("⏭ Step Into (F10)")).clicked() {
                on_step_into();
            }

            if ui.button("🔄 Reset CPU").clicked() {
                on_reset();
            }
        });

        ui.separator();

        ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
            for inst in instructions {
                let is_current = inst.address == current_pc;
                let bg_col = if is_current {
                    Color32::from_rgb(50, 65, 90)
                } else {
                    Color32::TRANSPARENT
                };

                ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, bg_col);

                ui.horizontal(|ui| {
                    let addr_text = if is_current {
                        RichText::new(format!("► 0x{:08X}", inst.address)).color(Color32::YELLOW).strong()
                    } else {
                        RichText::new(format!("  0x{:08X}", inst.address)).color(Color32::LIGHT_GRAY)
                    };
                    ui.label(addr_text);

                    let hex_bytes = inst
                        .opcode_bytes
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    ui.label(RichText::new(format!("{:9}", hex_bytes)).color(Color32::GRAY).monospace());

                    ui.label(RichText::new(&inst.mnemonic).strong().color(Color32::LIGHT_BLUE));
                    ui.label(RichText::new(&inst.operands).monospace());
                });
            }
        });
    }
}
