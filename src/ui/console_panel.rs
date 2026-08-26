use egui::{Color32, RichText, ScrollArea, Ui};

use crate::emulator::peripherals::UartController;

pub struct ConsolePanel;

impl ConsolePanel {
    pub fn render(ui: &mut Ui, uart: &mut UartController) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("UART Console (460800 baud)").strong());
            if ui.button("Clear Log").clicked() {
                uart.console_history.clear();
            }
        });

        ui.separator();

        ScrollArea::vertical()
            .stick_to_bottom(true)
            .max_height(140.0)
            .show(ui, |ui| {
                if uart.console_history.is_empty() {
                    ui.label(RichText::new("[No UART output yet]").italics().color(Color32::GRAY));
                } else {
                    ui.label(
                        RichText::new(&uart.console_history)
                            .monospace()
                            .color(Color32::from_rgb(180, 240, 180)),
                    );
                }
            });
    }
}
