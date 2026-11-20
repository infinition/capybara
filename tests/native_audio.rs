use capybara::emulator::Machine;

fn write(machine: &mut Machine, address: u32, value: u32) {
    machine.bus.write_u32(address, value, &mut machine.periph, &mut machine.cpu.nvic);
}

#[test]
fn factory_pwm_plays_without_the_legacy_sram_sound_flag() {
    let mut machine = Machine::new();
    // Captured Tropical validation tone: the Water sound flag is zero.
    for (offset, value) in [(0x14, 0x600), (0x18, 0x2274f), (0x24, 0x23880),
        (0x34, 0x100001), (0x3c, 0x1f), (0, 1)] {
        write(&mut machine, 0x40007000 + offset, value);
    }
    assert!(machine.son_en_cours());
    assert!((machine.note_courante() - 494.7).abs() < 0.1);
    assert_eq!(machine.voix_audio(), vec![(machine.note_courante(), 1.0)]);
    // A stale factory voice flag must not sustain the sound after PWM stops.
    let flag = machine.adresse_son_en_cours();
    write(&mut machine, flag, 1);
    write(&mut machine, 0x40007034, 0);
    assert!(!machine.son_en_cours());
    assert_eq!(machine.note_courante(), 0.0);
    assert!(machine.voix_audio().is_empty());
    // Restarting the same note must work too.
    write(&mut machine, 0x40007034, 0x100001);
    assert!(machine.note_courante() > 0.0);
    write(&mut machine, 0x40007000, 0);
    assert_eq!(machine.note_courante(), 0.0);
}

#[test]
fn older_factory_roms_keep_the_voice_table_fallback() {
    let mut machine = Machine::new();
    let voice = Machine::VOIX_AUDIO;
    write(&mut machine, voice, 96_000_000);
    write(&mut machine, voice + 4, 1500);
    write(&mut machine, voice + 8, 1);
    write(&mut machine, voice + 12, 100);
    let flag = machine.adresse_son_en_cours();
    write(&mut machine, flag, 1);
    machine.localiser_les_voix();
    assert_eq!(machine.note_programme(), None);
    assert_eq!(machine.note_courante(), 500.0);
    write(&mut machine, flag, 0);
    assert_eq!(machine.note_courante(), 0.0);
}
