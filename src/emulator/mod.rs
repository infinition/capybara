pub mod aes;
pub mod cpu;
pub mod etat;
pub mod loader;
pub mod mmu;
pub mod peripherals;
pub mod sonix;

pub use cpu::{Cpu, DisassembledInst, Disassembler, Mode, Registers, StepResult};
pub use loader::{FirmwareLoader, ImageKind, LoadReport, LoadedRegion};
pub use mmu::{BootRom, InternalSram, LogEntry, MemoryBus, MmioStat, MmioTrace, Pram, SpiFlash};
pub use peripherals::{
    DisplayController, FuseRegisters, GpioController, Peripherals, SysRegisters, Timers,
    UartController,
};

use std::collections::HashSet;
use std::path::Path;

/// Raison pour laquelle l'execution s'est arretee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    Breakpoint(u32),
    Halted(u32),
    /// Instruction non decodee : l'emulateur ne sait pas executer ce code.
    Undefined { pc: u32, opcode: u32 },
}

pub struct Machine {
    pub cpu: Cpu,
    pub bus: MemoryBus,
    pub periph: Peripherals,
    pub breakpoints: HashSet<u32>,
    pub is_running: bool,
    pub instructions_per_frame: u32,
    /// Console de debug du firmware, telle qu'elle sortirait sur l'UART.
    ///
    /// Dans la boucle de formatage du printf, l'instruction 0x00001070 appelle
    /// la sortie avec le caractere dans r0. L'intercepter donne le journal
    /// complet sans modeliser le port serie.
    pub console: String,
    pub firmware_path: Option<String>,
    /// Cle de la puce, indispensable pour dechiffrer un dump chiffre.
    pub device_key: Option<u32>,
    pub last_report: Option<LoadReport>,
    pub last_stop: Option<StopReason>,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn new() -> Self {
        let mut bus = MemoryBus::default();
        let mut periph = Peripherals::default();
        FirmwareLoader::install_idle_state(&mut bus);

        let mut cpu = Cpu::default();
        cpu.reset(&mut bus, &mut periph);

        Self {
            cpu,
            bus,
            periph,
            breakpoints: HashSet::new(),
            // Sans firmware charge, rien ne tourne.
            is_running: false,
            instructions_per_frame: 20_000,
            console: String::new(),
            firmware_path: None,
            device_key: None,
            last_report: None,
            last_stop: None,
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus, &mut self.periph);
        self.last_stop = None;
    }

    pub fn step(&mut self) -> StepResult {
        self.cpu.step(&mut self.bus, &mut self.periph)
    }

    pub fn run_frame(&mut self) -> StepResult {
        if !self.is_running {
            return StepResult::Halt;
        }

        let mut executed = 0;
        while executed < self.instructions_per_frame {
            let pc = self.cpu.regs.pc;
            if self.breakpoints.contains(&pc) {
                self.is_running = false;
                self.last_stop = Some(StopReason::Breakpoint(pc));
                return StepResult::Breakpoint;
            }
            if pc == Self::SORTIE_CONSOLE {
                let c = (self.cpu.regs.get_reg(0) & 0xFF) as u8;
                if c == 10 || (0x20..0x7F).contains(&c) {
                    self.console.push(c as char);
                }
                // Le journal ne sert qu'au diagnostic : on borne sa taille.
                if self.console.len() > 8000 {
                    let reste = self.console.split_off(self.console.len() - 4000);
                    self.console = reste;
                }
            }

            match self.cpu.step(&mut self.bus, &mut self.periph) {
                StepResult::Ok(_) => executed += 1,
                StepResult::Breakpoint => {
                    self.is_running = false;
                    self.last_stop = Some(StopReason::Breakpoint(pc));
                    return StepResult::Breakpoint;
                }
                StepResult::Halt => {
                    self.is_running = false;
                    self.last_stop = Some(StopReason::Halted(pc));
                    return StepResult::Halt;
                }
                // Une instruction non decodee fausse tout ce qui suit. On s'arrete
                // au lieu de continuer sur un etat de registres devenu faux.
                StepResult::Undefined(op) => {
                    self.is_running = false;
                    self.last_stop = Some(StopReason::Undefined { pc, opcode: op as u32 });
                    return StepResult::Undefined(op);
                }
            }
        }

        // L'afficheur n'est plus recopie depuis la SRAM : il recoit les trames
        // que le controleur de transferts lui pousse, comme sur la console.
        StepResult::Ok(executed)
    }

