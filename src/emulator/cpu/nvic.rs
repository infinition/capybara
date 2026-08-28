#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Nvic {
    pub iser: [u32; 8], // Interrupt Set-Enable
    pub icer: [u32; 8], // Interrupt Clear-Enable
    pub ispr: [u32; 8], // Interrupt Set-Pending
    pub icpr: [u32; 8], // Interrupt Clear-Pending
    pub syst_csr: u32,  // SysTick Control and Status
    pub syst_rvr: u32,  // SysTick Reload Value
    pub syst_cvr: u32,  // SysTick Current Value
    pub vtor: u32,      // Vector Table Offset Register
    /// SysTick est l'exception 15, une exception systeme. Elle ne passe pas par
    /// les registres ISER du NVIC, qui ne gouvernent que les IRQ externes : son
    /// seul interrupteur est le bit TICKINT de SYST_CSR.
    pub systick_pending: bool,
    /// Vrai quand quelque chose a pu devenir en attente.
    ///
    /// Sans lui, le coeur parcourait les huit mots de drapeaux avant chaque
    /// instruction, alors qu'il n'y a presque jamais rien a prendre. Le drapeau
    /// se corrige tout seul : on le pose des qu'une interruption est demandee,
    /// et le coeur l'efface quand il regarde et ne trouve rien.
    #[serde(default = "vrai")]
    pub en_attente: bool,
}

impl Default for Nvic {
    fn default() -> Self {
        Self {
            iser: [0; 8],
            icer: [0; 8],
            ispr: [0; 8],
            icpr: [0; 8],
            syst_csr: 0,
            syst_rvr: 0,
            syst_cvr: 0,
            vtor: 0x0000_0000,
            systick_pending: false,
            en_attente: true,
        }
    }
}

impl Nvic {
    pub fn read_reg(&self, addr: u32) -> u32 {
        match addr {
            0xE000_E010 => self.syst_csr,
            0xE000_E014 => self.syst_rvr,
            0xE000_E018 => self.syst_cvr,
            0xE000_ED08 => self.vtor,
            0xE000_E100..=0xE000_E11C => {
                let idx = ((addr - 0xE000_E100) / 4) as usize;
                self.iser[idx]
            }
            0xE000_E200..=0xE000_E21C => {
                let idx = ((addr - 0xE000_E200) / 4) as usize;
                self.ispr[idx]
            }
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, addr: u32, val: u32) {
        self.en_attente = true;
        match addr {
            0xE000_E010 => self.syst_csr = val & 0x7,
            0xE000_E014 => self.syst_rvr = val & 0x00FF_FFFF,
            0xE000_E018 => self.syst_cvr = 0, // Write clears CVR
            0xE000_ED08 => self.vtor = val & 0xFFFF_FF80,
            0xE000_E100..=0xE000_E11C => {
                let idx = ((addr - 0xE000_E100) / 4) as usize;
                self.iser[idx] |= val;
            }
            0xE000_E180..=0xE000_E19C => {
                let idx = ((addr - 0xE000_E180) / 4) as usize;
                self.iser[idx] &= !val;
            }
            0xE000_E200..=0xE000_E21C => {
                let idx = ((addr - 0xE000_E200) / 4) as usize;
                self.ispr[idx] |= val;
            }
            0xE000_E280..=0xE000_E29C => {
                let idx = ((addr - 0xE000_E280) / 4) as usize;
                self.ispr[idx] &= !val;
            }
            _ => {}
        }
    }

    /// Numero d'exception du SysTick dans la table de vecteurs.
    pub const SYSTICK_EXCEPTION: u32 = 15;

    pub fn tick_systick(&mut self, cycles: u32) -> bool {
        if (self.syst_csr & 1) == 0 {
            return false;
        }

        let mut trigger_irq = false;
        if self.syst_cvr <= cycles {
            self.syst_cvr = self.syst_rvr;
            self.syst_csr |= 1 << 16; // COUNTFLAG
            if (self.syst_csr & 2) != 0 {
                // TICKINT
                trigger_irq = true;
                self.systick_pending = true;
                self.en_attente = true;
            }
        } else {
            self.syst_cvr -= cycles;
        }

        trigger_irq
    }

    pub fn request_irq(&mut self, irq: u32) {
        self.en_attente = true;
        if irq < 240 {
            let idx = (irq / 32) as usize;
            let bit = irq % 32;
            self.ispr[idx] |= 1 << bit;
        }
    }

    pub fn get_highest_pending_irq(&self) -> Option<u32> {
        for idx in 0..8 {
            let active = self.iser[idx] & self.ispr[idx];
            if active != 0 {
                let bit = active.trailing_zeros();
                return Some((idx as u32) * 32 + bit);
            }
        }
        None
    }

    pub fn acknowledge_irq(&mut self, irq: u32) {
        if irq < 240 {
            let idx = (irq / 32) as usize;
            let bit = irq % 32;
            self.ispr[idx] &= !(1 << bit);
        }
    }
}

/// Valeur par defaut du drapeau d'attente pour les anciens instantanes : il
/// vaut mieux regarder une fois pour rien que manquer une interruption.
fn vrai() -> bool {
    true
}
