use super::registers::Registers;
use super::thumb16::{StepResult, Thumb16};
use crate::emulator::cpu::nvic::Nvic;
use crate::emulator::mmu::MemoryBus;
use crate::emulator::peripherals::Peripherals;

pub struct Thumb32;

impl Thumb32 {
    pub fn execute(
        w1: u16,
        w2: u16,
        regs: &mut Registers,
        bus: &mut MemoryBus,
        periph: &mut Peripherals,
        nvic: &mut Nvic,
    ) -> StepResult {
        // 1. Branch with Link (BL / BLX): 1111 0 s imm10  11 j1 1 j2 imm11
        if (w1 & 0xF800) == 0xF000 && (w2 & 0xD000) == 0xD000 {
            let s = ((w1 >> 10) & 1) as u32;
            let imm10 = (w1 & 0x03FF) as u32;
            let j1 = ((w2 >> 13) & 1) as u32;
            let j2 = ((w2 >> 11) & 1) as u32;
            let imm11 = (w2 & 0x07FF) as u32;

            let i1 = !(j1 ^ s) & 1;
            let i2 = !(j2 ^ s) & 1;

            let mut imm25 = (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);
            if (imm25 & 0x0100_0000) != 0 {
                imm25 |= 0xFE00_0000;
            }
            // Pour une instruction 32 bits, step() a deja avance de 4 : regs.pc
            // est donc l'adresse de retour, il ne reste qu'a marquer le bit Thumb.
            regs.lr = regs.pc | 1;
            regs.pc = (regs.pc as i32 + (imm25 as i32)) as u32;
            return StepResult::Ok(3);
        }

        // 1b. B.W inconditionnel, forme T4 : w2[15:14] = 10 et w2[12] = 1.
        //     Meme champ d'immediat que BL, sans ecriture de LR.
        if (w1 & 0xF800) == 0xF000 && (w2 & 0xD000) == 0x9000 {
            let s = ((w1 >> 10) & 1) as u32;
            let imm10 = (w1 & 0x03FF) as u32;
            let j1 = ((w2 >> 13) & 1) as u32;
            let j2 = ((w2 >> 11) & 1) as u32;
            let imm11 = (w2 & 0x07FF) as u32;
            let i1 = !(j1 ^ s) & 1;
            let i2 = !(j2 ^ s) & 1;
            let mut imm25 = (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);
            if (imm25 & 0x0100_0000) != 0 {
                imm25 |= 0xFE00_0000;
            }
            regs.pc = (regs.pc as i32 + (imm25 as i32)) as u32;
            return StepResult::Ok(3);
        }

        // 1c. B<cond>.W, forme T3 : w2[15:14] = 10 et w2[12] = 0.
        //     Sans ce cas, les branchements conditionnels longs tombaient dans le
        //     decodeur ALU immediat et s'executaient comme un ORR.
        if (w1 & 0xF800) == 0xF000 && (w2 & 0xD000) == 0x8000 {
            let cond = (w1 >> 6) & 0xF;
            if cond < 0xE {
                if !Thumb16::eval_condition(cond, regs) {
                    return StepResult::Ok(1);
                }
                let s = ((w1 >> 10) & 1) as u32;
                let imm6 = (w1 & 0x3F) as u32;
                let j1 = ((w2 >> 13) & 1) as u32;
                let j2 = ((w2 >> 11) & 1) as u32;
                let imm11 = (w2 & 0x07FF) as u32;
                let mut imm21 =
                    (s << 20) | (j2 << 19) | (j1 << 18) | (imm6 << 12) | (imm11 << 1);
                if (imm21 & 0x0010_0000) != 0 {
                    imm21 |= 0xFFE0_0000;
                }
                regs.pc = (regs.pc as i32 + (imm21 as i32)) as u32;
                return StepResult::Ok(3);
            }
        }

        // 2. MOVW / MOVT (Move 16-bit immediate): 1111 0 i 10 x 100 imm4 0 imm3 rd imm8
        if (w1 & 0xFBF0) == 0xF240 || (w1 & 0xFBF0) == 0xF2C0 {
            let is_movt = (w1 & 0x0080) != 0;
            let rd = ((w2 >> 8) & 0xF) as u8;
            let imm4 = (w1 & 0xF) as u32;
            let i = ((w1 >> 10) & 1) as u32;
            let imm3 = ((w2 >> 12) & 7) as u32;
            let imm8 = (w2 & 0xFF) as u32;
            let imm16 = (imm4 << 12) | (i << 11) | (imm3 << 8) | imm8;

            if is_movt {
                let current = regs.get_reg(rd);
                regs.set_reg(rd, (current & 0x0000_FFFF) | (imm16 << 16));
            } else {
                regs.set_reg(rd, imm16);
            }
            return StepResult::Ok(1);
        }

        // 3. 32-bit Multiply & Divide: 1111 1011 xxxx
        if (w1 & 0xFF80) == 0xFB80 || (w1 & 0xFF80) == 0xFB00 {
            let rn = (w1 & 0xF) as u8;
            let rd = ((w2 >> 8) & 0xF) as u8;
            let rm = (w2 & 0xF) as u8;
            let ra = ((w2 >> 12) & 0xF) as u8;
            let op1 = (w1 >> 4) & 0x7;
            let op2 = (w2 >> 4) & 0x7;

            let val_n = regs.get_reg(rn);
            let val_m = regs.get_reg(rm);

            // SDIV / UDIV
            if (w1 & 0xFFF0) == 0xFB90 && op2 == 0xF {
                // SDIV
                let res = if val_m == 0 { 0 } else { ((val_n as i32) / (val_m as i32)) as u32 };
                regs.set_reg(rd, res);
                return StepResult::Ok(2);
            }
            if (w1 & 0xFFF0) == 0xFBB0 && op2 == 0xF {
                // UDIV
                let res = if val_m == 0 { 0 } else { val_n / val_m };
                regs.set_reg(rd, res);
                return StepResult::Ok(2);
            }

            // MUL / MLA / MLS
            if op1 == 0 {
                if ra == 0xF {
                    // MUL
                    regs.set_reg(rd, val_n.wrapping_mul(val_m));
                } else if op2 == 0 {
                    // MLA
                    let val_a = regs.get_reg(ra);
                    regs.set_reg(rd, val_a.wrapping_add(val_n.wrapping_mul(val_m)));
                } else if op2 == 1 {
                    // MLS
                    let val_a = regs.get_reg(ra);
                    regs.set_reg(rd, val_a.wrapping_sub(val_n.wrapping_mul(val_m)));
                }
                return StepResult::Ok(1);
            }
        }

        // 4. Bitfield Extract / Insert: UBFX / SBFX / BFC / BFI: 1111 0011 01xx
        if (w1 & 0xFBF0) == 0xF3C0 || (w1 & 0xFBF0) == 0xF340 || (w1 & 0xFBF0) == 0xF360 {
            let is_ubfx = (w1 & 0x0070) == 0x0040;
            let is_sbfx = (w1 & 0x0070) == 0x0060;
            let is_bfi = (w1 & 0x0070) == 0x0020;
            let rn = (w1 & 0xF) as u8;
            let rd = ((w2 >> 8) & 0xF) as u8;
            let lsb = (((w1 >> 6) & 3) << 3) | ((w2 >> 12) & 7);
            let width_minus1 = (w2 & 0x1F) as u32;
            let width = width_minus1 + 1;

            if is_ubfx {
                let val = regs.get_reg(rn);
                let mask = (1u64 << width) - 1;
                let res = ((val as u64 >> lsb) & mask) as u32;
                regs.set_reg(rd, res);
                return StepResult::Ok(1);
            } else if is_sbfx {
                let val = regs.get_reg(rn);
                let shift = lsb;
                let mut res = (val >> shift) & ((1 << width) - 1);
                if (res & (1 << (width - 1))) != 0 {
                    res |= !((1 << width) - 1);
                }
                regs.set_reg(rd, res);
                return StepResult::Ok(1);
            } else if is_bfi {
                if rn == 0xF {
                    // BFC (Bit Field Clear)
                    let current = regs.get_reg(rd);
                    let mask = ((1 << width) - 1) << lsb;
                    regs.set_reg(rd, current & !mask);
                } else {
                    // BFI (Bit Field Insert)
                    let current = regs.get_reg(rd);
                    let val_n = regs.get_reg(rn);
                    let mask = ((1 << width) - 1) << lsb;
                    let inserted = (val_n & ((1 << width) - 1)) << lsb;
                    regs.set_reg(rd, (current & !mask) | inserted);
                }
                return StepResult::Ok(1);
            }
        }

        // 5. CLZ (Count Leading Zeros): 1111 1010 1011 rm 1111 rd 1000 rm
        if (w1 & 0xFFF0) == 0xFAB0 && (w2 & 0xF0F0) == 0xF080 {
            let rm = (w1 & 0xF) as u8;
            let rd = ((w2 >> 8) & 0xF) as u8;
            let val = regs.get_reg(rm);
            regs.set_reg(rd, val.leading_zeros());
            return StepResult::Ok(1);
        }

        // 6. Traitement de donnees 32 bits, deux formes qui partagent le meme
        //    champ d'operation sur 4 bits :
        //      immediat modifie : 1111 0 i 0 oooo S nnnn, avec w2[15] = 0
        //      registre decale  : 1110 101 oooo S nnnn
        //    Le garde-fou w2[15] = 0 est indispensable : sans lui, les
        //    branchements conditionnels longs (w2[15] = 1) etaient executes ici.
        let is_alu_imm = (w1 & 0xFA00) == 0xF000 && (w2 & 0x8000) == 0;
        let is_alu_reg = (w1 & 0xFE00) == 0xEA00;
        if is_alu_imm || is_alu_reg {
            let s_flag = (w1 & 0x0010) != 0;
            let rn = (w1 & 0xF) as u8;
            let rd = ((w2 >> 8) & 0xF) as u8;
            let op = (w1 >> 5) & 0xF;
            // Rn = 0xF est le marqueur MOV / MVN : la source vaut 0, pas PC.
            let val_n = if rn == 0xF { 0 } else { regs.get_reg(rn) };
            let carry_in = regs.flag_c();

            // Les operations logiques prennent leur retenue de l'etage de decalage,
            // les operations arithmetiques la produisent elles-memes.
            let (val_op2, shifter_c) = if is_alu_imm {
                let i = ((w1 >> 10) & 1) as u32;
                let imm3 = ((w2 >> 12) & 7) as u32;
                let imm8 = (w2 & 0xFF) as u32;
                thumb_expand_imm_c((i << 11) | (imm3 << 8) | imm8, carry_in)
            } else {
                let rm = (w2 & 0xF) as u8;
                let imm3 = ((w2 >> 12) & 0x7) as u32;
                let imm2 = ((w2 >> 6) & 0x3) as u32;
                let type_ = ((w2 >> 4) & 0x3) as u32;
                shift_c(regs.get_reg(rm), (imm3 << 2) | imm2, type_, carry_in)
            };

            let mut c_out = shifter_c;
            let mut v_out = regs.flag_v();
            let res = match op {
                0 => val_n & val_op2,  // AND / TST
                1 => val_n & !val_op2, // BIC
                2 => val_n | val_op2,  // ORR / MOV
                3 => val_n | !val_op2, // ORN / MVN
                4 => val_n ^ val_op2,  // EOR / TEQ
                8 => {
                    let (r, c, v) = add_with_carry(val_n, val_op2, false);
                    c_out = c;
                    v_out = v;
                    r
                } // ADD / CMN
                10 => {
                    let (r, c, v) = add_with_carry(val_n, val_op2, carry_in);
                    c_out = c;
                    v_out = v;
                    r
                } // ADC
                11 => {
                    let (r, c, v) = add_with_carry(val_n, !val_op2, carry_in);
                    c_out = c;
                    v_out = v;
                    r
                } // SBC
                13 => {
                    let (r, c, v) = add_with_carry(val_n, !val_op2, true);
                    c_out = c;
                    v_out = v;
                    r
                } // SUB / CMP
                14 => {
                    let (r, c, v) = add_with_carry(!val_n, val_op2, true);
                    c_out = c;
                    v_out = v;
                    r
                } // RSB
                _ => return StepResult::Undefined(w1),
            };

            // TST, TEQ, CMN et CMP s'ecrivent avec Rd = PC et ne rangent rien.
            if !(rd == 0xF && matches!(op, 0 | 4 | 8 | 13)) {
                regs.set_reg(rd, res);
            }
            if s_flag {
                regs.set_nz(res);
                regs.set_flag_c(c_out);
                regs.set_flag_v(v_out);
            }
            return StepResult::Ok(1);
        }

        // 7. 32-bit Single Data Transfer: LDR / STR (Byte, Halfword, Word)
        if (w1 & 0xFE00) == 0xF800 || (w1 & 0xFE00) == 0xF900 {
            let is_ldr = (w1 & 0x0010) != 0;
            // La largeur est codee en w1[6:5], pas en w1[8:7] : LDR.W etait lu
            // comme un LDRH et ne ramenait que les 16 bits de poids faible.
            let size = (w1 >> 5) & 3; // 0 = octet, 1 = demi-mot, 2 = mot
            // Le groupe 0xF9xx est la variante a extension de signe.
            let is_signed = (w1 & 0x0100) != 0;
            let rn = (w1 & 0xF) as u8;
            let rd = ((w2 >> 12) & 0xF) as u8;

            // Mise a jour differee de la base, pour les formes indexees.
            let mut writeback: Option<(u8, u32)> = None;

            let addr = if (w1 & 0x0080) != 0 {
                // Forme T3 : offset immediat 12 bits, toujours positif, sans
                // mise a jour de la base.
                let imm12 = (w2 & 0x0FFF) as u32;
                let base = if rn == 0xF { regs.pc & !3 } else { regs.get_reg(rn) };
                base.wrapping_add(imm12)
            } else if (w2 & 0x0800) != 0 {
                // Forme T4 : immediat 8 bits avec indexation P / U / W.
                // P = 0 designe le post-indexe : l'acces se fait a la base, et la
                // base est mise a jour ensuite. Sans cela, une boucle de recopie
                // relisait indefiniment le meme octet sans jamais avancer.
                let pre_index = (w2 & 0x0400) != 0;
                let add = (w2 & 0x0200) != 0;
                let write_base = (w2 & 0x0100) != 0;
                let imm8 = (w2 & 0xFF) as u32;
                let base = if rn == 0xF { regs.pc & !3 } else { regs.get_reg(rn) };
                let offset_addr = if add {
                    base.wrapping_add(imm8)
                } else {
                    base.wrapping_sub(imm8)
                };
                if write_base && rn != 0xF {
                    writeback = Some((rn, offset_addr));
                }
                if pre_index {
                    offset_addr
                } else {
                    base
                }
            } else {
                // Offset registre, decale de 0 a 3 bits.
                let rm = (w2 & 0xF) as u8;
                let shift = ((w2 >> 4) & 3) as u32;
                let base = if rn == 0xF { regs.pc & !3 } else { regs.get_reg(rn) };
                base.wrapping_add(regs.get_reg(rm) << shift)
            };

            if is_ldr {
                let val = match (size, is_signed) {
                    (0, false) => bus.read_u8(addr, periph, nvic) as u32,
                    (0, true) => bus.read_u8(addr, periph, nvic) as i8 as i32 as u32,
                    (1, false) => bus.read_u16(addr, periph, nvic) as u32,
                    (1, true) => bus.read_u16(addr, periph, nvic) as i16 as i32 as u32,
                    _ => bus.read_u32(addr, periph, nvic),
                };
                regs.set_reg(rd, val);
            } else {
                let val = regs.get_reg(rd);
                match size {
                    0 => bus.write_u8(addr, val as u8, periph, nvic),
                    1 => bus.write_u16(addr, val as u16, periph, nvic),
                    _ => bus.write_u32(addr, val, periph, nvic),
                }
            }
            // La base est mise a jour apres l'acces, jamais avant.
            if let Some((r, v)) = writeback {
                regs.set_reg(r, v);
            }
            return StepResult::Ok(2);
        }

        // 7b. Table branch TBB / TBH (w1 = 0xE8DF).
        //     TBB [Rn, Rm]      : cible = PC + 2 * octet[Rn + Rm].
        //     TBH [Rn, Rm, LSL#1]: cible = PC + 2 * demi-mot[Rn + 2*Rm].
        //     PC vaut ici deja l'adresse de l'instruction + 4 (avance par step()).
        if (w1 & 0xFFF0) == 0xE8D0 && (w2 & 0xFFE0) == 0xF000 {
            let is_tbh = (w2 & 0x0010) != 0;
            // Rn est dans le premier demi-mot, pas dans le second.
            let rn = (w1 & 0xF) as u8;
            let rm = (w2 & 0xF) as u8;
            let base = if rn == 0xF { regs.pc } else { regs.get_reg(rn) };
            let rm_val = regs.get_reg(rm);
            let table = if is_tbh {
                base.wrapping_add(rm_val << 1)
            } else {
                base.wrapping_add(rm_val)
            };
            let offset = if is_tbh {
                bus.read_u16(table, periph, nvic) as u32
            } else {
                bus.read_u8(table, periph, nvic) as u32
            };
            regs.pc = regs.pc.wrapping_add(offset << 1);
            return StepResult::Ok(2);
        }

        // 8. Acces multiples 32 bits : 1110 100 mm W L nnnn.
        //    mm = 01 -> increment apres (IA), mm = 10 -> decrement avant (DB).
        //    Le bit 6 vaut toujours 0 ici ; 0xE8Dx (TBB/TBH) l'a a 1 et est traite
        //    plus haut.
        if (w1 & 0xFE40) == 0xE800 && matches!((w1 >> 7) & 3, 1 | 2) {
            let decrement_before = ((w1 >> 7) & 3) == 2;
            let is_ldm = (w1 & 0x0010) != 0;
            let writeback = (w1 & 0x0020) != 0;
            let rn = (w1 & 0xF) as u8;
            let reg_list = w2 & 0xFFFF;
            let count = reg_list.count_ones();
            let base = regs.get_reg(rn);
            // En mode DB, la zone ecrite commence sous la base : c'est ce que fait
            // PUSH.W, et l'assimiler a un IA corrompait la pile.
            let start = if decrement_before {
                base.wrapping_sub(4 * count)
            } else {
                base
            };

            let mut addr = start;
            for i in 0..16 {
                if (reg_list & (1 << i)) == 0 {
                    continue;
                }
                if is_ldm {
                    let v = bus.read_u32(addr, periph, nvic);
                    if i == 15 {
                        // POP {..., pc} : le bit Thumb ne fait pas partie de l'adresse.
                        regs.pc = v & !1;
                    } else {
                        regs.set_reg(i as u8, v);
                    }
                } else {
                    let v = regs.get_reg(i as u8);
                    bus.write_u32(addr, v, periph, nvic);
                }
                addr = addr.wrapping_add(4);
            }

            if writeback {
                let new_base = if decrement_before {
                    start
                } else {
                    base.wrapping_add(4 * count)
                };
                regs.set_reg(rn, new_base);
            }
            return StepResult::Ok(3);
        }

        // 9. Data Barriers: DMB / DSB / ISB
        if (w1 & 0xFFF0) == 0xF3B0 && (w2 & 0xFFF0) == 0x8F40 {
            return StepResult::Ok(1);
        }

        // 10. MSR : 1111 0011 100 0 nnnn | 1000 10mm 0000 SYSm.
        //     L'ancien test attrapait la plage MRS et inversait les deux sens,
        //     si bien qu'un MSR PRIMASK n'etait pas decode du tout.
        if (w1 & 0xFFF0) == 0xF380 && (w2 & 0xF000) == 0x8000 {
            let rn = (w1 & 0xF) as u8;
            let sysm = (w2 & 0xFF) as u8;
            let val = regs.get_reg(rn);
            match sysm {
                // APSR et alias : seuls les drapeaux de condition sont ecrits.
                0..=3 => regs.xpsr = (regs.xpsr & 0x07FF_FFFF) | (val & 0xF800_0000),
                8 => regs.msp = val & !3,
                9 => regs.psp = val & !3,
                16 => regs.primask = val & 1,
                17 | 18 => regs.basepri = val & 0xFF,
                19 => regs.faultmask = val & 1,
                20 => regs.control = val & 3,
                _ => {}
            }
            return StepResult::Ok(2);
        }

        // 11. MRS : 1111 0011 1110 1111 | 1000 dddd 0000 SYSm.
        if w1 == 0xF3EF && (w2 & 0xF000) == 0x8000 {
            let rd = ((w2 >> 8) & 0xF) as u8;
            let sysm = (w2 & 0xFF) as u8;
            let val = match sysm {
                0 => regs.xpsr & 0xF800_0000,                    // APSR
                1 => regs.xpsr & (0xF800_0000 | 0x1FF),          // IAPSR
                2 => regs.xpsr & (0xF800_0000 | 0x0100_0000),    // EAPSR
                3 | 7 => regs.xpsr,                              // xPSR / IEPSR
                5 => regs.xpsr & 0x1FF,                          // IPSR
                6 => regs.xpsr & 0x0100_0000,                    // EPSR
                8 => regs.msp,
                9 => regs.psp,
                16 => regs.primask,
                17 | 18 => regs.basepri,
                19 => regs.faultmask,
                20 => regs.control,
                _ => 0,
            };
            regs.set_reg(rd, val);
            return StepResult::Ok(2);
        }

        // Rien n'a reconnu cet encodage. On le signale au lieu de l'executer comme
        // un NOP : une instruction avalee en silence fausse tout ce qui suit.
        StepResult::Undefined(w1)
    }
}

