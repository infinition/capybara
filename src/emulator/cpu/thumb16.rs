use super::registers::Registers;
use super::thumb32::add_with_carry;
use crate::emulator::cpu::nvic::Nvic;
use crate::emulator::mmu::MemoryBus;
use crate::emulator::peripherals::Peripherals;

pub enum StepResult {
    Ok(u32), // Cycles consumed
    Breakpoint,
    Halt,
    Undefined(u16),
}

pub struct Thumb16;

impl Thumb16 {
    pub fn execute(
        w: u16,
        regs: &mut Registers,
        bus: &mut MemoryBus,
        periph: &mut Peripherals,
        nvic: &mut Nvic,
    ) -> StepResult {
        // Aiguillage sur le quartet haut. C'etait une chaine d'une quinzaine de
        // tests de masque parcourue a chaque instruction, et les formes les plus
        // frequentes se trouvaient a la fin : les acces relatifs au pointeur de
        // pile en 1001, le groupe divers en 1011, les branchements en 1101 et
        // 1110. Le quartet haut suffit a separer tous les groupes, le compilateur
        // en fait une table de saut.
        match w >> 12 {
            // Decalage par immediat, et addition ou soustraction a trois registres.
            0x0 | 0x1 => Self::exec_decalage(w, regs),

            // Deplacement, comparaison, addition et soustraction immediates.
            0x2 | 0x3 => Self::exec_imm8(w, regs),

            0x4 => {
                if (w & 0x0C00) == 0x0000 {
                    // Traitement de donnees : 0100 00 op rm rdn
                    Self::exec_alu(w, regs)
                } else if (w & 0x0C00) == 0x0400 {
                    // Registres hauts, BX et BLX : 0100 01 op h1 h2 rm rdn
                    Self::exec_high_reg(w, regs)
                } else {
                    // LDR relatif au PC : 0100 1 rd imm8
                    let rd = ((w >> 8) & 0x7) as u8;
                    let imm = ((w & 0xFF) as u32) * 4;
                    let addr = ((regs.pc + 2) & !3) + imm;
                    let val = bus.read_u32(addr, periph, nvic);
                    regs.set_reg(rd, val);
                    return StepResult::Ok(2);
                }
            }

            // Chargement et rangement a offset registre : 0101 op rm rn rd
            0x5 => Self::exec_ldr_str_reg(w, regs, bus, periph, nvic),

            // Mot et octet a offset immediat : 011 op imm5 rn rd
            0x6 | 0x7 => Self::exec_ldr_str_imm(w, regs, bus, periph, nvic),

            // Demi mot a offset immediat : 1000 op imm5 rn rd
            0x8 => Self::exec_ldr_str_half(w, regs, bus, periph, nvic),

            // Relatif au pointeur de pile : 1001 op rd imm8
            0x9 => {
                let is_ldr = (w & 0x0800) != 0;
                let rd = ((w >> 8) & 0x7) as u8;
                let imm = ((w & 0xFF) as u32) * 4;
                let addr = regs.get_sp() + imm;
                if is_ldr {
                    let val = bus.read_u32(addr, periph, nvic);
                    regs.set_reg(rd, val);
                } else {
                    let val = regs.get_reg(rd);
                    bus.write_u32(addr, val, periph, nvic);
                }
                return StepResult::Ok(2);
            }

            // Adresse relative au pointeur de pile ou au PC : 1010 op rd imm8
            0xA => {
                let is_sp = (w & 0x0800) != 0;
                let rd = ((w >> 8) & 0x7) as u8;
                let imm = ((w & 0xFF) as u32) * 4;
                let base = if is_sp { regs.get_sp() } else { (regs.pc + 2) & !3 };
                regs.set_reg(rd, base + imm);
                return StepResult::Ok(1);
            }

            0xB => {
                // IT : 1011 1111 cccc mmmm, avec masque non nul. Un masque nul
                // designe les indications NOP, WFI, WFE et SEV.
                if (w & 0xFF00) == 0xBF00 {
                    if (w & 0x000F) != 0 {
                        regs.itstate = (w & 0xFF) as u8;
                    }
                    // NOP, WFI, WFE et SEV n'ont aucun effet ici.
                    return StepResult::Ok(1);
                }
                // BKPT
                if (w & 0xFF00) == 0xBE00 {
                    return StepResult::Breakpoint;
                }
                Self::exec_misc(w, regs, bus, periph, nvic)
            }

            // Acces multiples : 1100 op rn reg_list
            0xC => Self::exec_ldm_stm(w, regs, bus, periph, nvic),

            // Branchement conditionnel : 1101 cond imm8. Les conditions 1110 et
            // 1111 sont UDF et SVC, que le modele ne traite pas.
            0xD => {
                let cond = (w >> 8) & 0xF;
                if cond >= 0xE {
                    return StepResult::Undefined(w);
                }
                let imm8 = (w & 0xFF) as i8 as i32;
                if Self::eval_condition(cond, regs) {
                    // Le PC architectural vaut adresse + 4, or step() n'a avance
                    // que de 2 pour une instruction 16 bits : il manque 2.
                    regs.pc = (regs.pc as i32 + 2 + (imm8 * 2)) as u32;
                    return StepResult::Ok(2);
                }
                StepResult::Ok(1)
            }

            // Branchement inconditionnel : 1110 0 imm11. Au dela de 1110 1 on est
            // dans une instruction de 32 bits, qui n'arrive pas ici.
            0xE if (w & 0x0800) == 0 => {
                let mut imm11 = (w & 0x07FF) as i32;
                if (imm11 & 0x0400) != 0 {
                    imm11 |= !0x07FF;
                }
                regs.pc = (regs.pc as i32 + 2 + (imm11 * 2)) as u32;
                StepResult::Ok(2)
            }

            _ => StepResult::Undefined(w),
        }
    }