    /// Charge un dump et prepare le demarrage du vrai firmware.
    /// Adresses des deux pages de sauvegarde, principale puis copie.
    pub const PAGES_SAUVEGARDE: [usize; 2] = [0xEFE000, 0xEFF000];
    /// Longueur d'une page de sauvegarde, en-tete compris.
    pub const TAILLE_PAGE_SAUVEGARDE: usize = 0x1000;
    /// Polynome de la somme de controle des pages de sauvegarde, celui que le
    /// firmware programme dans l'accelerateur en 0x1000569E.
    pub const POLYNOME_SAUVEGARDE: u16 = 0xA001;
    /// Drapeau de pile faible, bit 3 du premier octet de l'etat sauvegarde.
    ///
    /// Le firmware le lit en 0x10030E54, imprime
    /// "** LOW BATTERY FLAG DETECTED **" et passe a l'etat 111, qui affiche
    /// "remplacez la pile" puis eteint la console. Le dump d'origine porte ce
    /// drapeau : la console etait en fin de pile au moment de l'extraction.
    pub const DRAPEAU_PILE_FAIBLE: u8 = 1 << 3;

    /// Somme de controle d'une page de sauvegarde, sur ses 0xFFC octets utiles.
    fn somme_sauvegarde(&self, page: usize) -> u16 {
        let mut crc: u16 = 0;
        for i in 4..Self::TAILLE_PAGE_SAUVEGARDE {
            crc ^= self.bus.flash.read_u8(page + i) as u16;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ Self::POLYNOME_SAUVEGARDE
                } else {
                    crc >> 1
                };
            }
        }
        crc
    }

    /// Efface le drapeau de pile faible des deux pages de sauvegarde et remet
    /// leur en-tete d'accord avec le contenu.
    ///
    /// C'est l'equivalent exact du geste physique : sans cela le firmware
    /// affiche son message de pile a remplacer et s'eteint, quel que soit le
    /// reste du modele.
    pub fn remplacer_la_pile(&mut self) {
        for page in Self::PAGES_SAUVEGARDE {
            let etat = self.bus.flash.read_u8(page + 4);
            if etat & Self::DRAPEAU_PILE_FAIBLE == 0 {
                continue;
            }
            self.bus.flash.write_u8(page + 4, etat & !Self::DRAPEAU_PILE_FAIBLE);
            let somme = self.somme_sauvegarde(page);
            self.bus.flash.write_u8(page, (somme & 0xFF) as u8);
            self.bus.flash.write_u8(page + 1, (somme >> 8) as u8);
            let complement = !somme;
            self.bus.flash.write_u8(page + 2, (complement & 0xFF) as u8);
            self.bus.flash.write_u8(page + 3, (complement >> 8) as u8);
        }
    }

    /// Instruction qui appelle la sortie caractere du printf de debug, avec le
    /// caractere dans r0.
    pub const SORTIE_CONSOLE: u32 = 0x0000_1070;

    /// Boutons de la console, avec l'identifiant que le firmware leur donne :
    /// port dans les bits hauts, broche dans les quatre bits bas.
    pub const BOUTON_MOLETTE: u32 = 0x08;
    pub const BOUTON_A: u32 = 0x09;
    pub const BOUTON_C: u32 = 0x0A;
    pub const BOUTON_B: u32 = 0x0B;
    pub const ENCODEUR_1: u32 = 0x20;
    pub const ENCODEUR_2: u32 = 0x21;

    /// Port correspondant a un identifiant de broche, s'il est modelise.
    fn port_de(&mut self, id: u32) -> Option<&mut crate::emulator::peripherals::GpioPort> {
        match id >> 4 {
            0 => Some(&mut self.periph.port0),
            1 => Some(&mut self.periph.port1),
            2 => Some(&mut self.periph.port2),
            _ => None,
        }
    }

    /// Boucle de veille profonde du firmware, en PRAM.
    ///
    /// Elle demande la mise hors tension du coeur par le bit 0 de
    /// `0x45000300`, execute un `WFI`, puis se rebranche sur elle meme sans
    /// aucune condition de sortie : le saut de `0x00002432` vers `0x000023D0`
    /// est inconditionnel, et les deux seules interruptions restees autorisees,
    /// 2 et 3, ont des gestionnaires qui reviennent dans la boucle. Aucune
    /// sortie logicielle n'existe donc, et le reveil ne peut venir que du
    /// materiel, qui remet le coeur a zero. C'est ce que reproduit `appuyer`.
    pub const VEILLE_PROFONDE: std::ops::Range<u32> = 0x0000_23D0..0x0000_2434;

    /// Vrai quand le coeur est gare dans cette boucle.
    pub fn en_veille_profonde(&self) -> bool {
        Self::VEILLE_PROFONDE.contains(&self.cpu.regs.pc)
    }

    /// Tire une broche vers le bas, ce que fait un appui.
    ///
    /// Les entrees sont a resistance de tirage : au repos elles se lisent
    /// hautes, un appui les tire bas. C'est la convention que le firmware
    /// attend, verifiee sur les broches 0x20 et 0x21 de l'encodeur.
    ///
    /// En veille profonde, l'appui ne tire pas seulement la broche : il rallume
    /// la console. La memoire vive est effacee par le demarrage du firmware,
    /// mais la sauvegarde est en flash et l'horloge continue de tourner, donc
    /// la partie reprend la ou elle en etait.
    pub fn appuyer(&mut self, id: u32) {
        if self.en_veille_profonde() {
            self.reset();
            self.is_running = true;
            return;
        }
        let broche = id & 0xF;
        if let Some(port) = self.port_de(id) {
            port.appuyer(broche);
        }
    }

    /// Relache une broche, qui remonte par sa resistance de tirage.
    pub fn relacher(&mut self, id: u32) {
        let broche = id & 0xF;
        if let Some(port) = self.port_de(id) {
            port.relacher(broche);
        }
    }

    pub fn load_firmware_file<P: AsRef<Path>>(&mut self, path: P) -> Result<LoadReport, String> {
        let p = path.as_ref();
        let report = FirmwareLoader::load_flash_dump(&mut self.bus, p, self.device_key)?;

        self.firmware_path = Some(p.to_string_lossy().to_string());
        // Le firmware peut relire la cle dans les fusibles, comme sur la puce.
        self.periph.fuses.device_key = self.device_key;
        self.bus.mmio_trace.clear();
        self.bus.mmio_trace.enabled = true;

        // L'image chargee sert de fond aux instantanes.
        self.bus.flash.figer_reference();

        self.reset();
        self.is_running = report.bootable;
        // L'afficheur n'est plus recopie depuis la SRAM : il recoit les trames
        // que le controleur de transferts lui pousse, comme sur la console.
        self.last_report = Some(report.clone());
        Ok(report)
    }

    pub fn get_disassembly_window(&mut self, count: usize) -> Vec<DisassembledInst> {
        self.get_disassembly_at(self.cpu.regs.pc, count)
    }

    pub fn get_disassembly_at(&mut self, start_addr: u32, count: usize) -> Vec<DisassembledInst> {
        let mut list = Vec::new();
        let mut cur_pc = start_addr;

        for _ in 0..count {
            let w1 = self.bus.read_u16(cur_pc, &mut self.periph, &self.cpu.nvic);
            let w2 = self.bus.read_u16(cur_pc + 2, &mut self.periph, &self.cpu.nvic);
            let inst = Disassembler::disassemble(cur_pc, &[w1, w2]);
            let advance = if inst.is_32bit { 4 } else { 2 };
            list.push(inst);
            cur_pc = cur_pc.wrapping_add(advance);
        }

        list
    }
}
