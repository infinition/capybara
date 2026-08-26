#[derive(Debug, Clone)]
pub struct DisassembledInst {
    pub address: u32,
    pub opcode_bytes: Vec<u8>,
    pub mnemonic: String,
    pub operands: String,
    pub is_32bit: bool,
}

pub struct Disassembler;

impl Disassembler {
    pub fn disassemble(address: u32, raw_words: &[u16]) -> DisassembledInst {
        if raw_words.is_empty() {
            return DisassembledInst {
                address,
                opcode_bytes: vec![0, 0],
                mnemonic: "???".to_string(),
                operands: "".to_string(),
                is_32bit: false,
            };
        }

        let w1 = raw_words[0];
        let is_32 = (w1 & 0xF800) == 0xE800 || (w1 & 0xF800) == 0xF000 || (w1 & 0xF800) == 0xF800;

        if is_32 && raw_words.len() >= 2 {
            let w2 = raw_words[1];
            let bytes = vec![
                (w1 & 0xFF) as u8,
                (w1 >> 8) as u8,
                (w2 & 0xFF) as u8,
                (w2 >> 8) as u8,
            ];
            let (mnem, ops) = Self::disasm_thumb32(address, w1, w2);
            DisassembledInst {
                address,
                opcode_bytes: bytes,
                mnemonic: mnem,
                operands: ops,
                is_32bit: true,
            }
        } else {
            let bytes = vec![(w1 & 0xFF) as u8, (w1 >> 8) as u8];
            let (mnem, ops) = Self::disasm_thumb16(address, w1);
            DisassembledInst {
                address,
                opcode_bytes: bytes,
                mnemonic: mnem,
                operands: ops,
                is_32bit: false,
            }
        }
    }

