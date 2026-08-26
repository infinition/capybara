# Tamagotchi Paradise Hardware Emulator & Virtual Pet (Rust)

A high-performance ARM Cortex-M3 (Sonix SNC73410) hardware emulator and virtual pet simulator for Tamagotchi Paradise on Windows, built with Rust, eframe, and egui.

## Features

### 1. Low-Level Hardware Emulation (Sonix SNC73410)
- **ARM Cortex-M3 (ARMv7-M) Core**:
  - Full support for Thumb-16 and Thumb-2 (32-bit) instruction sets.
  - Hardware registers (R0-R12, MSP, PSP, LR, PC, xPSR, PRIMASK).
  - Exception handler and NVIC interrupt controller with SysTick timer.
- **Memory Bus Architecture**:
  - 16 MB Macronix KH25L12833F SPI NOR Flash (XIP mapped).
  - 128 KB internal SRAM / PRAM + 16 KB Mailbox RAM.
  - 64 KB Boot ROM with `OSC_CTRL` protection bit logic.
- **Peripherals**:
  - Sonix SYS0 system control registers.
  - Hardware LCD controller with 128x128 RGB565 VRAM framebuffer.
  - GPIO controller for physical Buttons A, B, C and rotary dial encoder.
  - UART serial port with bidirectional FIFO and real-time console logging.
  - Timers and Watchdog Timer (WDT).
- **Firmware Loading**: Load any raw `flash.bin`, `bootrom.bin`, or custom firmware dumps.

### 2. Interactive GUI & Live Debugger
- **Emulated LCD Display**: Real-time pixel output drawn directly from emulated hardware VRAM inside a virtual Tamagotchi shell.
- **Live Disassembler**: Real-time instruction stream around PC, Step Into (F10), Step Over, Run, Pause, and Reset.
- **CPU Register Inspector**: Live view of all 16 registers and APSR condition flags (N, Z, C, V).
- **Hex Memory Viewer**: Real-time hex inspection across Flash, SRAM, BootROM, and MMIO address ranges.
- **UART Serial Terminal**: Live text output emitted by the running firmware.

### 3. Simulation & Companion Tools
- Includes full Paradise virtual pet simulation engine, mini-games, secret code validation, and sound synthesis.

## Quick Start

### Build & Run

```bash
cargo run --release
```

### Run Tests

```bash
cargo test
```

### Keyboard Shortcuts

| Key | Action |
| --- | --- |
| `A` / `Left Arrow` | Button A (Select / Action) |
| `B` / `Space` / `Enter` | Button B (Confirm / Action) |
| `C` / `Escape` / `Right Arrow` | Button C (Cancel / Back) |
| `Mouse Wheel` | Turn Side Rotary Dial |
| `F10` | Single Step Instruction (Debugger) |
