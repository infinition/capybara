//! Instantanes de la machine : sauvegarde, restauration, et retour en arriere.
//!
//! Un instantane ne recopie pas les seize mega-octets de flash. Elle ne change
//! que lorsque le jeu sauvegarde, quelques pages a peine : on garde une image de
//! reference prise au chargement, et l'instantane ne retient que les pages
//! reellement modifiees depuis. Restaurer consiste alors a remettre chaque page
//! salie soit a sa version de l'instantane, soit a celle de reference.

use std::collections::BTreeMap;

use crate::emulator::cpu::registers::Registers;
use crate::emulator::cpu::Nvic;
use crate::emulator::peripherals::Peripherals;
use crate::emulator::Machine;

/// Taille d'une page de flash suivie, en octets.
pub const PAGE_FLASH: usize = 0x1000;

/// Etat complet de la machine a un instant donne.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Instantane {
    /// Nombre de pas executes depuis le demarrage, pour se reperer.
    pub cycles: u64,
    pub pram: Vec<u8>,
    pub sram: Vec<u8>,
    /// Pages de flash modifiees depuis le chargement, avec leur contenu.
    pub pages_flash: BTreeMap<usize, Vec<u8>>,
    pub regs: Registers,
    pub nvic: Nvic,
    pub is_halted: bool,
    pub periph: Peripherals,
    pub console: String,
    pub is_running: bool,
    /// Dump de flash d'ou vient cet etat.
    ///
    /// Un instantane ne porte que les pages de flash modifiees : il ne veut
    /// rien dire sans son firmware. Le retenir permet de le recharger tout seul
    /// au lieu d'obliger a le faire d'abord a la main.
    #[serde(default)]
    pub firmware: String,
}

impl Machine {
    /// Prend un instantane de l'etat courant.
    pub fn instantane(&self) -> Instantane {
        let mut pages_flash = BTreeMap::new();
        for &page in &self.bus.flash.pages_salies {
            let debut = page * PAGE_FLASH;
            let fin = (debut + PAGE_FLASH).min(self.bus.flash.data.len());
            if debut < fin {
                pages_flash.insert(page, self.bus.flash.data[debut..fin].to_vec());
            }
        }
        Instantane {
            cycles: self.cpu.cycles,
            pram: self.bus.pram.data.clone(),
            sram: self.bus.sram.data.clone(),
            pages_flash,
            regs: self.cpu.regs.clone(),
            nvic: self.cpu.nvic.clone(),
            is_halted: self.cpu.is_halted,
            periph: self.periph.clone(),
            console: self.console.clone(),
            is_running: self.is_running,
            firmware: self.firmware_path.clone().unwrap_or_default(),
        }
    }

    /// Remet la machine dans l'etat d'un instantane.
    ///
    /// Les pages de flash salies depuis le chargement et absentes de
    /// l'instantane reviennent a l'image de reference : sans cela une
    /// sauvegarde faite apres l'instantane survivrait a la restauration.
    pub fn restaurer(&mut self, etat: &Instantane) {
        // La memoire vive change entierement : les adresses de voix relevees
        // avant ne designent plus rien de sur.
        self.voix.clear();
        // L'union des deux ensembles, pas seulement les pages salies par la
        // machine courante. Une machine qui vient d'etre chargee n'a rien sali :
        // ne parcourir que ses pages revenait a ignorer toutes celles de
        // l'instantane, donc a perdre la sauvegarde du jeu et a faire repartir
        // le firmware sur sa premiere mise en route.
        let salies: std::collections::BTreeSet<usize> = self
            .bus
            .flash
            .pages_salies
            .iter()
            .copied()
            .chain(etat.pages_flash.keys().copied())
            .collect();
        for page in salies {
            let debut = page * PAGE_FLASH;
            let fin = (debut + PAGE_FLASH).min(self.bus.flash.data.len());
            if debut >= fin {
                continue;
            }
            match etat.pages_flash.get(&page) {
                Some(contenu) => {
                    let n = contenu.len().min(fin - debut);
                    self.bus.flash.data[debut..debut + n].copy_from_slice(&contenu[..n]);
                }
                None => {
                    if self.bus.flash.reference.len() >= fin {
                        self.bus.flash.data[debut..fin]
                            .copy_from_slice(&self.bus.flash.reference[debut..fin]);
                    }
                }
            }
        }
        self.bus.flash.pages_salies = etat.pages_flash.keys().copied().collect();

        self.bus.pram.data.clone_from(&etat.pram);
        self.bus.sram.data.clone_from(&etat.sram);
        self.cpu.regs = etat.regs.clone();
        self.cpu.nvic = etat.nvic.clone();
        self.cpu.cycles = etat.cycles;
        self.cpu.is_halted = etat.is_halted;
        self.periph = etat.periph.clone();
        self.console.clone_from(&etat.console);
        self.is_running = etat.is_running;
        // L'ecran doit etre repeint, son contenu vient de changer.
        self.periph.display.dirty = true;
    }
}

