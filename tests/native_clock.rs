use capybara::emulator::{Machine, peripherals::SnSysRegisters as Sys};

fn tick(sys: &mut Sys, seconds: u32) {
    for _ in 0..seconds { sys.tick(96_000_000); }
}

#[test]
fn setting_the_clock_discards_the_old_counter_and_fraction() {
    let mut sys = Sys::default();
    sys.write_reg(Sys::CONTROLE_RTC, 0xe0);
    tick(&mut sys, 6000);
    sys.tick(48_000_000);
    // Tropical's date-confirmation sequence stops and reconfigures ISO RTC.
    for value in [0x60, 0x20, 0x20, 0x60] {
        sys.write_reg(Sys::CONTROLE_RTC, value);
        sys.tick(96_000_000);
        assert_eq!(sys.read_reg(Sys::COMPTEUR_SECONDES), 0);
    }
    sys.write_reg(Sys::CONTROLE_RTC, 0xe0);
    sys.tick(48_000_000);
    assert_eq!(sys.secondes, 0);
    sys.tick(48_000_000);
    assert_eq!(sys.secondes, 1);
    // Applying a new offset of 15:54 must not add the former 6000 seconds.
    assert_eq!(15 * 3600 + 54 * 60 + sys.secondes, 57_241);
}

#[test]
fn enabled_rtc_configuration_and_cpu_reset_preserve_time() {
    let mut machine = Machine::new();
    machine.periph.snsys.write_reg(Sys::CONTROLE_RTC, 0xe0);
    machine.periph.snsys.write_reg(Sys::COMPTEUR_SECONDES, 12345);
    machine.periph.snsys.write_reg(Sys::CONTROLE_RTC, 0xe8);
    assert_eq!(machine.periph.snsys.secondes, 12345);
    machine.reset();
    assert_eq!(machine.periph.snsys.secondes, 12345);
    tick(&mut machine.periph.snsys, 1);
    let snapshot = machine.instantane();
    let mut restored = Machine::new();
    restored.restaurer(&snapshot);
    assert_eq!(restored.periph.snsys.secondes, 12346);
    tick(&mut restored.periph.snsys, 1);
    assert_eq!(restored.periph.snsys.secondes, 12347);
}