    /// Decalage par immediat, et addition ou soustraction a trois registres.
    fn exec_decalage(w: u16, regs: &mut Registers) -> StepResult {
        let op = (w >> 11) & 0x3;
        let imm5 = ((w >> 6) & 0x1F) as u32;
        let rm = (w >> 3) & 0x7;
        let rd = w & 0x7;
        let rm_val = regs.get_reg(rm as u8);

        let res = match op {
            0 => {
                // LSL
                if imm5 == 0 {
                    rm_val
                } else {
                    regs.set_flag_c((rm_val & (1 << (32 - imm5))) != 0);
                    rm_val << imm5
                }
            }
            1 => {
                // LSR
                let shift = if imm5 == 0 { 32 } else { imm5 };
                regs.set_flag_c((rm_val & (1 << (shift - 1))) != 0);
                if shift >= 32 { 0 } else { rm_val >> shift }
            }
            2 => {
                // ASR
                let shift = if imm5 == 0 { 32 } else { imm5 };
                regs.set_flag_c((rm_val & (1 << (shift - 1))) != 0);
                let s = rm_val as i32;
                if shift >= 32 {
                    if s < 0 { 0xFFFF_FFFF } else { 0 }
                } else {
                    (s >> shift) as u32
                }
            }
            _ => {
                // Add/Sub 3-register: 0001 10 op rn rd / rm rn rd
                return Self::exec_add_sub(w, regs);
            }
        };

        regs.set_reg(rd as u8, res);
        regs.set_nz(res);
        return StepResult::Ok(1);
    }

    /// Deplacement, comparaison, addition et soustraction immediates.
    fn exec_imm8(w: u16, regs: &mut Registers) -> StepResult {
        let op = (w >> 11) & 0x3;
        let rd = ((w >> 8) & 0x7) as u8;
        let imm8 = (w & 0xFF) as u32;

        match op {
            0 => {
                // MOVS rd, #imm8
                regs.set_reg(rd, imm8);
                regs.set_nz(imm8);
            }
            1 => {
                // CMP rn, #imm8
                let rn_val = regs.get_reg(rd);
                let (res, borrow) = rn_val.overflowing_sub(imm8);
                regs.set_nz(res);
                regs.set_flag_c(!borrow);
                regs.set_flag_v(((rn_val ^ imm8) & (rn_val ^ res) & 0x8000_0000) != 0);
            }
            2 => {
                // ADDS rd, #imm8
                let rd_val = regs.get_reg(rd);
                let (res, carry) = rd_val.overflowing_add(imm8);
                regs.set_reg(rd, res);
                regs.set_nz(res);
                regs.set_flag_c(carry);
                regs.set_flag_v((!(rd_val ^ imm8) & (rd_val ^ res) & 0x8000_0000) != 0);
            }
            3 => {
                // SUBS rd, #imm8
                let rd_val = regs.get_reg(rd);
                let (res, borrow) = rd_val.overflowing_sub(imm8);
                regs.set_reg(rd, res);
                regs.set_nz(res);
                regs.set_flag_c(!borrow);
                regs.set_flag_v(((rd_val ^ imm8) & (rd_val ^ res) & 0x8000_0000) != 0);
            }
            _ => {}
        }
        return StepResult::Ok(1);
    }

