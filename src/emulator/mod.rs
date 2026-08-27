pub mod aes;
pub mod cpu;
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

        self.periph.display.sync_from_sram(&self.bus.sram.data);
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

    pub fn load_firmware_file<P: AsRef<Path>>(&mut self, path: P) -> Result<LoadReport, String> {
        let p = path.as_ref();
        let report = FirmwareLoader::load_flash_dump(&mut self.bus, p, self.device_key)?;

        self.firmware_path = Some(p.to_string_lossy().to_string());
        // Le firmware peut relire la cle dans les fusibles, comme sur la puce.
        self.periph.fuses.device_key = self.device_key;
        self.bus.mmio_trace.clear();
        self.bus.mmio_trace.enabled = true;

        self.reset();
        self.is_running = report.bootable;
        self.periph.display.sync_from_sram(&self.bus.sram.data);
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