impl Instantane {
    /// Ecrit l'instantane dans un fichier.
    ///
    /// Le format est du JSON : verbeux pour des tableaux d'octets, mais lisible
    /// et sans dependance de plus. Un etat pese environ un mega-octet, ce qui
    /// reste sans importance pour un outil de mise au point.
    pub fn ecrire(&self, chemin: &std::path::Path) -> Result<(), String> {
        let texte = serde_json::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(chemin, texte).map_err(|e| e.to_string())
    }

    /// Relit un instantane ecrit par `ecrire`.
    pub fn lire(chemin: &std::path::Path) -> Result<Self, String> {
        let texte = std::fs::read_to_string(chemin).map_err(|e| e.to_string())?;
        serde_json::from_str(&texte).map_err(|e| e.to_string())
    }
}

/// Anneau d'instantanes automatiques, pour revenir juste avant un blocage.
///
/// Il ne garde que les derniers : c'est un filet, pas un historique. Chacun
/// coute la SRAM et la PRAM, soit environ deux cents kilo-octets.
pub struct Historique {
    etats: std::collections::VecDeque<Instantane>,
    /// Nombre d'instantanes conserves.
    pub profondeur: usize,
    /// Ecart minimal entre deux prises, en pas executes.
    pub ecart: u64,
    dernier: u64,
}

impl Default for Historique {
    fn default() -> Self {
        Self {
            etats: std::collections::VecDeque::new(),
            profondeur: 12,
            // Two seconds of console time. The counter is in emulated cycles
            // and the console runs ninety-six million per second, so the
            // previous fifty million took a snapshot twice a second — each one
            // copying RAM, program memory and every peripheral. That is a
            // regular hitch for a safety net that need not be so fine.
            ecart: 192_000_000,
            dernier: 0,
        }
    }
}

impl Historique {
    /// Prend un instantane si l'ecart est atteint.
    pub fn suivre(&mut self, machine: &Machine) {
        let cycles = machine.cpu.cycles;
        if cycles < self.dernier.saturating_add(self.ecart) && !self.etats.is_empty() {
            return;
        }
        self.dernier = cycles;
        if self.etats.len() >= self.profondeur {
            self.etats.pop_front();
        }
        self.etats.push_back(machine.instantane());
    }

    /// Rend le dernier instantane et l'ote de l'anneau.
    pub fn reculer(&mut self) -> Option<Instantane> {
        // Le dernier est souvent tout proche du present : on rend l'avant
        // dernier quand il existe, pour reellement revenir en arriere.
        if self.etats.len() >= 2 {
            self.etats.pop_back();
        }
        let etat = self.etats.pop_back();
        if let Some(e) = &etat {
            self.dernier = e.cycles;
        }
        etat
    }

    pub fn len(&self) -> usize {
        self.etats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.etats.is_empty()
    }

    pub fn vider(&mut self) {
        self.etats.clear();
        self.dernier = 0;
    }
}