    fn exec_add_sub(w: u16, regs: &mut Registers) -> StepResult {
        let is_sub = (w & 0x0200) != 0;
        let is_imm = (w & 0x0400) != 0;
        let rn = ((w >> 3) & 0x7) as u8;
        let rd = (w & 0x7) as u8;
        let val1 = regs.get_reg(rn);
        let val2 = if is_imm {
            ((w >> 6) & 0x7) as u32
        } else {
            let rm = ((w >> 6) & 0x7) as u8;
            regs.get_reg(rm)
        };

        if is_sub {
            let (res, borrow) = val1.overflowing_sub(val2);
            regs.set_reg(rd, res);
            regs.set_nz(res);
            regs.set_flag_c(!borrow);
            regs.set_flag_v(((val1 ^ val2) & (val1 ^ res) & 0x8000_0000) != 0);
        } else {
            let (res, carry) = val1.overflowing_add(val2);
            regs.set_reg(rd, res);
            regs.set_nz(res);
            regs.set_flag_c(carry);
            regs.set_flag_v((!(val1 ^ val2) & (val1 ^ res) & 0x8000_0000) != 0);
        }

        StepResult::Ok(1)
    }

    /// Traitement de donnees 16 bits, forme registre : 0100 00 oooo mmm ddd.
    ///
    /// La table etait incomplete : CMP, CMN, ADC, SBC, RSB, LSR, ASR et ROR
    /// tombaient dans la branche par defaut. CMP en particulier ne posait aucun
    /// drapeau, ce qui faisait echouer tous les BEQ et BNE qui la suivent.
    fn exec_alu(w: u16, regs: &mut Registers) -> StepResult {
        let op = (w >> 6) & 0xF;
        let rm = ((w >> 3) & 0x7) as u8;
        let rdn = (w & 0x7) as u8;
        let val_m = regs.get_reg(rm);
        let val_dn = regs.get_reg(rdn);
        let carry_in = regs.flag_c();

        // Retenue et debordement ne bougent que si l'operation les produit.
        let mut c_out = carry_in;
        let mut v_out = regs.flag_v();
        // TST, CMP et CMN ne rangent pas leur resultat.
        let mut writes_back = true;

        let res = match op {
            0 => val_dn & val_m,  // AND
            1 => val_dn ^ val_m,  // EOR
            2 => {
                // LSL par registre, decalage sur les 8 bits de poids faible.
                let (v, c) = shift_by_reg(val_dn, val_m & 0xFF, 0, carry_in);
                c_out = c;
                v
            }
            3 => {
                // LSR
                let (v, c) = shift_by_reg(val_dn, val_m & 0xFF, 1, carry_in);
                c_out = c;
                v
            }
            4 => {
                // ASR
                let (v, c) = shift_by_reg(val_dn, val_m & 0xFF, 2, carry_in);
                c_out = c;
                v
            }
            5 => {
                // ADC
                let (v, c, o) = add_with_carry(val_dn, val_m, carry_in);
                c_out = c;
                v_out = o;
                v
            }
            6 => {
                // SBC
                let (v, c, o) = add_with_carry(val_dn, !val_m, carry_in);
                c_out = c;
                v_out = o;
                v
            }
            7 => {
                // ROR
                let (v, c) = shift_by_reg(val_dn, val_m & 0xFF, 3, carry_in);
                c_out = c;
                v
            }
            8 => {
                // TST
                writes_back = false;
                val_dn & val_m
            }
            9 => {
                // RSB rd, rm, #0, alias NEG
                let (v, c, o) = add_with_carry(!val_m, 0, true);
                c_out = c;
                v_out = o;
                v
            }
            10 => {
                // CMP
                writes_back = false;
                let (v, c, o) = add_with_carry(val_dn, !val_m, true);
                c_out = c;
                v_out = o;
                v
            }
            11 => {
                // CMN
                writes_back = false;
                let (v, c, o) = add_with_carry(val_dn, val_m, false);
                c_out = c;
                v_out = o;
                v
            }
            12 => val_dn | val_m,             // ORR
            13 => val_dn.wrapping_mul(val_m), // MUL
            14 => val_dn & !val_m,            // BIC
            _ => !val_m,                      // MVN
        };

        if writes_back {
            regs.set_reg(rdn, res);
        }
        regs.set_nz(res);
        regs.set_flag_c(c_out);
        regs.set_flag_v(v_out);
        StepResult::Ok(1)
    }

