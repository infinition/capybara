use egui::{Color32, RichText, ScrollArea, Ui};

use crate::emulator::peripherals::UartController;
use crate::hw_bridge::UartBridge;
use crate::i18n::I18n;

pub struct ConsolePanel;

impl ConsolePanel {
    pub fn render(
        ui: &mut Ui,
        uart: &mut UartController,
        bridge: &mut UartBridge,
        refus: Option<&crate::emulator::TraceRefus>,
        i18n: &I18n,
    ) {
        ui.group(|ui| {
            ui.label(RichText::new(i18n.choisir("Liaison serie UART1", "UART1 serial link")).strong());
            ui.horizontal_wrapped(|ui| {
                ui.label(i18n.choisir("Port :", "Port:"));
                ui.add_enabled(
                    !bridge.is_connected,
                    egui::TextEdit::singleline(&mut bridge.port_name)
                        .desired_width(90.0)
                        .hint_text("COM5"),
                );

                if !bridge.available_ports.is_empty() && !bridge.is_connected {
                    egui::ComboBox::from_id_salt("ports_uart_hote")
                        .selected_text(i18n.choisir("Ports detectes", "Detected ports"))
                        .show_ui(ui, |ui| {
                            for port in bridge.available_ports.clone() {
                                if ui.selectable_label(false, &port).clicked() {
                                    bridge.port_name = port;
                                }
                            }
                        });
                }

                if ui
                    .add_enabled(!bridge.is_connected, egui::Button::new(i18n.choisir("Actualiser", "Refresh")))
                    .clicked()
                {
                    bridge.refresh_ports();
                }

                if bridge.is_connected {
                    if ui.button(i18n.choisir("Deconnecter", "Disconnect")).clicked() {
                        bridge.disconnect();
                    }
                } else if ui.button(i18n.choisir("Connecter", "Connect")).clicked() {
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
                (i18n.choisir("Connecte, 460800 bauds, 8N1", "Connected, 460800 baud, 8N1"), Color32::from_rgb(120, 220, 140))
            } else {
                (i18n.choisir("Deconnecte", "Disconnected"), Color32::GRAY)
            };
            ui.label(RichText::new(etat).small().color(couleur));
            if bridge.bytes_sent != 0 || bridge.bytes_received != 0 {
                ui.label(
                    RichText::new(format!(
                        "{} {}, {} {}",
                        bridge.bytes_sent,
                        i18n.choisir("octets vers l'hote", "bytes to host"),
                        bridge.bytes_received,
                        i18n.choisir("octets vers le Tama", "bytes to Tama")
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
                        "{} {debit} {}, {attente} {}",
                        i18n.choisir("debit programme", "configured rate"),
                        i18n.choisir("bauds", "baud"),
                        i18n.choisir("octets en attente sur la ligne", "bytes waiting on the line")
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
                        "{} {}, {} {}",
                        uart.tx_perdus,
                        i18n.choisir("octets perdus a l'emission", "bytes lost while sending"),
                        uart.rx_jetes,
                        i18n.choisir("ecartes par un vidage de file", "discarded while clearing the queue")
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
                        "{} : {}",
                        i18n.choisir("recu", "received"),
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
                        "{}  : {}",
                        i18n.choisir("emis", "sent"),
                        UartBridge::trace_hex(&bridge.debut_vers_hote)
                    ))
                    .small()
                    .monospace()
                    .color(Color32::from_rgb(150, 230, 190)),
                );
            }
            #[cfg(not(target_arch = "wasm32"))]
            if bridge.is_connected {
                ui.label(
                    RichText::new(format!(
                        "{} : {}",
                        i18n.choisir("capture", "capture"),
                        crate::hw_bridge::uart_terminal::dossier_captures().display()
                    ))
                    .small()
                    .color(Color32::GRAY),
                );
            }
            // Le contexte du premier refus emis sur la liaison. C'est de la
            // que part la recherche : l'echange n'a lieu qu'avec un outil
            // exterieur, et toute sonde qui le ralentit l'empeche d'avoir lieu.
            if let Some(t) = refus {
                ui.separator();
                ui.label(
                    RichText::new(i18n.choisir("Premier refus emis", "First rejection sent"))
                        .strong()
                        .color(Color32::from_rgb(240, 170, 120)),
                );
                ui.label(
                    RichText::new(format!("PC {:#010x}   LR {:#010x}   SP {:#010x}", t.pc, t.lr, t.sp))
                        .small()
                        .monospace(),
                );
                let regs: Vec<String> = t
                    .registres
                    .iter()
                    .enumerate()
                    .map(|(i, v)| format!("r{i}={v:#010x}"))
                    .collect();
                ui.label(RichText::new(regs.join("  ")).small().monospace());
                if !t.chemin.is_empty() {
                    // Le chemin reellement parcouru, contrairement a la pile
                    // qui garde des valeurs perimees.
                    let bouts: Vec<String> =
                        t.chemin.iter().rev().take(18).map(|a| format!("{a:#010x}")).collect();
                    ui.label(
                        RichText::new(format!("chemin : {}", bouts.join(" <- ")))
                            .small()
                            .monospace()
                            .color(Color32::from_rgb(230, 200, 150)),
                    );
                }
                if !t.retours.is_empty() {
                    let chaine: Vec<String> =
                        t.retours.iter().map(|a| format!("{a:#010x}")).collect();
                    ui.label(
                        RichText::new(format!("appels : {}", chaine.join(" <- ")))
                            .small()
                            .monospace(),
                    );
                }
                ui.separator();
            }
            if let Some(message) = &bridge.last_error {
                ui.label(RichText::new(message).small().color(Color32::LIGHT_RED));
            }
            ui.label(
                RichText::new(
                    i18n.choisir(
                        "Utiliser une paire de ports serie virtuels : ce programme ouvre un cote, l'outil de transfert ouvre l'autre.",
                        "Use a pair of virtual serial ports: this program opens one side and the transfer tool opens the other.",
                    ),
                )
                .small()
                .color(Color32::GRAY),
            );
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label(RichText::new("UART console").strong());
            if ui.button(i18n.choisir("Effacer", "Clear")).clicked() {
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
                        RichText::new(i18n.choisir("[Aucune sortie UART]", "[No UART output]"))
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
