#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    Thread,
    Handler,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StackPointerType {
    Main,    // MSP
    Process, // PSP
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Registers {
    pub r: [u32; 13], // R0 - R12
    pub msp: u32,     // Main Stack Pointer
    pub psp: u32,     // Process Stack Pointer
    pub lr: u32,      // R14 (Link Register)
    pub pc: u32,      // R15 (Program Counter)
    pub xpsr: u32,    // Program Status Register
    pub primask: u32, // Interrupt mask
    pub basepri: u32, // Seuil de priorite d'interruption
    pub faultmask: u32, // Masque de faute
    pub control: u32, // Control register
    /// ITSTATE : [7:4] condition courante, [3:0] masque du bloc IT.
    pub itstate: u8,
    pub mode: Mode,
}

impl Default for Registers {
    fn default() -> Self {
        Self {
            r: [0; 13],
            msp: 0x2001_0000, // Top of SRAM default
            psp: 0x2001_0000,
            lr: 0xFFFF_FFFF,
            pc: 0x0000_0000,
            xpsr: 0x0100_0000, // Thumb bit set by default
            primask: 0,
            basepri: 0,
            faultmask: 0,
            control: 0,
            itstate: 0,
            mode: Mode::Thread,
        }
    }
}

impl Registers {
    pub fn get_sp(&self) -> u32 {
        if self.use_psp() {
            self.psp
        } else {
            self.msp
        }
    }

    pub fn set_sp(&mut self, val: u32) {
        if self.use_psp() {
            self.psp = val;
        } else {
            self.msp = val;
        }
    }

    pub fn use_psp(&self) -> bool {
        self.mode == Mode::Thread && (self.control & 0x02) != 0
    }

    pub fn get_reg(&self, reg: u8) -> u32 {
        match reg {
            0..=12 => self.r[reg as usize],
            13 => self.get_sp(),
            14 => self.lr,
            15 => self.pc,
            _ => 0,
        }
    }

    pub fn set_reg(&mut self, reg: u8, val: u32) {
        match reg {
            0..=12 => self.r[reg as usize] = val,
            13 => self.set_sp(val),
            14 => self.lr = val,
            // Ecrire dans R15 est un branchement avec echange de jeu
            // d'instructions. Le bit 0 y designe le jeu, il ne fait pas partie
            // de l'adresse, et le coeur est Thumb seul. Sans l'effacer, un
            // LDR, un MOV ou un POP vers le PC laisse une adresse impaire : la
            // lecture d'instruction se decale alors d'un octet, et l'execution
            // part dans du code qui se lit encore mais ne veut plus rien dire.
            15 => self.pc = val & !1,
            _ => {}
        }
    }

    // APSR condition flags
    pub fn flag_n(&self) -> bool {
        (self.xpsr & 0x8000_0000) != 0
    }

    pub fn set_flag_n(&mut self, val: bool) {
        if val {
            self.xpsr |= 0x8000_0000;
        } else {
            self.xpsr &= !0x8000_0000;
        }
    }

    pub fn flag_z(&self) -> bool {
        (self.xpsr & 0x4000_0000) != 0
    }

    pub fn set_flag_z(&mut self, val: bool) {
        if val {
            self.xpsr |= 0x4000_0000;
        } else {
            self.xpsr &= !0x4000_0000;
        }
    }

    pub fn flag_c(&self) -> bool {
        (self.xpsr & 0x2000_0000) != 0
    }

    pub fn set_flag_c(&mut self, val: bool) {
        if val {
            self.xpsr |= 0x2000_0000;
        } else {
            self.xpsr &= !0x2000_0000;
        }
    }

    pub fn flag_v(&self) -> bool {
        (self.xpsr & 0x1000_0000) != 0
    }

    pub fn set_flag_v(&mut self, val: bool) {
        if val {
            self.xpsr |= 0x1000_0000;
        } else {
            self.xpsr &= !0x1000_0000;
        }
    }

    pub fn set_nz(&mut self, val: u32) {
        self.set_flag_n((val as i32) < 0);
        self.set_flag_z(val == 0);
    }
}