    fn exec_high_reg(w: u16, regs: &mut Registers) -> StepResult {
        let op = (w >> 8) & 0x3;
        let h1 = (w >> 7) & 1;
        let h2 = (w >> 6) & 1;
        let rm = (((w >> 3) & 0x7) | (h2 << 3)) as u8;
        let rdn = ((w & 0x7) | (h1 << 3)) as u8;

        // Lire R15 doit rendre le PC architectural, soit l'adresse de
        // l'instruction plus 4. Or step() n'a avance que de 2 pour une
        // instruction 16 bits. Sans ce rattrapage, la sequence de code
        // relogeable LDR.W r11, [pc, #..] / ADD r11, pc calcule un pointeur de
        // fonction deux octets trop bas, qui tombe sur le BX lr de la fonction
        // precedente au lieu de son point d'entree.
        let lire = |r: u8| if r == 15 { regs.pc.wrapping_add(2) } else { regs.get_reg(r) };
        let val_m = lire(rm);

        match op {
            0 => {
                // ADD
                let val_dn = lire(rdn);
                regs.set_reg(rdn, val_dn.wrapping_add(val_m));
            }
            1 => {
                // CMP
                let val_dn = lire(rdn);
                let (res, borrow) = val_dn.overflowing_sub(val_m);
                regs.set_nz(res);
                regs.set_flag_c(!borrow);
                regs.set_flag_v(((val_dn ^ val_m) & (val_dn ^ res) & 0x8000_0000) != 0);
            }
            2 => {
                // MOV
                regs.set_reg(rdn, val_m);
            }
            3 => {
                // BX / BLX
                let is_blx = (w & 0x0080) != 0;
                if is_blx {
                    // step() a deja avance de 2 : regs.pc est l'adresse de retour.
                    // L'ancien calcul ajoutait 2 de trop et sautait une instruction.
                    regs.lr = regs.pc | 1;
                }
                regs.pc = val_m & !1;
                return StepResult::Ok(2);
            }
            _ => {}
        }
        StepResult::Ok(1)
    }

