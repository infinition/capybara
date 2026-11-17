use capybara::emulator::Machine;
use capybara::emulator::etat::Instantane;

#[test]
fn snapshots_preserve_boot_info_and_homebrew_mailboxes() {
    let mut machine = Machine::new();
    machine.bus.sram.mailbox[0xF60..0xF64]
        .copy_from_slice(&0x20000000u32.to_le_bytes());
    machine.bus.sram.mailbox[0x818..0x81C]
        .copy_from_slice(&0x60011000u32.to_le_bytes());
    machine.bus.sram.mailbox[0xFFC..0x1000].copy_from_slice(b"PMXL");
    let snapshot = machine.instantane();
    machine.bus.sram.mailbox.fill(0);
    let restored: Instantane = serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap();
    machine.restaurer(&restored);
    assert_eq!(machine.bus.sram.mailbox, snapshot.mailbox);
}

#[test]
fn legacy_snapshots_keep_loader_installed_boot_info() {
    let mut machine = Machine::new();
    let mut legacy = serde_json::to_value(machine.instantane()).unwrap();
    legacy.as_object_mut().unwrap().remove("mailbox");
    let snapshot: Instantane = serde_json::from_value(legacy).unwrap();
    machine.bus.sram.mailbox[0x818..0x81C]
        .copy_from_slice(&0x60011000u32.to_le_bytes());
    machine.restaurer(&snapshot);
    assert_eq!(&machine.bus.sram.mailbox[0x818..0x81C], &0x60011000u32.to_le_bytes());
}
