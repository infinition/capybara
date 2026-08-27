use super::registers::Registers;
use super::thumb16::StepResult;
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

        // 6. 32-bit Data Processing / ALU.
        //    Immediat modifie : 1111 0 i 0 op S Rn  (bit 9 = 0)
        //    Registre decale  : 1110 1010 0 op S Rn  (bits 15:8 = 1110 1010)
        if (w1 & 0xFA00) == 0xF000 || (w1 & 0xFA00) == 0xEA00 {
            let s_flag = (w1 & 0x0010) != 0;
            let rn = (w1 & 0xF) as u8;
            let rd = ((w2 >> 8) & 0xF) as u8;
            // Rn = 0xF est le marqueur MOV / MVN : la source vaut 0, pas PC.
            let val_n = if rn == 0xF { 0 } else { regs.get_reg(rn) };

            if (w1 & 0x0200) == 0 {
                // Immediat modifie, code sur 12 bits i:imm3:imm8.
                let op = (w1 >> 5) & 0xF;
                let i = ((w1 >> 10) & 1) as u32;
                let imm3 = ((w2 >> 12) & 7) as u32;
                let imm8 = (w2 & 0xFF) as u32;
                let val_op2 = thumb_expand_imm((i << 11) | (imm3 << 8) | imm8);

                let res = match op {
                    0 => val_n & val_op2, // AND / TST
                    1 => val_n & !val_op2, // BIC
                    2 => val_n | val_op2, // ORR / MOV
                    3 => val_n | !val_op2, // ORN / MVN
                    4 => val_n ^ val_op2, // EOR / TEQ
                    8 => val_n.wrapping_add(val_op2), // ADD / CMN
                    10 => val_n.wrapping_add(val_op2).wrapping_add(if regs.flag_c() { 1 } else { 0 }), // ADC
                    11 => val_n.wrapping_sub(val_op2).wrapping_sub(if !regs.flag_c() { 1 } else { 0 }), // SBC
                    13 => val_n.wrapping_sub(val_op2), // SUB / CMP
                    14 => val_op2.wrapping_sub(val_n), // RSB
                    _ => val_n,
                };
                // TST/TEQ/CMN/CMP n'ecrivent pas de destination (rd == 0xF).
                if !(rd == 0xF && matches!(op, 0 | 4 | 8 | 13)) {
                    regs.set_reg(rd, res);
                }
                if s_flag {
                    regs.set_nz(res);
                }
            } else {
                // Registre decale : Rm << / >> type.
                let op = (w1 >> 5) & 0x7;
                let rm = (w2 & 0xF) as u8;
                let imm3 = ((w2 >> 12) & 0x7) as u32;
                let imm2 = ((w2 >> 6) & 0x3) as u32;
                let type_ = ((w2 >> 4) & 0x3) as u32;
                let shift = (imm3 << 2) | imm2;
                let val_op2 = shift_operand(regs.get_reg(rm), shift, type_);

                let res = match op {
                    0 => val_n & val_op2, // AND / TST
                    1 => val_n ^ val_op2, // EOR / TEQ
                    2 => val_n.wrapping_sub(val_op2), // SUB / CMP
                    3 => val_op2.wrapping_sub(val_n), // RSB
                    4 => val_n.wrapping_add(val_op2), // ADD / CMN
                    5 => val_n.wrapping_add(val_op2).wrapping_add(if regs.flag_c() { 1 } else { 0 }), // ADC
                    6 => val_n.wrapping_sub(val_op2).wrapping_sub(if !regs.flag_c() { 1 } else { 0 }), // SBC
                    7 => val_n | val_op2, // ORR
                    _ => val_n,
                };
                // TST/TEQ/CMP/CMN n'ecrivent pas de destination.
                if !(rd == 0xF && matches!(op, 0 | 1 | 2 | 4)) {
                    regs.set_reg(rd, res);
                }
                if s_flag {
                    regs.set_nz(res);
                }
            }
            return StepResult::Ok(1);
        }

        // 7. 32-bit Single Data Transfer: LDR / STR (Byte, Halfword, Word)
        if (w1 & 0xFE00) == 0xF800 || (w1 & 0xFE00) == 0xF900 {
            let is_ldr = (w1 & 0x0010) != 0;
            let size = (w1 >> 7) & 3; // 0 = byte, 1 = halfword, 2 = word
            let rn = (w1 & 0xF) as u8;
            let rd = ((w2 >> 12) & 0xF) as u8;

            let addr = if (w1 & 0x0080) != 0 {
                // Immediate 12-bit offset
                let imm12 = (w2 & 0x0FFF) as u32;
                let base = if rn == 0xF { regs.pc & !3 } else { regs.get_reg(rn) };
                base.wrapping_add(imm12)
            } else if (w2 & 0x0800) != 0 {
                // 8-bit immediate with sign/indexing
                let is_add = (w2 & 0x0200) != 0;
                let imm8 = (w2 & 0xFF) as u32;
                let base = if rn == 0xF { regs.pc & !3 } else { regs.get_reg(rn) };
                if is_add { base.wrapping_add(imm8) } else { base.wrapping_sub(imm8) }
            } else {
                // Register shifted offset
                let rm = (w2 & 0xF) as u8;
                let shift = ((w2 >> 4) & 3) as u32;
                let base = regs.get_reg(rn);
                let off = regs.get_reg(rm) << shift;
                base.wrapping_add(off)
            };

            if is_ldr {
                let val = match size {
                    0 => bus.read_u8(addr, periph, nvic) as u32,
                    1 => bus.read_u16(addr, periph, nvic) as u32,
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
            return StepResult::Ok(2);
        }

        // 8. 32-bit Multiple Load/Store: STMDB / LDMIA (1110 1000 10xx rn)
        if (w1 & 0xFE40) == 0xE800 || (w1 & 0xFE40) == 0xE840 {
            let is_ldm = (w1 & 0x0010) != 0;
            let rn = (w1 & 0xF) as u8;
            let reg_list = w2 & 0xFFFF;
            let mut addr = regs.get_reg(rn);

            for i in 0..16 {
                if (reg_list & (1 << i)) != 0 {
                    if is_ldm {
                        let v = bus.read_u32(addr, periph, nvic);
                        regs.set_reg(i as u8, v);
                    } else {
                        let v = regs.get_reg(i as u8);
                        bus.write_u32(addr, v, periph, nvic);
                    }
                    addr = addr.wrapping_add(4);
                }
            }
            regs.set_reg(rn, addr);
            return StepResult::Ok(3);
        }

        // 9. Data Barriers: DMB / DSB / ISB
        if (w1 & 0xFFF0) == 0xF3B0 && (w2 & 0xFFF0) == 0x8F40 {
            return StepResult::Ok(1);
        }

        // 10. MRS / MSR (Move to/from special register)
        if (w1 & 0xFFE0) == 0xF3E0 {
            let is_mrs = (w1 & 0x0010) == 0;
            let rd = ((w2 >> 8) & 0xF) as u8;
            if is_mrs {
                regs.set_reg(rd, regs.xpsr);
            } else {
                let val = regs.get_reg(rd);
                regs.xpsr = val;
            }
            return StepResult::Ok(2);
        }

        StepResult::Ok(1)
    }
}

