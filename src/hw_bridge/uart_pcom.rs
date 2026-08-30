/// Nombre de lectures consecutives par tour de boucle.
///
/// Le tampon du systeme derriere un port serie est petit, souvent quatre kilo
/// octets. Une seule lecture par image d'interface ne suffit pas a le vider
/// quand l'outil de transfert envoie un bloc de plus de quatre mille octets :
/// le tampon deborde et le systeme jette des octets, sans que rien ne le
/// signale. On vide donc le port jusqu'a ce qu'il n'ait plus rien a rendre.
const LECTURES_PAR_TOUR: usize = 16;

use std::collections::VecDeque;
use std::io::{self, Read, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::emulator::peripherals::UartController;

/// Pont entre l'UART1 emule et un flux serie de l'ordinateur hote.
pub struct UartHostBridge {
    pub port_name: String,
    pub baud_rate: u32,
    pub is_connected: bool,
    pub bytes_sent: usize,
    pub bytes_received: usize,
    pub last_error: Option<String>,
    /// Premiers octets de chaque sens depuis la connexion. Quand un transfert
    /// echoue sans qu'aucun octet ne se perde, seuls les octets eux memes
    /// disent si l'en-tete du paquet arrive intact.
    pub debut_vers_tama: Vec<u8>,
    pub debut_vers_hote: Vec<u8>,
    pub available_ports: Vec<String>,

    // Files du transport en memoire, utilisees par les tests et par les
    // integrations qui ne passent pas par un port serie du systeme.
    host_rx_stream: VecDeque<u8>,
    host_tx_stream: VecDeque<u8>,
    pending_to_host: VecDeque<u8>,

    #[cfg(not(target_arch = "wasm32"))]
    serial: Option<Box<dyn serialport::SerialPort>>,
    /// Octets releves par le fil de lecture, en attente d'etre remis au
    /// controleur.
    #[cfg(not(target_arch = "wasm32"))]
    recus: Arc<Mutex<VecDeque<u8>>>,
    /// Drapeau d'arret du fil de lecture.
    #[cfg(not(target_arch = "wasm32"))]
    arret_lecteur: Option<Arc<AtomicBool>>,
}

impl Default for UartHostBridge {
    fn default() -> Self {
        let mut bridge = Self {
            port_name: String::new(),
            baud_rate: 460_800,
            is_connected: false,
            bytes_sent: 0,
            bytes_received: 0,
            last_error: None,
            debut_vers_tama: Vec::new(),
            debut_vers_hote: Vec::new(),
            available_ports: Vec::new(),
            host_rx_stream: VecDeque::new(),
            host_tx_stream: VecDeque::new(),
            pending_to_host: VecDeque::new(),
            #[cfg(not(target_arch = "wasm32"))]
            serial: None,
            #[cfg(not(target_arch = "wasm32"))]
            recus: Arc::new(Mutex::new(VecDeque::new())),
            #[cfg(not(target_arch = "wasm32"))]
            arret_lecteur: None,
        };
        bridge.refresh_ports();
        bridge
    }
}

impl UartHostBridge {
    /// Cree un pont en memoire deja connecte. Ce constructeur est destine aux
    /// tests et aux transports personnalises.
    pub fn new(port_name: &str, baud_rate: u32) -> Self {
        Self {
            port_name: port_name.to_string(),
            baud_rate,
            is_connected: true,
            bytes_sent: 0,
            bytes_received: 0,
            last_error: None,
            debut_vers_tama: Vec::new(),
            debut_vers_hote: Vec::new(),
            available_ports: Vec::new(),
            host_rx_stream: VecDeque::new(),
            host_tx_stream: VecDeque::new(),
            pending_to_host: VecDeque::new(),
            #[cfg(not(target_arch = "wasm32"))]
            serial: None,
            #[cfg(not(target_arch = "wasm32"))]
            recus: Arc::new(Mutex::new(VecDeque::new())),
            #[cfg(not(target_arch = "wasm32"))]
            arret_lecteur: None,
        }
    }

    /// Relit les ports serie declares par le systeme.
    pub fn refresh_ports(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match serialport::available_ports() {
                Ok(ports) => {
                    self.available_ports = ports.into_iter().map(|p| p.port_name).collect();
                    self.available_ports.sort();
                    self.available_ports.dedup();
                    if self.port_name.is_empty() && self.available_ports.len() == 1 {
                        self.port_name = self.available_ports[0].clone();
                    }
                }
                Err(e) => self.last_error = Some(format!("Liste des ports indisponible : {e}")),
            }
        }
    }

    /// Ouvre un port hote en 460800 bauds, huit bits, sans parite, un bit
    /// d'arret et sans controle de flux.
    pub fn connect(&mut self, port_name: &str) -> Result<(), String> {
        self.disconnect();
        self.port_name = port_name.trim().to_string();
        self.last_error = None;
        if self.port_name.is_empty() {
            let message = "Choisissez un port serie".to_string();
            self.last_error = Some(message.clone());
            return Err(message);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let port = serialport::new(&self.port_name, self.baud_rate)
                .data_bits(serialport::DataBits::Eight)
                .parity(serialport::Parity::None)
                .stop_bits(serialport::StopBits::One)
                .flow_control(serialport::FlowControl::None)
                // Une milliseconde tronquait les ecritures : le pilote
                // comptait trente deux octets emis quand l'autre bout n'en
                // recevait que vingt deux. La lecture se faisant desormais dans
                // un fil dedie, un delai confortable ne fige plus l'interface.
                .timeout(Duration::from_millis(50))
                .open()
                .map_err(|e| {
                    let message = format!("{} : {e}", self.port_name);
                    self.last_error = Some(message.clone());
                    message
                })?;
            // Un fil dedie vide le port sans discontinuer. Sans lui, plus rien
            // ne lit pendant la tranche d'emulation, le tampon du systeme
            // deborde et il jette des octets en silence : un bloc de quatre
            // mille octets arrivait ampute de quelques dizaines.
            match port.try_clone() {
                Ok(lecture) => {
                    let arret = Arc::new(AtomicBool::new(false));
                    let fanion = Arc::clone(&arret);
                    let file = Arc::clone(&self.recus);
                    std::thread::spawn(move || {
                        let mut lecture = lecture;
                        let mut buf = [0u8; 4096];
                        while !fanion.load(Ordering::Relaxed) {
                            match lecture.read(&mut buf) {
                                Ok(n) if n > 0 => {
                                    if let Ok(mut f) = file.lock() {
                                        f.extend(buf[..n].iter().copied());
                                    }
                                }
                                Ok(_) => {}
                                Err(e) if attente_normale(&e) => {}
                                Err(_) => break,
                            }
                        }
                    });
                    self.arret_lecteur = Some(arret);
                }
                Err(e) => {
                    let message = format!("{} : {e}", self.port_name);
                    self.last_error = Some(message.clone());
                    return Err(message);
                }
            }
            self.serial = Some(port);
            self.is_connected = true;
            return Ok(());
        }

        #[cfg(target_arch = "wasm32")]
        {
            let message =
                "Les ports serie locaux ne sont pas disponibles dans le navigateur".to_string();
            self.last_error = Some(message.clone());
            Err(message)
        }
    }

    /// Ferme le port sans toucher aux donnees deja emises par le controleur.
    pub fn disconnect(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(arret) = self.arret_lecteur.take() {
                arret.store(true, Ordering::Relaxed);
            }
            self.serial = None;
            if let Ok(mut f) = self.recus.lock() {
                f.clear();
            }
        }
        self.is_connected = false;
        self.pending_to_host.clear();
    }

    /// Ecrit des octets du cote hote d'un pont en memoire.
    pub fn host_write(&mut self, bytes: &[u8]) {
        self.host_rx_stream.extend(bytes.iter().copied());
    }

    /// Lit les octets recus par le cote hote d'un pont en memoire.
    pub fn host_read(&mut self) -> Vec<u8> {
        self.host_tx_stream.drain(..).collect()
    }

    /// Synchronise le transport en memoire avec le controleur.
    pub fn sync(&mut self, uart: &mut UartController) {
        if !self.is_connected {
            return;
        }

        let to_host = uart.drain_hote();
        self.bytes_sent += to_host.len();
        self.host_tx_stream.extend(to_host);

        let from_host: Vec<u8> = self.host_rx_stream.drain(..).collect();
        self.bytes_received += from_host.len();
        uart.inject_rx_bytes(&from_host);
    }


    /// Echange les donnees disponibles avec le port serie ouvert. La methode
    /// est bornee et peut etre appelee a chaque image de l'interface.
    pub fn poll_serial(&mut self, uart: &mut UartController) {
        if !self.is_connected {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.pending_to_host.extend(uart.drain_hote());
            let mut erreur_fatale = None;

            if let Some(port) = self.serial.as_mut() {
                // On insiste jusqu'a ce que tout soit parti. Une ecriture
                // partielle laissait derriere elle des reponses tronquees, et
                // l'outil de transfert ne voyait jamais l'acquittement qu'il
                // attendait avant d'envoyer le bloc suivant.
                while !self.pending_to_host.is_empty() && erreur_fatale.is_none() {
                    let donnees = self.pending_to_host.make_contiguous();
                    match port.write(donnees) {
                        Ok(0) => break,
                        Ok(n) => {
                            let envoyes: Vec<u8> =
                                self.pending_to_host.iter().take(n).copied().collect();
                            Self::garder_debut(&mut self.debut_vers_hote, &envoyes);
                            self.pending_to_host.drain(..n);
                            self.bytes_sent += n;
                        }
                        Err(e) if attente_normale(&e) => break,
                        Err(e) => {
                            erreur_fatale = Some(format!("Ecriture {} : {e}", self.port_name))
                        }
                    }
                }
                // Les octets doivent partir tout de suite : l'autre bout attend
                // une reponse pour continuer, il ne relancera rien.
                let _ = port.flush();
            }

            // Les octets releves par le fil de lecture sont remis au
            // controleur. Rien ne se perd entre deux images de l'interface.
            let arrives: Vec<u8> = match self.recus.lock() {
                Ok(mut f) => f.drain(..).collect(),
                Err(_) => Vec::new(),
            };
            if !arrives.is_empty() {
                self.bytes_received += arrives.len();
                Self::garder_debut(&mut self.debut_vers_tama, &arrives);
                uart.inject_rx_bytes(&arrives);
            }

            if let Some(message) = erreur_fatale {
                self.last_error = Some(message);
                self.disconnect();
            }
        }
    }

    /// Transfert direct avec un flux implementant `Read + Write`.
    pub fn sync_with_stream<S: Read + Write>(
        &mut self,
        uart: &mut UartController,
        stream: &mut S,
    ) -> io::Result<()> {
        self.pending_to_host.extend(uart.drain_hote());
        if !self.pending_to_host.is_empty() {
            let donnees = self.pending_to_host.make_contiguous();
            let n = stream.write(donnees)?;
            self.pending_to_host.drain(..n);
            self.bytes_sent += n;
        }

        let mut buf = [0u8; 4096];
        match stream.read(&mut buf) {
            Ok(n) if n > 0 => {
                self.bytes_received += n;
                uart.inject_rx_bytes(&buf[..n]);
            }
            Ok(_) => {}
            Err(e) if attente_normale(&e) => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Retient les premiers octets d'un sens, sans jamais grossir.
    fn garder_debut(trace: &mut Vec<u8>, octets: &[u8]) {
        const MAX: usize = 96;
        if trace.len() >= MAX {
            return;
        }
        let reste = MAX - trace.len();
        trace.extend(octets.iter().take(reste).copied());
    }

    /// Rend une trace en hexadecimal, lisible dans l'interface.
    pub fn trace_hex(octets: &[u8]) -> String {
        octets.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
    }

    pub fn pending_bytes(&self) -> usize {
        self.pending_to_host.len()
    }
}

fn attente_normale(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

impl Drop for UartHostBridge {
    fn drop(&mut self) {
        self.disconnect();
    }
}

pub type UartBridge = UartHostBridge;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pont_memoire_bidirectionnel_sans_perte() {
        let mut bridge = UartHostBridge::new("test", 460_800);
        let mut uart = UartController::new();

        bridge.host_write(&(0..32u8).collect::<Vec<_>>());
        bridge.sync(&mut uart);
        uart.tick(100_000, 96_000_000);
        assert_eq!(uart.rx_fifo.len(), 16);
        assert_eq!(uart.rx_in.len(), 16);

        uart.write_reg(0x00, 0x12);
        uart.write_reg(0x00, 0x34);
        uart.tick(4_200, 96_000_000);
        bridge.sync(&mut uart);
        assert_eq!(bridge.host_read(), vec![0x12, 0x34]);
        assert_eq!(bridge.bytes_sent, 2);
        assert_eq!(bridge.bytes_received, 32);
    }
}