/// Addition avec retenue, telle que definie par AddWithCarry de l'architecture.
/// Rend le resultat, la retenue sortante et le debordement signe.
pub(crate) fn add_with_carry(a: u32, b: u32, carry_in: bool) -> (u32, bool, bool) {
    let (s1, c1) = a.overflowing_add(b);
    let (res, c2) = s1.overflowing_add(carry_in as u32);
    let carry = c1 || c2;
    let overflow = ((a ^ res) & (b ^ res) & 0x8000_0000) != 0;
    (res, carry, overflow)
}

/// Deplie un immediat modifie 12 bits (i:imm3:imm8) selon ThumbExpandImm_C,
/// et rend la retenue associee.
fn thumb_expand_imm_c(imm12: u32, carry_in: bool) -> (u32, bool) {
    if imm12 & 0xC00 == 0 {
        // imm12[11:10] == 00 : imm8 decale de (imm3 * 8), retenue inchangee.
        let imm8 = imm12 & 0xFF;
        let shift = ((imm12 >> 8) & 0x3) * 8;
        (imm8 << shift, carry_in)
    } else {
        // Rotation d'un octet 0b1:imm12[6:0] de imm12[11:7] bits.
        let unrotated = 0x80 | (imm12 & 0x7F);
        let v = unrotated.rotate_right((imm12 >> 7) & 0x1F);
        (v, (v & 0x8000_0000) != 0)
    }
}

