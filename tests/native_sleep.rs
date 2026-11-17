use capybara::emulator::Machine;

#[test]
fn native_gpio_addresses_do_not_mean_the_cpu_is_asleep() {
    let mut machine = Machine::new();
    // Tropical uses this address for GPIO setup, unlike older Water builds.
    machine.cpu.regs.pc = 0x23E4;
    assert!(!machine.en_veille_profonde());
    machine.appuyer(Machine::BOUTON_B);
    assert_eq!(machine.cpu.regs.pc, 0x23E4);
    assert!(machine.broche_basse(Machine::BOUTON_B));
}

#[test]
fn wake_clears_the_hardware_sleep_request_and_migrates_old_snapshots() {
    let mut machine = Machine::new();
    machine.bus.pram.data[..4].copy_from_slice(&0x1801EF18u32.to_le_bytes());
    machine.bus.pram.data[4..8].copy_from_slice(&0x2F5u32.to_le_bytes());
    machine.periph.snsys.write_reg(0x300, 0x91);
    // Model a legacy snapshot with its register value but no new flag.
    machine.periph.snsys.power_down_requested = false;
    assert!(machine.en_veille_profonde());
    machine.appuyer(Machine::BOUTON_B);
    assert_eq!(machine.cpu.regs.pc, 0x2F4);
    assert_eq!(machine.periph.snsys.read_reg(0x300), 0x90);
    assert!(!machine.en_veille_profonde());
    machine.cpu.regs.pc = 0x23E4;
    machine.appuyer(Machine::BOUTON_B);
    assert_eq!(machine.cpu.regs.pc, 0x23E4);
}

#[test]
fn native_sound_timer_reset_acknowledges_without_sticking() {
    let mut machine = Machine::new();
    machine.bus.write_u32(0x40007008, 12345, &mut machine.periph, &mut machine.cpu.nvic);
    machine.bus.write_u32(0x40007000, 2, &mut machine.periph, &mut machine.cpu.nvic);
    assert_eq!(machine.bus.read_u32(0x40007000, &mut machine.periph, &machine.cpu.nvic) & 2, 0);
    assert_eq!(machine.bus.read_u32(0x40007008, &mut machine.periph, &machine.cpu.nvic), 0);
    machine.periph.timers.buzzer_regs[0] = 2; // pre-fix snapshot
    assert_eq!(machine.bus.read_u32(0x40007000, &mut machine.periph, &machine.cpu.nvic), 0);
}