/// Deplie un immediat modifie 12 bits (i:imm3:imm8) selon ThumbExpandImm.
fn thumb_expand_imm(imm12: u32) -> u32 {
    if imm12 & 0xC00 == 0 {
        // imm12[11:10] == 00 : imm8 decale de (imm3 * 8).
        let imm8 = imm12 & 0xFF;
        let shift = ((imm12 >> 8) & 0x3) * 8;
        imm8 << shift
    } else {
        // Rotation d'un octet 0b1:imm12[6:0] de imm12[11:7] bits.
        let unrotated = 0x80 | (imm12 & 0x7F);
        unrotated.rotate_right((imm12 >> 7) & 0x1F)
    }
}

/// Operande registre decale d'une instruction ALU 32 bits.
fn shift_operand(rm: u32, shift: u32, type_: u32) -> u32 {
    match type_ {
        0 => if shift == 0 { rm } else { rm << shift }, // LSL
        1 => if shift == 0 { 0 } else { rm >> shift }, // LSR #32 vaut 0
        2 => {
            // ASR, #0 equivaut a #32 (extension de signe).
            if shift == 0 {
                ((rm as i32) >> 31) as u32
            } else {
                ((rm as i32) >> shift) as u32
            }
        }
        3 => {
            // ROR ; shift == 0 est RRX, approximation sans carry (rare).
            if shift == 0 { rm >> 1 } else { rm.rotate_right(shift) }
        }
        _ => rm,
    }
}
