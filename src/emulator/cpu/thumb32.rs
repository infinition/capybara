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
        // Branch with Link (BL / BLX): 1111 0 s imm10  11 j1 1 j2 imm11
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
            regs.lr = (regs.pc | 1) + 2;
            regs.pc = (regs.pc as i32 + (imm25 as i32)) as u32;
            return StepResult::Ok(3);
        }

        // MOVW (Move 16-bit immediate): 1111 0 i 10 0100 imm4 0 imm3 rd imm8
        // MOVT (Move Top 16-bit): 1111 0 i 10 1100 imm4 0 imm3 rd imm8
        if (w1 & 0xFB70) == 0xF240 && (w2 & 0x8000) == 0 {
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

        // LDR.W / STR.W (immediate or register): 1111 100 x x 0 x x
        if (w1 & 0xFE50) == 0xF850 {
            let is_ldr = (w1 & 0x0010) != 0;
            let rn = (w1 & 0xF) as u8;
            let rd = ((w2 >> 12) & 0xF) as u8;
            let imm12 = (w2 & 0x0FFF) as u32;
            let addr = regs.get_reg(rn) + imm12;

            if is_ldr {
                let val = bus.read_u32(addr, periph, nvic);
                regs.set_reg(rd, val);
            } else {
                let val = regs.get_reg(rd);
                bus.write_u32(addr, val, periph, nvic);
            }
            return StepResult::Ok(2);
        }

        // Data Barriers: DMB / DSB / ISB: 1111 0011 1011 xxxx 1000 1111 xxxx xxxx
        if (w1 & 0xFFF0) == 0xF3B0 && (w2 & 0xFFF0) == 0x8F40 {
            return StepResult::Ok(1);
        }

        // MRS / MSR (Move to/from special register): 1111 0011 111x
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