/// Operande registre decale d'une instruction ALU 32 bits, avec sa retenue.
fn shift_c(rm: u32, shift: u32, type_: u32, carry_in: bool) -> (u32, bool) {
    match type_ {
        // LSL, decalage nul laisse la retenue intacte.
        0 => {
            if shift == 0 {
                (rm, carry_in)
            } else {
                (rm << shift, (rm >> (32 - shift)) & 1 != 0)
            }
        }
        // LSR, #0 vaut #32.
        1 => {
            let s = if shift == 0 { 32 } else { shift };
            if s == 32 {
                (0, (rm >> 31) & 1 != 0)
            } else {
                (rm >> s, (rm >> (s - 1)) & 1 != 0)
            }
        }
        // ASR, #0 vaut #32, extension de signe.
        2 => {
            let s = if shift == 0 { 32 } else { shift };
            if s >= 32 {
                (((rm as i32) >> 31) as u32, (rm >> 31) & 1 != 0)
            } else {
                (((rm as i32) >> s) as u32, (rm >> (s - 1)) & 1 != 0)
            }
        }
        // ROR, #0 est RRX : la retenue entrante devient le bit 31.
        _ => {
            if shift == 0 {
                (((carry_in as u32) << 31) | (rm >> 1), rm & 1 != 0)
            } else {
                let v = rm.rotate_right(shift);
                (v, (v >> 31) & 1 != 0)
            }
        }
    }
}
