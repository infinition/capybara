# Tamagotchi Paradise Desktop (Rust)

Desktop virtual pet simulator and firmware inspector for Tamagotchi Paradise, built in Rust for Windows with eframe and egui.

## Features

- **Full Life Cycle Simulation**: Egg, Baby, Child, Teen, Adult, and Senior evolution stages with care mistake tracking, discipline training, hunger, happiness, energy, and hygiene vitals.
- **Paradise Rotary Dial & 3-Level Zoom**:
  - **Micro Level**: Cell health inspection, microscopic parasite cleansing.
  - **Normal Level**: Pet habitat, feeding, pet interaction, poop cleaning, sleeping.
  - **Paradise Island Level**: Island biomes (Lush Garden, Turquoise Ocean, Starry Sky), fruit tree watering and harvesting.
- **Interactive Controls**:
  - Physical shell buttons A, B, C (Mouse click and keyboard shortcuts: A / Left Arrow, B / Space / Enter, C / Escape / Right Arrow).
  - Side rotary wheel with real rotation simulation (Drag or mouse wheel scroll).
- **Mini-Games**:
  - Berry Catcher: Fast-paced falling fruit catching game.
  - Paradise Wheel of Fortune: Precision timing stop game for Gotchi-Coins.
- **Chiptune Audio Synthesizer**: Procedural square/sine wave sound effects for button clicks, eating, dial ticks, alerts, cures, and victory jingles.
- **Persistent State**: Automatic real-time save system in user AppData.
- **Firmware & Hardware Inspector**:
  - Sonix SNC73410 flash layout parser for SPI NOR Macronix KH25L12833F (128 Mbit).
  - Virtual UART / P-COM protocol simulation.
- **Internationalization (i18n)**: English and French language support switchable dynamically.
- **Customizable Shell Themes**: Ocean Blue, Jungle Green, Sunset Pink, Cyber Grey.

## Building and Running

### Prerequisites

- Rust 1.80+ (MSVC toolchain on Windows)
- Cargo

### Compilation

```bash
cargo build --release
```

### Running

```bash
cargo run --release
```

## Running Tests

```bash
cargo test
```

## Keyboard Shortcuts

| Key | Action |
| --- | --- |
| `A` / `Left Arrow` | Button A (Move Left / Select / Water / Cuddle) |
| `B` / `Space` / `Enter` | Button B (Confirm / Action / Stop Wheel / Sleep / Biome) |
| `C` / `Escape` / `Right Arrow` | Button C (Cancel / Move Right / Clean Room) |
| `Mouse Wheel` / `Drag on Dial` | Turn Paradise Zoom Dial |
