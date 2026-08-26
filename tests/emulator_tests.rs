use tamagotchi_paradise_rs::emulator::cpu::registers::Registers;
use tamagotchi_paradise_rs::emulator::cpu::thumb16::{StepResult, Thumb16};
use tamagotchi_paradise_rs::emulator::cpu::thumb32::Thumb32;
use tamagotchi_paradise_rs::emulator::cpu::Nvic;
use tamagotchi_paradise_rs::emulator::mmu::MemoryBus;
use tamagotchi_paradise_rs::emulator::peripherals::Peripherals;
use tamagotchi_paradise_rs::emulator::Machine;

#[test]
fn test_emulator_initialization_and_reset() {
    let mut machine = Machine::new();
    assert!(machine.is_running);
    assert_eq!(machine.cpu.regs.msp, 0x2001_0000);
    assert_eq!(machine.cpu.regs.pc, 0x0800_0020);
}

#[test]
fn test_thumb16_mov_and_add() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // MOVS r0, #42 (0x202A)
    let res1 = Thumb16::execute(0x202A, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert!(matches!(res1, StepResult::Ok(_)));
    assert_eq!(regs.get_reg(0), 42);
    assert!(!regs.flag_z());
    assert!(!regs.flag_n());

    // ADDS r0, #8 (0x3008)
    let res2 = Thumb16::execute(0x3008, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert!(matches!(res2, StepResult::Ok(_)));
    assert_eq!(regs.get_reg(0), 50);
}

#[test]
fn test_thumb32_movw_movt() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // MOVW r1, #0x4100 (w1 = 0xF244, w2 = 0x1100) -> 0x4100
    // MOVT r1, #0x4500 (w1 = 0xF2C4, w2 = 0x5100) -> 0x45004100
    let res1 = Thumb32::execute(0xF244, 0x1100, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert!(matches!(res1, StepResult::Ok(_)));
    assert_eq!(regs.get_reg(1), 0x0000_4100);

    let res2 = Thumb32::execute(0xF2C4, 0x5100, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert!(matches!(res2, StepResult::Ok(_)));
    assert_eq!(regs.get_reg(1), 0x4500_4100);
}

#[test]
fn test_sonix_sys0_osc_ctrl_hide_bit() {
    let mut machine = Machine::new();
    assert!(!machine.bus.boot_rom.is_hidden);

    // Write OSC_CTRL |= 0x08 (hide ROM)
    machine.bus.write_u32(0x4500_0000, 0x08, &mut machine.periph, &mut machine.cpu.nvic);
    assert!(machine.bus.boot_rom.is_hidden);
}

#[test]
fn test_uart_console_capture() {
    let mut machine = Machine::new();

    // Write characters to UART data register (0x41000000)
    machine.bus.write_u32(0x4100_0000, b'H' as u32, &mut machine.periph, &mut machine.cpu.nvic);
    machine.bus.write_u32(0x4100_0000, b'I' as u32, &mut machine.periph, &mut machine.cpu.nvic);
    machine.bus.write_u32(0x4100_0000, b'!' as u32, &mut machine.periph, &mut machine.cpu.nvic);

    assert_eq!(machine.periph.uart.console_history, "HI!");
}

#[test]
fn test_gpio_buttons_and_dial() {
    let mut machine = Machine::new();

    // Initial state (all pull-up 1)
    let initial_gpio = machine.bus.read_u32(0x4400_0000, &mut machine.periph, &machine.cpu.nvic);
    assert_eq!(initial_gpio, 0xFFFF_FFFF);

    // Press Button A (bit 0 goes low)
    machine.periph.gpio.set_button_a(true);
    let pressed_gpio = machine.bus.read_u32(0x4400_0000, &mut machine.periph, &machine.cpu.nvic);
    assert_eq!(pressed_gpio & 1, 0);

    // Turn dial
    machine.periph.gpio.step_dial(3);
    let dial_val = machine.bus.read_u32(0x4400_0004, &mut machine.periph, &machine.cpu.nvic);
    assert_eq!(dial_val, 3);
}

#[test]
fn test_firmware_execution_step() {
    let mut machine = Machine::new();
    machine.reset();

    let initial_pc = machine.cpu.regs.pc;
    let step_res = machine.step();
    assert!(matches!(step_res, StepResult::Ok(_)));
    assert_ne!(machine.cpu.regs.pc, initial_pc);
}
