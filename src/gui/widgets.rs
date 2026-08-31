//! L'inspecteur de flash, la seule fenetre modale qui ait survecu.
//!
//! Le reste de ce fichier decrivait une reimplementation du jeu, faite avant
//! que le projet ne fasse tourner le vrai firmware. Elle ne servait plus a
//! rien et elle est partie avec le module `core`.

use egui::{Color32, RichText, ScrollArea, Window};

use crate::hw_bridge::FlashInspector;
use crate::i18n::I18n;

/// Fenetre modale ouverte, s'il y en a une.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    FlashInspector,
}

pub struct GuiWidgets;

impl GuiWidgets {
    pub fn render_flash_inspector_modal(
        ctx: &egui::Context,
        i18n: &I18n,
        active_modal: &mut ActiveModal,
        inspector: &FlashInspector,
    ) {
        if *active_modal != ActiveModal::FlashInspector {
            return;
        }

        let mut open = true;
        Window::new(i18n.t("hw_title"))
            .open(&mut open)
            .collapsible(false)
            .default_width(480.0)
            .show(ctx, |ui| {
                ui.label(RichText::new(i18n.t("hw_desc")).italics());
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(i18n.choisir("Edition detectee :", "Detected edition:"));
                    ui.label(RichText::new(&inspector.detected_edition).color(Color32::from_rgb(100, 220, 255)).strong());
                });

                ui.horizontal(|ui| {
                    ui.label(i18n.choisir("Taille de la flash :", "Flash size:"));
                    ui.label(
                        RichText::new(format!(
                            "{} MB (128 Mbit)",
                            inspector.file_size / (1024 * 1024)
                        ))
                        .strong(),
                    );
                });

                ui.label(format!("Magic / Header: {}", inspector.header_magic));
                ui.label(format!(
                    "ARC2 Container: {} tables ({} KB / {} octets)",
                    inspector.arc2_assets_count,
                    inspector.arc2_total_bytes / 1024,
                    inspector.arc2_total_bytes
                ));
                ui.label(i18n.t("hw_uart_status"));

                ui.add_space(8.0);
                ui.label(
                    RichText::new(i18n.choisir(
                        "Organisation memoire et partitions :",
                        "Memory layout and partitions:",
                    ))
                    .strong(),
                );

                ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for section in &inspector.sections {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&section.name).strong());
                                ui.label(format!(
                                    "0x{:06X} - 0x{:06X} ({} KB)",
                                    section.offset_start,
                                    section.offset_end,
                                    section.size_bytes / 1024
                                ));
                                ui.colored_label(Color32::GREEN, &section.status);
                            });
                            ui.label(RichText::new(&section.description).small());
                        });
                    }
                });
            });

        if !open {
            *active_modal = ActiveModal::None;
        }
    }
}