    fn exec_ldr_str_reg(
        w: u16,
        regs: &mut Registers,
        bus: &mut MemoryBus,
        periph: &mut Peripherals,
        nvic: &mut Nvic,
    ) -> StepResult {
        let op = (w >> 9) & 0x7;
        let rm = ((w >> 6) & 0x7) as u8;
        let rn = ((w >> 3) & 0x7) as u8;
        let rd = (w & 0x7) as u8;
        let addr = regs.get_reg(rn).wrapping_add(regs.get_reg(rm));

        match op {
            0 => bus.write_u32(addr, regs.get_reg(rd), periph, nvic), // STR
            1 => bus.write_u16(addr, regs.get_reg(rd) as u16, periph, nvic), // STRH
            2 => bus.write_u8(addr, regs.get_reg(rd) as u8, periph, nvic), // STRB
            4 => {
                let v = bus.read_u32(addr, periph, nvic);
                regs.set_reg(rd, v);
            } // LDR
            5 => {
                let v = bus.read_u16(addr, periph, nvic) as u32;
                regs.set_reg(rd, v);
            } // LDRH
            6 => {
                let v = bus.read_u8(addr, periph, nvic) as u32;
                regs.set_reg(rd, v);
            } // LDRB
            // Les deux formes a extension de signe manquaient, et ne faisaient
            // rien : le registre gardait sa valeur d'avant. Le firmware lit par
            // LDRSB la borne d'une boucle de remplissage dans une table de
            // petits entiers signes ; sans chargement, la borne restait
            // l'adresse de la table et le remplissage ecrasait la memoire
            // jusqu'a corrompre ses propres pointeurs.
            3 => {
                let v = bus.read_u8(addr, periph, nvic) as i8 as i32 as u32;
                regs.set_reg(rd, v);
            } // LDRSB
            7 => {
                let v = bus.read_u16(addr, periph, nvic) as i16 as i32 as u32;
                regs.set_reg(rd, v);
            } // LDRSH
            _ => {}
        }
        StepResult::Ok(2)
    }

    fn exec_ldr_str_imm(
        w: u16,
        regs: &mut Registers,
        bus: &mut MemoryBus,
        periph: &mut Peripherals,
        nvic: &mut Nvic,
    ) -> StepResult {
        let is_byte = (w & 0x1000) != 0;
        let is_ldr = (w & 0x0800) != 0;
        let imm5 = ((w >> 6) & 0x1F) as u32;
        let rn = ((w >> 3) & 0x7) as u8;
        let rd = (w & 0x7) as u8;
        let imm = if is_byte { imm5 } else { imm5 * 4 };
        let addr = regs.get_reg(rn) + imm;

        if is_byte {
            if is_ldr {
                let v = bus.read_u8(addr, periph, nvic) as u32;
                regs.set_reg(rd, v);
            } else {
                bus.write_u8(addr, regs.get_reg(rd) as u8, periph, nvic);
            }
        } else if is_ldr {
            let v = bus.read_u32(addr, periph, nvic);
            regs.set_reg(rd, v);
        } else {
            bus.write_u32(addr, regs.get_reg(rd), periph, nvic);
        }

        StepResult::Ok(2)
    }

    fn exec_ldr_str_half(
        w: u16,
        regs: &mut Registers,
        bus: &mut MemoryBus,
        periph: &mut Peripherals,
        nvic: &mut Nvic,
    ) -> StepResult {
        let is_ldr = (w & 0x0800) != 0;
        let imm5 = (((w >> 6) & 0x1F) as u32) * 2;
        let rn = ((w >> 3) & 0x7) as u8;
        let rd = (w & 0x7) as u8;
        let addr = regs.get_reg(rn) + imm5;

        if is_ldr {
            let v = bus.read_u16(addr, periph, nvic) as u32;
            regs.set_reg(rd, v);
        } else {
            bus.write_u16(addr, regs.get_reg(rd) as u16, periph, nvic);
        }
        StepResult::Ok(2)
    }

