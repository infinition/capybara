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
    /// Head address of the last tight loop crossed.
    ///
    /// `u32::MAX` means "none": it is not a reachable code address.
    boucle_tete: u32,
    /// Register state at the last pass through this loop's head.
    boucle_regs: Registers,
    /// Stack pointer of the tracked loop.
    ///
    /// A deeper loop — lower pointer — runs inside a function called from the
    /// tracked one. It is not allowed to take its place: the firmware's wait
    /// loop calls a function on every iteration, and the slightest backward
    /// branch in that function replaced the tracked head, so the wait was never
    /// compared against itself. That is exactly what `boucle_probe` showed: two
    /// thirds of the time in a loop with identical registers, and not one
    /// skip.
    boucle_sp: u32,
    /// Cycle count at the last pass through the tracked loop's head.
    ///
    /// Protection against deeper loops only holds while the tracked loop is
    /// still running: between two passes through its head it may perfectly well
    /// call a function containing loops of its own. But a short loop seen once
    /// in a high-level function and then left must not forbid tracking anything
    /// below it forever — that is how the TE wait, three call levels under the
    /// main loop, stayed invisible.
    boucle_vu_a: u64,
    /// Iterations of this loop where the registers differed.
    ///
    /// Past `ECHECS_MAX` the loop is judged active and we stop watching it
    /// until it is left: comparing and copying the registers is therefore paid
    /// a handful of times per loop, not on every iteration of a memcpy.
    boucle_echecs: u8,
    /// Iterations elapsed since a loop was judged active.
    ///
    /// The verdict is not final: a loop may work for a while and then start
    /// waiting. Without re-examination, three unlucky iterations at the start —
    /// three SysTick interrupts, say — condemned a whole wait of several
    /// thousand iterations.
    boucle_recul: u16,
    /// Cycles gained by the fast-forward, and the number of skips. For the
    /// diagnostic: it is the only way to tell whether the firmware spends its
    /// time waiting, and therefore whether the optimisation is worth anything.
    pub cycles_sautes: u64,
    pub sauts: u64,
    /// Fast-forward switch.
    ///
    /// Idle recognition is conservative but it remains a heuristic: if a loop
    /// polled a register whose read has a side effect, skipping it would change
    /// behaviour. Clearing `repos_permis` puts the core back in its old
    /// step-exact mode, which settles any doubt in two runs; the tests do so
    /// through `Cpu::step_exact`.
    ///
    /// The interface also clears it on its own while a serial link is open. The
    /// whole point of the fast-forward is to trade *when the firmware notices*
    /// for speed, by up to `SAUT_MAXIMUM` cycles. That trade is free while the
    /// console only talks to itself; it is not free when a transfer protocol on
    /// the other end of a wire is counting the milliseconds. See
    /// `repos_permis`.
    pub repos_actif: bool,
    /// What `repos_actif` should return to once nothing forbids it.
    ///
    /// Kept apart so the interface can switch the fast-forward off and back on
    /// without losing the choice made at construction.
    pub repos_permis: bool,
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
            boucle_tete: u32::MAX,
            boucle_regs: Registers::default(),
            boucle_sp: 0,
            boucle_vu_a: 0,
            boucle_echecs: 0,
            boucle_recul: 0,
            cycles_sautes: 0,
            sauts: 0,
            repos_actif: true,
            repos_permis: true,
        }
    }

    /// Turns the idle fast-forward off for good: every instruction is then
    /// executed, exactly as before the optimisation existed.
    pub fn step_exact(&mut self) {
        self.repos_actif = false;
        self.repos_permis = false;
    }

    pub fn reset(&mut self, bus: &mut MemoryBus, periph: &mut Peripherals) {
        self.regs = Registers::default();
        self.cycles = 0;
        self.is_halted = false;
        self.cycles_en_attente = 0;
        self.boucle_tete = u32::MAX;
        self.boucle_regs = Registers::default();
        self.boucle_sp = 0;
        self.boucle_vu_a = 0;
        self.boucle_echecs = 0;
        self.boucle_recul = 0;
        self.cycles_sautes = 0;
        self.sauts = 0;

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

        // WFI and WFE. The decoder treated them as NOP: the core went straight
        // back into the firmware's wait loop and interpreted it at full speed
        // to do nothing. These two forms say exactly "nothing will happen until
        // a peripheral speaks" — the one place in the model where we know for
        // certain the clock can be advanced without executing.
        if self.repos_actif && (w1 == 0xBF30 || w1 == 0xBF20) {
            self.cycles += 1;
            self.cycles_en_attente += 1;
            let _ = self.sauter_le_temps(periph);
            return StepResult::Ok(1);
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
                    self.nvic
                        .request_irq(crate::emulator::peripherals::dma::IRQ);
                }
                self.cycles_en_attente += c as u32;
                if self.cycles_en_attente >= Self::GRAIN_PERIPHERIQUES {
                    let ecoules = self.cycles_en_attente;
                    self.cycles_en_attente = 0;
                    self.entretenir_peripheriques(ecoules, periph);
                }
                self.guetter_le_repos(pc, bus, periph);
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

    /// Longest backward branch still treated as a wait loop, in bytes.
    ///
    /// A polling loop fits in a few instructions: call or read, test a bit, go
    /// back. Sixty-four bytes leave room for two or three register reads and
    /// their tests. Beyond that it is real work, and `ECHECS_MAX` bounds what a
    /// working loop costs if we watched it for nothing anyway.
    const PORTEE_BOUCLE: u32 = 64;

    /// Longest fast-forward, in cycles.
    ///
    /// Eight thousand one hundred and ninety-two cycles are about eighty-five
    /// microseconds, roughly one byte's time at 115200 baud and a hundredth of
    /// the firmware's shortest deadline. That is the most the firmware can lag
    /// in noticing that a polled register changed, since the loop only rereads
    /// it once the skip is over. The peripherals are serviced at the usual
    /// granularity throughout: their progress is not approximated, only the
    /// observation of it is.
    const SAUT_MAXIMUM: u32 = 8192;

    /// Consecutive iterations with differing registers before giving up on a
    /// loop.
    ///
    /// A wait loop is recognised on the second iteration; a working loop never
    /// will be. Three iterations settle it, and bound what the watching costs a
    /// ten-thousand-iteration memcpy.
    const ECHECS_MAX: u8 = 3;

    /// Iterations to let pass before re-examining a loop judged active.
    ///
    /// Rare enough that comparing registers costs nothing on a working loop —
    /// one iteration in sixty-four — and frequent enough that a wait misjudged
    /// at the start is picked up after a few microseconds rather than never.
    const RECUL: u16 = 64;

    /// How long, in cycles, a tracked loop holds its place against deeper
    /// loops.
    ///
    /// One iteration of the TE wait is a hundred and forty cycles, call
    /// included; two thousand leave ample margin. A loop that has not come back
    /// through its head for longer than that is no longer running: it has given
    /// up control, and whatever runs below can be tracked.
    const PROTECTION: u64 = 2048;

    /// Recognises a wait loop and, where applicable, advances the clock
    /// instead of interpreting it.
    ///
    /// The firmware spends most of its time waiting: it does one frame's work,
    /// then spins in place until the next signal from the display or the
    /// SysTick. Interpreting that loop at full speed amounts to emulating doing
    /// nothing, conscientiously.
    ///
    /// Recognition is conservative. Three things are needed together: a short
    /// backward branch, no store to memory during the whole iteration, and
    /// registers strictly identical to the previous pass through the loop head.
    /// An iteration meeting all three can produce no observable effect: the
    /// next will do exactly the same, and will keep doing so until a peripheral
    /// changes something. We can therefore jump straight to that moment.
    ///
    /// The cost is bounded per loop, not per iteration. The hot half, here,
    /// does only two address comparisons and folds into `step`; everything else
    /// lives in `observer_la_boucle`, out of line, reached only on a short
    /// backward branch. An early version compared and copied the registers on
    /// every iteration of every short loop, memcpy included: it taxed the whole
    /// firmware to recognise its waits alone.
    #[inline(always)]
    fn guetter_le_repos(&mut self, pc_avant: u32, bus: &mut MemoryBus, periph: &mut Peripherals) {
        let cible = self.regs.pc;
        if cible >= pc_avant || pc_avant - cible > Self::PORTEE_BOUCLE {
            return;
        }
        self.observer_la_boucle(cible, bus, periph);
    }

    /// Cold half of the detector: an iteration of a short loop headed at
    /// `cible` has just closed.
    ///
    /// The order of the tests matters. A loop not yet tracked is recorded
    /// without looking at the write flag: that flag was filled with no stack
    /// floor in place, and it is true of the PUSH of any function called. An
    /// early version consulted it first, and so never recorded a loop that
    /// calls anything — which is to say the TE wait, which calls its pin read
    /// on every iteration. The flag only means something from the second pass
    /// on, once the floor is set.
    #[inline(never)]
    fn observer_la_boucle(&mut self, cible: u32, bus: &mut MemoryBus, periph: &mut Peripherals) {
        if !self.repos_actif {
            return;
        }
        let sp = self.regs.get_sp();
        let vivante = self.cycles.wrapping_sub(self.boucle_vu_a) < Self::PROTECTION;
        if self.boucle_tete != u32::MAX && vivante && sp < self.boucle_sp {
            // Inner loop of a tracked loop that is still running: let it pass
            // untouched, and without consuming the write flag, which belongs
            // to the outer loop.
            return;
        }
        if self.boucle_tete != cible || !vivante {
            // New loop, or return to one left long ago: start over. The floor
            // is set now; the flag accumulated so far means nothing and is
            // cleared.
            self.boucle_tete = cible;
            self.boucle_sp = sp;
            self.boucle_vu_a = self.cycles;
            self.boucle_echecs = 0;
            self.boucle_recul = 0;
            bus.plancher_pile = sp;
            bus.a_ecrit = false;
            let etat = self.regs.clone();
            self.boucle_regs = etat;
            return;
        }
        self.boucle_vu_a = self.cycles;
        if self.boucle_echecs >= Self::ECHECS_MAX {
            // Loop judged active. We do not give up for good: now and then we
            // retake the state and give the next iteration a chance. The frame
            // wait lasts several thousand iterations; three interrupts landing
            // at the start condemned all of it.
            bus.a_ecrit = false;
            self.boucle_recul += 1;
            if self.boucle_recul >= Self::RECUL {
                self.boucle_recul = 0;
                self.boucle_echecs = Self::ECHECS_MAX - 1;
                self.boucle_sp = sp;
                bus.plancher_pile = sp;
                let etat = self.regs.clone();
                self.boucle_regs = etat;
            }
            return;
        }
        let a_ecrit = bus.a_ecrit;
        bus.a_ecrit = false;
        if a_ecrit {
            // An iteration that stores above the floor is not idle. But it may
            // be an interrupt handler that just ran, and the next iteration may
            // be clean: count a failure and retake the state rather than give
            // up. A loop that stores on every iteration reaches `ECHECS_MAX` in
            // three and costs nothing thereafter.
            self.boucle_echecs += 1;
            let etat = self.regs.clone();
            self.boucle_regs = etat;
            return;
        }
        if self.regs == self.boucle_regs {
            let saute = self.sauter_le_temps(periph);
            // The skip just made the clock jump: the loop is still alive and
            // must be marked as such, or the protection would lapse right
            // after every skip.
            self.boucle_vu_a = self.cycles;
            if saute {
                self.boucle_echecs = 0;
                self.boucle_recul = 0;
            } else {
                // Nothing to skip because an interrupt is already pending
                // without being takeable: no point insisting every iteration.
            }
        } else {
            self.boucle_echecs += 1;
            self.boucle_sp = sp;
            bus.plancher_pile = sp;
            let etat = self.regs.clone();
            self.boucle_regs = etat;
        }
    }

    /// Advances the clock without interpreting, while nothing can change.
    ///
    /// Peripherals are serviced at the usual granularity: what they count, they
    /// count exactly as if the core had run. The loop stops as soon as an
    /// interrupt becomes possible, or after `SAUT_MAXIMUM` cycles for waits
    /// that break on a mere change in a polled register, with no interrupt.
    fn sauter_le_temps(&mut self, periph: &mut Peripherals) -> bool {
        if self.nvic.en_attente {
            return false;
        }
        let grain = Self::GRAIN_PERIPHERIQUES;
        let mut saute = 0u32;
        while saute < Self::SAUT_MAXIMUM && !self.nvic.en_attente {
            self.entretenir_peripheriques(grain, periph);
            saute += grain;
            if periph.dma.irq_a_lever {
                periph.dma.irq_a_lever = false;
                self.nvic
                    .request_irq(crate::emulator::peripherals::dma::IRQ);
            }
        }
        self.cycles += saute as u64;
        self.cycles_sautes += saute as u64;
        self.sauts += 1;
        saute > 0
    }

    /// Fait avancer tout ce qui vit au rythme des cycles.
    fn entretenir_peripheriques(&mut self, ecoules: u32, periph: &mut Peripherals) {
        // Le SysTick pose lui-meme son drapeau d'attente : c'est une exception
        // systeme, pas une IRQ externe a inscrire dans ISPR.
        self.nvic.tick_systick(ecoules);
        // Le TE de l'ecran est entretenu ici : c'est un signal exterieur, sans
        // quoi le firmware l'attend sans fin.
        if periph.port1.tick(ecoules) {
            self.nvic
                .request_irq(crate::emulator::peripherals::gpio_port::PORT1_IRQ);
        }
        if let Some(irq) = periph.tic.tick(ecoules) {
            self.nvic.request_irq(irq);
        }
        if periph.adc_pile.irq_a_lever | periph.adc_pile.tick(ecoules) {
            periph.adc_pile.irq_a_lever = false;
            self.nvic
                .request_irq(crate::emulator::peripherals::adc_pile::IRQ);
        }
        if periph.timers.tick(ecoules) {
            self.nvic.request_irq(16);
        }
        // Les lignes serie avancent a leur debit programme. Le tampon d'entree
        // peut ainsi contenir un gros bloc venu de l'hote sans saturer la FIFO
        // materielle de seize octets.
        periph.uart.tick(
            ecoules,
            crate::emulator::peripherals::snsys::CYCLES_PAR_SECONDE as u32,
        );
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

    fn enter_exception(
        &mut self,
        exception_num: u32,
        bus: &mut MemoryBus,
        periph: &mut Peripherals,
    ) {
        let mut sp = self.regs.get_sp();

        // L'etat du bloc IT voyage dans le xPSR empile, aux places que lui donne
        // l'architecture : bits 26:25 pour ses deux bits bas, 15:10 pour les six
        // hauts. Sans cela une exception prise entre un IT et son instruction
        // conditionnelle laisse le gestionnaire heriter de la condition, et sa
        // premiere instruction est sautee. C'est ainsi que le gestionnaire du TE
        // perdait le PUSH de son adresse de retour et ne revenait jamais.
        let it = self.regs.itstate as u32;
        let xpsr_empile =
            (self.regs.xpsr & !0x0600_FC00) | ((it & 0x3) << 25) | (((it >> 2) & 0x3F) << 10);

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
        self.regs.itstate = ((((xpsr >> 25) & 0x3) | (((xpsr >> 10) & 0x3F) << 2)) & 0xFF) as u8;
        self.regs.mode = Mode::Thread;
        self.regs.set_sp(sp);
        self.regs.pc = ret & !1;
    }
}
