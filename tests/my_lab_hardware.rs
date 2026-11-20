use capybara::emulator::{peripherals::DisplayController, Machine};

#[test]
fn my_lab_panel_rotates_a_window_into_the_landscape_framebuffer() {
    let mut panel = DisplayController::for_my_lab();
    for (command, range) in [(0x2a, [0, 126, 0, 127]), (0x2b, [0, 159, 0, 159])] {
        panel.write_panel_byte(false, command);
        for byte in range {
            panel.write_panel_byte(true, byte);
        }
    }
    panel.write_panel_byte(false, 0x2c);
    panel.write_panel_byte(true, 0xf8);
    panel.write_panel_byte(true, 0);
    panel.recevoir_trame(&[0x07e0]);
    assert_eq!((panel.width, panel.height, panel.vram.len()), (160, 128, 20480));
    assert_eq!(panel.vram[126 * 160], 0xf800);
    assert_eq!(panel.vram[127 * 160], 0x07e0);
    assert_eq!(panel.trames, 1);
}

#[test]
fn my_lab_aes_engine_encrypts_a_block_through_mmio() {
    let mut machine = Machine::new();
    let aes = 0x4002_9000;
    let mut write = |offset, value| {
        machine.bus.write_u32(aes + offset, value, &mut machine.periph, &mut machine.cpu.nvic);
    };
    let offsets = [0x1c, 0x20, 0x24, 0x28, 0x40, 0x44, 0x80, 0x84];
    // AES-256 ECB NIST vector. The Sonix registers place the low word first.
    for (index, offset) in offsets.into_iter().enumerate() {
        let top = 31 - 4 * index as u8;
        write(offset, u32::from_le_bytes([top, top - 1, top - 2, top - 3]));
    }
    write(0x10, 0x1800_0100);
    write(0x14, 0x1800_0200);
    write(0x18, 0);
    machine.bus.sram.data[0x100..0x110].copy_from_slice(&[
        0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0,
    ]);
    machine.bus.write_u32(aes, 0x6000_0001, &mut machine.periph, &mut machine.cpu.nvic);
    assert_eq!(&machine.bus.sram.data[0x200..0x210], &[
        0x89, 0x60, 0x49, 0x4b, 0x90, 0x49, 0xfc, 0xea, 0xbf, 0x45, 0x67, 0x51, 0xca, 0xb7, 0xa2, 0x8e,
    ]);
    assert_eq!(machine.bus.read_u32(aes, &mut machine.periph, &machine.cpu.nvic), 0x6000_0000);
}