    fn exec_misc(
        w: u16,
        regs: &mut Registers,
        bus: &mut MemoryBus,
        periph: &mut Peripherals,
        nvic: &mut Nvic,
    ) -> StepResult {
        // PUSH and POP first. They are the two commonest forms in this group —
        // every function call runs a pair — yet they sat behind six mask tests.
        // Their ranges, 0xB4xx-0xB5xx and 0xBCxx-0xBDxx, overlap none of the
        // forms moved below, so the order changes nothing that gets decoded.
        //
        // The words are gathered, then stored in one go. The array runs from
        // the low address to the high one, the stack's own order: low registers
        // at the bottom, LR or PC at the top.

        // PUSH: 1011 010 m reg_list
        if (w & 0xFE00) == 0xB400 {
            let has_lr = (w & 0x0100) != 0;
            let list = w & 0xFF;
            let mut mots = [0u32; 9];
            let mut n = 0usize;
            for i in 0..8u8 {
                if (list & (1 << i)) != 0 {
                    mots[n] = regs.get_reg(i);
                    n += 1;
                }
            }
            if has_lr {
                mots[n] = regs.lr;
                n += 1;
            }
            let sp = regs.get_sp().wrapping_sub(4 * n as u32);
            if !bus.ecrire_mots(sp, &mots[..n]) {
                let mut a = sp;
                for v in &mots[..n] {
                    bus.write_u32(a, *v, periph, nvic);
                    a = a.wrapping_add(4);
                }
            }
            regs.set_sp(sp);
            return StepResult::Ok(2);
        }

        // POP: 1011 110 p reg_list
        if (w & 0xFE00) == 0xBC00 {
            let has_pc = (w & 0x0100) != 0;
            let list = w & 0xFF;
            let n = list.count_ones() as usize + has_pc as usize;
            let sp = regs.get_sp();
            let mut mots = [0u32; 9];
            if !bus.lire_mots(sp, &mut mots[..n]) {
                let mut a = sp;
                for m in mots[..n].iter_mut() {
                    *m = bus.read_u32(a, periph, nvic);
                    a = a.wrapping_add(4);
                }
            }
            let mut k = 0usize;
            for i in 0..8u8 {
                if (list & (1 << i)) != 0 {
                    regs.set_reg(i, mots[k]);
                    k += 1;
                }
            }
            if has_pc {
                // The Thumb bit is not part of the address.
                regs.pc = mots[k] & !1;
            }
            regs.set_sp(sp.wrapping_add(4 * n as u32));
            return StepResult::Ok(2);
        }

        // Adjust SP: 1011 0000 s imm7
        if (w & 0xFF00) == 0xB000 {
            let is_sub = (w & 0x0080) != 0;
            let imm = ((w & 0x007F) as u32) * 4;
            let sp = regs.get_sp();
            regs.set_sp(if is_sub { sp - imm } else { sp + imm });
            return StepResult::Ok(1);
        }

        // CBZ / CBNZ: 1011 op 0 i 1 imm5 rn
        if (w & 0xF500) == 0xB100 {
            let is_cbnz = (w & 0x0800) != 0;
            let i = ((w >> 9) & 1) as u32;
            let imm5 = ((w >> 3) & 0x1F) as u32;
            let rn = (w & 0x7) as u8;
            let imm = (i << 6) | (imm5 << 1);
            let val = regs.get_reg(rn);
            if (is_cbnz && val != 0) || (!is_cbnz && val == 0) {
                // Le PC architectural vaut adresse + 4, or step() n'a avance
                // que de 2 pour une instruction 16 bits : il manque 2.
                regs.pc = regs.pc.wrapping_add(2).wrapping_add(imm);
                return StepResult::Ok(2);
            }
            return StepResult::Ok(1);
        }

        // SXTH / SXTB / UXTH / UXTB: 1011 0010 op rm rd
        if (w & 0xFF00) == 0xB200 {
            let op = (w >> 6) & 3;
            let rm = ((w >> 3) & 7) as u8;
            let rd = (w & 7) as u8;
            let rm_val = regs.get_reg(rm);
            let res = match op {
                0 => (rm_val as i16 as i32) as u32, // SXTH
                1 => (rm_val as i8 as i32) as u32,  // SXTB
                2 => rm_val & 0xFFFF,               // UXTH
                3 => rm_val & 0xFF,                 // UXTB
                _ => rm_val,
            };
            regs.set_reg(rd, res);
            return StepResult::Ok(1);
        }

        // REV / REV16 / REVSH: 1011 1010 op rm rd
        if (w & 0xFF00) == 0xBA00 {
            let op = (w >> 6) & 3;
            let rm = ((w >> 3) & 7) as u8;
            let rd = (w & 7) as u8;
            let rm_val = regs.get_reg(rm);
            let res = match op {
                0 => rm_val.swap_bytes(), // REV
                1 => ((rm_val & 0x00FF00FF) << 8) | ((rm_val & 0xFF00FF00) >> 8), // REV16
                3 => (rm_val as i16).swap_bytes() as i32 as u32, // REVSH
                _ => rm_val,
            };
            regs.set_reg(rd, res);
            return StepResult::Ok(1);
        }

        // CPSID / CPSIE: 1011 0110 011 x
        if (w & 0xFFE0) == 0xB660 {
            let is_disable = (w & 0x0010) != 0;
            regs.primask = if is_disable { 1 } else { 0 };
            return StepResult::Ok(1);
        }

        StepResult::Ok(1)
    }

