pub mod disasm;
pub mod nvic;
pub mod registers;
pub mod thumb16;
pub mod thumb32;

pub use disasm::{DisassembledInst, Disassembler};
pub use nvic::Nvic;
pub use registers::{Mode, Registers};
pub use thumb16::{StepResult, Thumb16};
pub use thumb32::Thumb32;

use crate::emulator::mmu::MemoryBus;
use crate::emulator::peripherals::Peripherals;

pub struct Cpu {
    pub regs: Registers,
    pub nvic: Nvic,
    pub cycles: u64,
    pub is_halted: bool,
    /// Cycles pas encore distribues aux peripheriques.
    ///
    /// Les entretenir a chaque instruction coutait sept appels par pas, dont
    /// une division en soixante quatre bits pour le signal de trame. Or rien ne
    /// se joue en dessous de quelques microsecondes : le SysTick compte 96000
    /// cycles, la demi periode de trame 800000. On les regroupe donc, ce qui ne
    /// change rien a ce que le firmware observe et rend le coeur nettement plus
    /// rapide.
    cycles_en_attente: u32,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            regs: Registers::default(),
            nvic: Nvic::default(),
            cycles: 0,
            is_halted: false,
            cycles_en_attente: 0,
        }
    }

    pub fn reset(&mut self, bus: &mut MemoryBus, periph: &mut Peripherals) {
        self.regs = Registers::default();
        self.cycles = 0;
        self.is_halted = false;
        self.cycles_en_attente = 0;

        // Fetch initial SP from 0x00000000 / VTOR
        let sp = bus.read_u32(self.nvic.vtor, periph, &self.nvic);
        // Fetch initial PC (Reset Vector) from 0x00000004 / VTOR + 4
        let pc = bus.read_u32(self.nvic.vtor + 4, periph, &self.nvic);

        self.regs.msp = sp;
        self.regs.pc = pc & !1; // Clear Thumb bit for address
    }

    pub fn step(&mut self, bus: &mut MemoryBus, periph: &mut Peripherals) -> StepResult {
        if self.is_halted {
            return StepResult::Halt;
        }

        let pc = self.regs.pc;

        // Retour d'exception. Le coeur ne branche pas vraiment vers 0xFFFFFFFx :
        // cette valeur placee dans LR a l'entree du handler demande la
        // restauration du contexte empile.
        if pc >= 0xFFFF_FFF0 {
            if self.regs.mode == Mode::Handler {
                self.exception_return(bus, periph);
                return StepResult::Ok(1);
            }
            self.is_halted = true;
            return StepResult::Halt;
        }
        if pc == 0 {
            self.is_halted = true;
            return StepResult::Halt;
        }

        // Exceptions en attente. On ne les prend que depuis le mode Thread :
        // sans modele de priorites, autoriser la preemption d'un handler par un
        // autre empilerait indefiniment.
        if self.nvic.en_attente && self.regs.mode == Mode::Thread && self.regs.primask == 0 {
            if self.nvic.systick_pending {
                self.nvic.systick_pending = false;
                self.enter_exception(Nvic::SYSTICK_EXCEPTION, bus, periph);
                return StepResult::Ok(1);
            }
            if let Some(irq) = self.nvic.get_highest_pending_irq() {
                self.enter_exception(irq + 16, bus, periph);
                return StepResult::Ok(1);
            }
            // Rien a prendre, et on etait en etat de le prendre : inutile de
            // regarder a nouveau tant que rien n'est demande.
            self.nvic.en_attente = false;
        }

        // La trace MMIO attribue chaque acces a l'instruction qui le provoque.
        bus.current_pc = pc;
        // Les deux demi mots viennent d'une seule resolution de region. Le
        // chemin general reste la pour le code qui ne serait ni en PRAM ni dans
        // la fenetre XIP, ce qui n'arrive pas en fonctionnement normal.
        let (w1, w2_lu) = match bus.fetch_pair(pc, periph) {
            Some(paire) => paire,
            None => {
                let premier = bus.read_u16(pc, periph, &self.nvic);
                // Le second demi mot n'est lu que s'il existe : hors des deux
                // memoires de code, une lecture de plus pourrait tomber sur un
                // registre et fausser la trace.
                let second = if Self::est_longue(premier) {
                    bus.read_u16(pc.wrapping_add(2), periph, &self.nvic)
                } else {
                    0
                };
                (premier, second)
            }
        };

        let is_32 = Self::est_longue(w1);
        let w2 = if is_32 {
            self.regs.pc = self.regs.pc.wrapping_add(4);
            w2_lu
        } else {
            self.regs.pc = self.regs.pc.wrapping_add(2);
            0
        };

        // Bloc IT : l'instruction courante est conditionnee par ITSTATE[7:4].
        // On fait avancer l'etat avant d'executer, car une instruction IT ne peut
        // pas elle-meme se trouver dans un bloc.
        if (self.regs.itstate & 0x0F) != 0 {
            let cond = ((self.regs.itstate >> 4) & 0xF) as u16;
            let taken = Thumb16::eval_condition(cond, &self.regs);
            self.advance_itstate();
            if !taken {
                self.cycles += 1;
                return StepResult::Ok(1);
            }
        }

        let result = if is_32 {
            Thumb32::execute(w1, w2, &mut self.regs, bus, periph, &mut self.nvic)
        } else {
            Thumb16::execute(w1, &mut self.regs, bus, periph, &mut self.nvic)
        };

        match result {
            StepResult::Ok(c) => {
                self.cycles += c as u64;
                // Le bus realise la copie du controleur de transferts mais ne
                // voit pas le NVIC : la fin de transfert se signale ici. Elle
                // reste hors du regroupement, un simple drapeau ne coutant rien
                // et l'ecran attendant cette interruption au plus tot.
                if periph.dma.irq_a_lever {
                    periph.dma.irq_a_lever = false;
                    self.nvic.request_irq(crate::emulator::peripherals::dma::IRQ);
                }
                self.cycles_en_attente += c as u32;
                if self.cycles_en_attente >= Self::GRAIN_PERIPHERIQUES {
                    let ecoules = self.cycles_en_attente;
                    self.cycles_en_attente = 0;
                    self.entretenir_peripheriques(ecoules, periph);
                }
                StepResult::Ok(c)
            }
            StepResult::Breakpoint => StepResult::Breakpoint,
            StepResult::Halt => {
                self.is_halted = true;
                StepResult::Halt
            }
            StepResult::Undefined(op) => StepResult::Undefined(op),
        }
    }

    /// Vrai pour le premier demi mot d'une instruction de 32 bits.
    #[inline(always)]
    fn est_longue(w: u16) -> bool {
        (w & 0xF800) == 0xE800 || (w & 0xF800) == 0xF000 || (w & 0xF800) == 0xF800
    }

    /// Grain d'entretien des peripheriques, en cycles.
    ///
    /// Deux cent cinquante six cycles valent moins de trois microsecondes a
    /// 96 MHz, cent fois plus fin que la plus courte echeance du firmware.
    const GRAIN_PERIPHERIQUES: u32 = 256;

    /// Fait avancer tout ce qui vit au rythme des cycles.
    fn entretenir_peripheriques(&mut self, ecoules: u32, periph: &mut Peripherals) {
        // Le SysTick pose lui-meme son drapeau d'attente : c'est une exception
        // systeme, pas une IRQ externe a inscrire dans ISPR.
        self.nvic.tick_systick(ecoules);
        // Le TE de l'ecran est entretenu ici : c'est un signal exterieur, sans
        // quoi le firmware l'attend sans fin.
        if periph.port1.tick(ecoules) {
            self.nvic.request_irq(crate::emulator::peripherals::gpio_port::PORT1_IRQ);
        }
        if let Some(irq) = periph.tic.tick(ecoules) {
            self.nvic.request_irq(irq);
        }
        if periph.adc_pile.irq_a_lever | periph.adc_pile.tick(ecoules) {
            periph.adc_pile.irq_a_lever = false;
            self.nvic.request_irq(crate::emulator::peripherals::adc_pile::IRQ);
        }
        if periph.timers.tick(ecoules) {
            self.nvic.request_irq(16);
        }
        if periph.uart.irq_pending {
            self.nvic.request_irq(37);
        }
        // Le compteur de secondes de la zone systeme. C'est la seule source de
        // temps du calendrier du jeu : sans lui la date reste sur celle qui a
        // ete reglee, et rien ne vieillit. Son alarme est ce qui sort la console
        // de sa veille profonde.
        periph.snsys.tick(ecoules);
    }

    /// ITAdvance : le masque est decale d'un cran, et le bloc se termine quand
    /// les trois bits de poids faible sont nuls.
    fn advance_itstate(&mut self) {
        if (self.regs.itstate & 0x07) == 0 {
            self.regs.itstate = 0;
        } else {
            let low = (self.regs.itstate & 0x1F) << 1;
            self.regs.itstate = (self.regs.itstate & 0xE0) | (low & 0x1F);
        }
    }

    fn enter_exception(&mut self, exception_num: u32, bus: &mut MemoryBus, periph: &mut Peripherals) {
        let mut sp = self.regs.get_sp();

        // L'etat du bloc IT voyage dans le xPSR empile, aux places que lui donne
        // l'architecture : bits 26:25 pour ses deux bits bas, 15:10 pour les six
        // hauts. Sans cela une exception prise entre un IT et son instruction
        // conditionnelle laisse le gestionnaire heriter de la condition, et sa
        // premiere instruction est sautee. C'est ainsi que le gestionnaire du TE
        // perdait le PUSH de son adresse de retour et ne revenait jamais.
        let it = self.regs.itstate as u32;
        let xpsr_empile = (self.regs.xpsr & !0x0600_FC00)
            | ((it & 0x3) << 25)
            | (((it >> 2) & 0x3F) << 10);

        // Stack frame: R0, R1, R2, R3, R12, LR, ReturnAddress (PC), xPSR
        let frame = [
            self.regs.get_reg(0),
            self.regs.get_reg(1),
            self.regs.get_reg(2),
            self.regs.get_reg(3),
            self.regs.get_reg(12),
            self.regs.lr,
            self.regs.pc,
            xpsr_empile,
        ];

        for &val in frame.iter().rev() {
            sp -= 4;
            bus.write_u32(sp, val, periph, &mut self.nvic);
        }
        self.regs.set_sp(sp);

        self.regs.lr = 0xFFFF_FFF9; // Return to Thread mode with Main Stack
        self.regs.mode = Mode::Handler;
        // Le gestionnaire demarre hors de tout bloc IT.
        self.regs.itstate = 0;

        let handler_addr = bus.read_u32(self.nvic.vtor + exception_num * 4, periph, &self.nvic);
        self.regs.pc = handler_addr & !1;
        // Seules les IRQ externes ont un bit dans ISPR. Acquitter une exception
        // systeme via ce chemin effacerait le bit d'une IRQ sans rapport.
        if exception_num >= 16 {
            self.nvic.acknowledge_irq(exception_num - 16);
        }
    }

    /// Restaure le contexte empile par `enter_exception` et rend la main au
    /// code interrompu.
    fn exception_return(&mut self, bus: &mut MemoryBus, periph: &mut Peripherals) {
        let mut sp = self.regs.get_sp();
        let mut pop = |bus: &mut MemoryBus, periph: &mut Peripherals, nvic: &Nvic| {
            let v = bus.read_u32(sp, periph, nvic);
            sp += 4;
            v
        };

        let r0 = pop(bus, periph, &self.nvic);
        let r1 = pop(bus, periph, &self.nvic);
        let r2 = pop(bus, periph, &self.nvic);
        let r3 = pop(bus, periph, &self.nvic);
        let r12 = pop(bus, periph, &self.nvic);
        let lr = pop(bus, periph, &self.nvic);
        let ret = pop(bus, periph, &self.nvic);
        let xpsr = pop(bus, periph, &self.nvic);

        self.regs.set_reg(0, r0);
        self.regs.set_reg(1, r1);
        self.regs.set_reg(2, r2);
        self.regs.set_reg(3, r3);
        self.regs.set_reg(12, r12);
        self.regs.lr = lr;
        self.regs.xpsr = xpsr;
        // Le bloc IT interrompu reprend la ou il en etait.
        self.regs.itstate =
            ((((xpsr >> 25) & 0x3) | (((xpsr >> 10) & 0x3F) << 2)) & 0xFF) as u8;
        self.regs.mode = Mode::Thread;
        self.regs.set_sp(sp);
        self.regs.pc = ret & !1;
    }
}
