use egui::{Color32, RichText, ScrollArea, Ui};

use crate::emulator::peripherals::UartController;
use crate::hw_bridge::UartBridge;

pub struct ConsolePanel;

impl ConsolePanel {
    pub fn render(ui: &mut Ui, uart: &mut UartController, bridge: &mut UartBridge) {
        ui.group(|ui| {
            ui.label(RichText::new("Liaison serie UART1").strong());
            ui.horizontal_wrapped(|ui| {
                ui.label("Port :");
                ui.add_enabled(
                    !bridge.is_connected,
                    egui::TextEdit::singleline(&mut bridge.port_name)
                        .desired_width(90.0)
                        .hint_text("COM5"),
                );

                if !bridge.available_ports.is_empty() && !bridge.is_connected {
                    egui::ComboBox::from_id_salt("ports_uart_hote")
                        .selected_text("Ports detectes")
                        .show_ui(ui, |ui| {
                            for port in bridge.available_ports.clone() {
                                if ui.selectable_label(false, &port).clicked() {
                                    bridge.port_name = port;
                                }
                            }
                        });
                }

                if ui
                    .add_enabled(!bridge.is_connected, egui::Button::new("Actualiser"))
                    .clicked()
                {
                    bridge.refresh_ports();
                }

                if bridge.is_connected {
                    if ui.button("Deconnecter").clicked() {
                        bridge.disconnect();
                    }
                } else if ui.button("Connecter").clicked() {
                    let port = bridge.port_name.clone();
                    // La ligne est videe avant d'ouvrir : le journal de
                    // demarrage de la console attend sinon dans la file de
                    // sortie et part en premier vers l'outil de transfert.
                    uart.vider_la_ligne();
                    bridge.debut_vers_tama.clear();
                    bridge.debut_vers_hote.clear();
                    let _ = bridge.connect(&port);
                }
            });

            let (etat, couleur) = if bridge.is_connected {
                ("Connecte, 460800 bauds, 8N1", Color32::from_rgb(120, 220, 140))
            } else {
                ("Deconnecte", Color32::GRAY)
            };
            ui.label(RichText::new(etat).small().color(couleur));
            if bridge.bytes_sent != 0 || bridge.bytes_received != 0 {
                ui.label(
                    RichText::new(format!(
                        "{} octets vers l'hote, {} octets vers le Tama",
                        bridge.bytes_sent, bridge.bytes_received
                    ))
                    .small(),
                );
            }
            // Le debit que le firmware s'est programme, et ce qui attend encore
            // sur la ligne. Un retard qui gonfle sans redescendre signifie que
            // l'hote parle plus vite que la console ne peut ecouter, et c'est
            // ce qui fait expirer les delais de l'outil de transfert.
            if bridge.is_connected {
                let debit = uart.baud_rate(crate::emulator::peripherals::snsys::CYCLES_PAR_SECONDE as u32);
                let attente = uart.rx_in.len();
                let couleur = if attente > 2000 {
                    Color32::from_rgb(230, 140, 110)
                } else {
                    Color32::GRAY
                };
                ui.label(
                    RichText::new(format!(
                        "debit programme {debit} bauds, {attente} octets en attente sur la ligne"
                    ))
                    .small()
                    .color(couleur),
                );
            }
            // Un transfert qui echoue sur une somme de controle vient presque
            // toujours d'octets perdus. Ces deux compteurs disent lesquels, et
            // de quel cote, sans avoir a instrumenter quoi que ce soit.
            if uart.tx_perdus != 0 || uart.rx_jetes != 0 {
                ui.label(
                    RichText::new(format!(
                        "{} octets perdus a l'emission, {} ecartes par un vidage de file",
                        uart.tx_perdus, uart.rx_jetes
                    ))
                    .small()
                    .color(Color32::from_rgb(230, 180, 90)),
                );
            }
            // Les premiers octets de chaque sens. Quand un transfert echoue sans
            // qu'aucun octet ne se perde, eux seuls disent si l'en-tete du
            // paquet arrive intact.
            if !bridge.debut_vers_tama.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "recu : {}",
                        UartBridge::trace_hex(&bridge.debut_vers_tama)
                    ))
                    .small()
                    .monospace()
                    .color(Color32::from_rgb(150, 190, 230)),
                );
            }
            if !bridge.debut_vers_hote.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "emis  : {}",
                        UartBridge::trace_hex(&bridge.debut_vers_hote)
                    ))
                    .small()
                    .monospace()
                    .color(Color32::from_rgb(150, 230, 190)),
                );
            }
            if let Some(message) = &bridge.last_error {
                ui.label(RichText::new(message).small().color(Color32::LIGHT_RED));
            }
            ui.label(
                RichText::new(
                    "Utiliser une paire de ports serie virtuels : ce programme ouvre un cote, l'outil de transfert ouvre l'autre.",
                )
                .small()
                .color(Color32::GRAY),
            );
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label(RichText::new("Console UART").strong());
            if ui.button("Effacer").clicked() {
                uart.console_history.clear();
            }
        });

        ui.separator();

        ScrollArea::vertical()
            .stick_to_bottom(true)
            .max_height(140.0)
            .show(ui, |ui| {
                if uart.console_history.is_empty() {
                    ui.label(
                        RichText::new("[Aucune sortie UART]")
                            .italics()
                            .color(Color32::GRAY),
                    );
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