    fn exec_ldm_stm(
        w: u16,
        regs: &mut Registers,
        bus: &mut MemoryBus,
        periph: &mut Peripherals,
        nvic: &mut Nvic,
    ) -> StepResult {
        let is_ldm = (w & 0x0800) != 0;
        let rn = ((w >> 8) & 0x7) as u8;
        let list = w & 0xFF;
        let base = regs.get_reg(rn);
        let n = list.count_ones() as usize;
        let mut mots = [0u32; 8];

        if is_ldm {
            if !bus.lire_mots(base, &mut mots[..n]) {
                let mut a = base;
                for m in mots[..n].iter_mut() {
                    *m = bus.read_u32(a, periph, nvic);
                    a = a.wrapping_add(4);
                }
            }
            let mut k = 0usize;
            for i in 0..8u8 {
                if (list & (1 << i)) != 0 {
                    regs.set_reg(i, mots[k]);
                    k += 1;
                }
            }
        } else {
            let mut k = 0usize;
            for i in 0..8u8 {
                if (list & (1 << i)) != 0 {
                    mots[k] = regs.get_reg(i);
                    k += 1;
                }
            }
            if !bus.ecrire_mots(base, &mots[..n]) {
                let mut a = base;
                for v in &mots[..n] {
                    bus.write_u32(a, *v, periph, nvic);
                    a = a.wrapping_add(4);
                }
            }
        }
        // Unconditional writeback, as before: the model does not handle the
        // case where the base register appears in the list.
        regs.set_reg(rn, base.wrapping_add(4 * n as u32));
        StepResult::Ok(2)
    }

    pub(crate) fn eval_condition(cond: u16, regs: &Registers) -> bool {
        match cond {
            0 => regs.flag_z(),                 // EQ
            1 => !regs.flag_z(),                // NE
            2 => regs.flag_c(),                 // CS / HS
            3 => !regs.flag_c(),                // CC / LO
            4 => regs.flag_n(),                 // MI
            5 => !regs.flag_n(),                // PL
            6 => regs.flag_v(),                 // VS
            7 => !regs.flag_v(),                // VC
            8 => regs.flag_c() && !regs.flag_z(), // HI
            9 => !regs.flag_c() || regs.flag_z(), // LS
            10 => regs.flag_n() == regs.flag_v(), // GE
            11 => regs.flag_n() != regs.flag_v(), // LT
            12 => !regs.flag_z() && (regs.flag_n() == regs.flag_v()), // GT
            13 => regs.flag_z() || (regs.flag_n() != regs.flag_v()),  // LE
            _ => true,
        }
    }
}

/// Decalage par registre, avec la retenue sortante. Un decalage nul laisse la
/// retenue intacte, une amplitude superieure a 32 vide le registre.
fn shift_by_reg(val: u32, amount: u32, type_: u32, carry_in: bool) -> (u32, bool) {
    if amount == 0 {
        return (val, carry_in);
    }
    match type_ {
        // LSL
        0 => match amount {
            n if n < 32 => (val << n, (val >> (32 - n)) & 1 != 0),
            32 => (0, val & 1 != 0),
            _ => (0, false),
        },
        // LSR
        1 => match amount {
            n if n < 32 => (val >> n, (val >> (n - 1)) & 1 != 0),
            32 => (0, (val >> 31) & 1 != 0),
            _ => (0, false),
        },
        // ASR
        2 => {
            if amount < 32 {
                (((val as i32) >> amount) as u32, (val >> (amount - 1)) & 1 != 0)
            } else {
                let s = ((val as i32) >> 31) as u32;
                (s, (val >> 31) & 1 != 0)
            }
        }
        // ROR
        _ => {
            let n = amount % 32;
            if n == 0 {
                (val, (val >> 31) & 1 != 0)
            } else {
                let v = val.rotate_right(n);
                (v, (v >> 31) & 1 != 0)
            }
        }
    }
}