    fn disasm_thumb16(address: u32, w: u16) -> (String, String) {
        // NOP
        if w == 0xBF00 {
            return ("NOP".to_string(), "".to_string());
        }
        // WFI
        if w == 0xBF30 {
            return ("WFI".to_string(), "".to_string());
        }
        // BKPT
        if (w & 0xFF00) == 0xBE00 {
            return ("BKPT".to_string(), format!("#0x{:02X}", w & 0xFF));
        }

        // Push / Pop
        if (w & 0xFE00) == 0xB400 {
            let has_lr = (w & 0x0100) != 0;
            let mut list = Vec::new();
            for i in 0..8 {
                if (w & (1 << i)) != 0 {
                    list.push(format!("r{}", i));
                }
            }
            if has_lr {
                list.push("lr".to_string());
            }
            return ("PUSH".to_string(), format!("{{{}}}", list.join(", ")));
        }
        if (w & 0xFE00) == 0xBC00 {
            let has_pc = (w & 0x0100) != 0;
            let mut list = Vec::new();
            for i in 0..8 {
                if (w & (1 << i)) != 0 {
                    list.push(format!("r{}", i));
                }
            }
            if has_pc {
                list.push("pc".to_string());
            }
            return ("POP".to_string(), format!("{{{}}}", list.join(", ")));
        }

        // Mov immediate: 0010 0 rrd imm8
        if (w & 0xF800) == 0x2000 {
            let rd = (w >> 8) & 0x7;
            let imm = w & 0xFF;
            return ("MOVS".to_string(), format!("r{}, #{}", rd, imm));
        }

        // Cmp immediate: 0010 1 rrn imm8
        if (w & 0xF800) == 0x2800 {
            let rn = (w >> 8) & 0x7;
            let imm = w & 0xFF;
            return ("CMP".to_string(), format!("r{}, #{}", rn, imm));
        }

        // Add immediate: 0011 0 rrd imm8
        if (w & 0xF800) == 0x3000 {
            let rd = (w >> 8) & 0x7;
            let imm = w & 0xFF;
            return ("ADDS".to_string(), format!("r{}, #{}", rd, imm));
        }

        // Sub immediate: 0011 1 rrd imm8
        if (w & 0xF800) == 0x3800 {
            let rd = (w >> 8) & 0x7;
            let imm = w & 0xFF;
            return ("SUBS".to_string(), format!("r{}, #{}", rd, imm));
        }

        // Ldr PC-relative: 0100 1 rrd imm8
        if (w & 0xF800) == 0x4800 {
            let rd = (w >> 8) & 0x7;
            let imm = ((w & 0xFF) as u32) * 4;
            let target = (address & !3) + 4 + imm;
            return ("LDR".to_string(), format!("r{}, [pc, #{}] ; 0x{:08X}", rd, imm, target));
        }

        // Ldr / Str SP-relative: 1001 x rrd imm8
        if (w & 0xF000) == 0x9000 {
            let is_ldr = (w & 0x0800) != 0;
            let rd = (w >> 8) & 0x7;
            let imm = (w & 0xFF) * 4;
            let op = if is_ldr { "LDR" } else { "STR" };
            return (op.to_string(), format!("r{}, [sp, #{}]", rd, imm));
        }

        // Conditional branch: 1101 cond imm8
        if (w & 0xF000) == 0xD000 && (w & 0x0F00) != 0x0E00 && (w & 0x0F00) != 0x0F00 {
            let cond = (w >> 8) & 0xF;
            let imm8 = (w & 0xFF) as i8 as i32;
            let target = (address as i32 + 4 + (imm8 * 2)) as u32;
            let cond_name = match cond {
                0 => "BEQ",
                1 => "BNE",
                2 => "BCS",
                3 => "BCC",
                4 => "BMI",
                5 => "BPL",
                6 => "BVS",
                7 => "BVC",
                8 => "BHI",
                9 => "BLS",
                10 => "BGE",
                11 => "BLT",
                12 => "BGT",
                13 => "BLE",
                _ => "B",
            };
            return (cond_name.to_string(), format!("0x{:08X}", target));
        }

        // Unconditional branch: 1110 0 imm11
        if (w & 0xF800) == 0xE000 {
            let mut imm11 = (w & 0x07FF) as i32;
            if (imm11 & 0x0400) != 0 {
                imm11 |= !0x07FF; // Sign extend
            }
            let target = (address as i32 + 4 + (imm11 * 2)) as u32;
            return ("B".to_string(), format!("0x{:08X}", target));
        }

        // BX / BLX reg: 0100 0111 x rrm 000
        if (w & 0xFF80) == 0x4700 {
            let is_blx = (w & 0x0080) != 0;
            let rm = (w >> 3) & 0xF;
            let op = if is_blx { "BLX" } else { "BX" };
            return (op.to_string(), format!("r{}", rm));
        }

        ("THUMB16".to_string(), format!("0x{:04X}", w))
    }

    fn disasm_thumb32(address: u32, w1: u16, w2: u16) -> (String, String) {
        // BL / BLX: 1111 0 s imm10  11 j1 1 j2 imm11
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
            let target = (address as i32 + 4 + (imm25 as i32)) as u32;
            return ("BL".to_string(), format!("0x{:08X}", target));
        }

        // MOVW / MOVT
        if (w1 & 0xFB70) == 0xF240 && (w2 & 0x8000) == 0 {
            let is_movt = (w1 & 0x0080) != 0;
            let rd = ((w2 >> 8) & 0xF) as u8;
            let imm4 = (w1 & 0xF) as u32;
            let i = ((w1 >> 10) & 1) as u32;
            let imm3 = ((w2 >> 12) & 7) as u32;
            let imm8 = (w2 & 0xFF) as u32;
            let imm16 = (imm4 << 12) | (i << 11) | (imm3 << 8) | imm8;
            let op = if is_movt { "MOVT" } else { "MOVW" };
            return (op.to_string(), format!("r{}, #0x{:04X}", rd, imm16));
        }

        ("THUMB32".to_string(), format!("0x{:04X} 0x{:04X}", w1, w2))
    }
}
