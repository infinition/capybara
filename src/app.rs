use eframe::egui;
use egui::{CentralPanel, Context, Key, SidePanel, TopBottomPanel};

use crate::audio::{AudioEngine, SoundEffect};
use crate::emulator::Machine;
use crate::gui::ShellColor;
use crate::i18n::{I18n, Language};
use crate::ui::{ConsolePanel, CpuPanel, DisasmPanel, LcdPanel, MemoryPanel};

pub struct TamagotchiApp {
    pub machine: Machine,
    pub audio: AudioEngine,
    pub i18n: I18n,
    pub shell_color: ShellColor,
    pub show_debugger: bool,
    pub hex_base_addr: u32,
    pub last_frame_time: std::time::Instant,
    pub load_path_input: String,
    pub status_msg: Option<String>,
}

impl TamagotchiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let audio = AudioEngine::new();
        let i18n = I18n::default();
        let machine = Machine::new();

        Self {
            machine,
            audio,
            i18n,
            shell_color: ShellColor::OceanBlue,
            show_debugger: true,
            hex_base_addr: 0x0800_0000,
            last_frame_time: std::time::Instant::now(),
            load_path_input: String::new(),
            status_msg: Some("Tamagotchi Paradise Hardware Emulation Ready.".to_string()),
        }
    }
}

impl eframe::App for TamagotchiApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let now = std::time::Instant::now();
        let _dt = (now - self.last_frame_time).as_secs_f32().min(0.1);
        self.last_frame_time = now;

        // Auto repaint for 60 FPS emulator loop
        ctx.request_repaint();

        // 1. Run CPU execution frame
        if self.machine.is_running {
            self.machine.run_frame();
        }

        // 2. Keyboard Inputs
        let key_a = ctx.input(|i| i.key_down(Key::A) || i.key_down(Key::ArrowLeft));
        let key_b = ctx.input(|i| i.key_down(Key::B) || i.key_down(Key::Space) || i.key_down(Key::Enter));
        let key_c = ctx.input(|i| i.key_down(Key::C) || i.key_down(Key::Escape) || i.key_down(Key::ArrowRight));
        let key_f10 = ctx.input(|i| i.key_pressed(Key::F10));

        if key_f10 {
            self.machine.is_running = false;
            self.machine.step();
        }

        // Support Drag and Drop of firmware files onto the emulator
        ctx.input(|i| {
            if let Some(dropped) = i.raw.dropped_files.first() {
                if let Some(path) = &dropped.path {
                    self.load_path_input = path.to_string_lossy().to_string();
                    match self.machine.load_firmware_file(path) {
                        Ok(bytes) => {
                            self.status_msg = Some(format!("Loaded {} bytes from dropped file.", bytes));
                        }
                        Err(e) => {
                            self.status_msg = Some(format!("Load error: {}", e));
                        }
                    }
                }
            }
        });

        // 3. Top Status & Menu Bar
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tamagotchi Paradise SNC73410 Emulator").strong());

                ui.separator();

                if ui.button(if self.show_debugger { "Hide Debugger" } else { "Show Debugger" }).clicked() {
                    self.show_debugger = !self.show_debugger;
                }

                ui.separator();

                ui.label("Shell Color:");
                if ui.selectable_label(self.shell_color == ShellColor::OceanBlue, "Ocean").clicked() {
                    self.shell_color = ShellColor::OceanBlue;
                }
                if ui.selectable_label(self.shell_color == ShellColor::JungleGreen, "Jungle").clicked() {
                    self.shell_color = ShellColor::JungleGreen;
                }
                if ui.selectable_label(self.shell_color == ShellColor::SunsetPink, "Sunset").clicked() {
                    self.shell_color = ShellColor::SunsetPink;
                }
                if ui.selectable_label(self.shell_color == ShellColor::CyberGrey, "Cyber").clicked() {
                    self.shell_color = ShellColor::CyberGrey;
                }

                ui.separator();

                ui.label("Language:");
                if ui.selectable_label(self.i18n.language() == Language::Fr, "FR").clicked() {
                    self.i18n.set_language(Language::Fr);
                }
                if ui.selectable_label(self.i18n.language() == Language::En, "EN").clicked() {
                    self.i18n.set_language(Language::En);
                }
            });
        });

        // 4. Debugger Side Panel
        if self.show_debugger {
            SidePanel::right("debug_panel")
                .min_width(420.0)
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.add_space(4.0);

                    // Firmware File Loader Box
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Firmware / Flash Dump (.bin):").strong());
                        ui.horizontal(|ui| {
                            if ui.button(egui::RichText::new("📂 Parcourir / Browse...").strong()).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Firmware Binary (*.bin, *.rom, *.hex, *.elf)", &["bin", "rom", "hex", "elf", "raw", "dump"])
                                    .set_title("Sélectionner un dump de firmware Tamagotchi")
                                    .pick_file()
                                {
                                    self.load_path_input = path.to_string_lossy().to_string();
                                    match self.machine.load_firmware_file(&path) {
                                        Ok(bytes) => {
                                            self.status_msg = Some(format!("Loaded {} bytes into Flash.", bytes));
                                        }
                                        Err(e) => {
                                            self.status_msg = Some(format!("Load error: {}", e));
                                        }
                                    }
                                }
                            }

                            ui.text_edit_singleline(&mut self.load_path_input);
                            if ui.button("Load").clicked() && !self.load_path_input.is_empty() {
                                match self.machine.load_firmware_file(&self.load_path_input) {
                                    Ok(bytes) => {
                                        self.status_msg = Some(format!("Loaded {} bytes into Flash.", bytes));
                                    }
                                    Err(e) => {
                                        self.status_msg = Some(format!("Load error: {}", e));
                                    }
                                }
                            }
                        });
                        if let Some(msg) = &self.status_msg {
                            ui.label(egui::RichText::new(msg).small().color(egui::Color32::from_rgb(255, 230, 80)));
                        }
                    });

                    ui.separator();

                    // CPU Registers Inspector
                    CpuPanel::render(
                        ui,
                        &self.machine.cpu.regs,
                        self.machine.cpu.cycles,
                        self.machine.is_running,
                        &self.i18n,
                    );

                    ui.separator();

                    // Disassembly Stream
                    let instructions = self.machine.get_disassembly_window(10);
                    let current_pc = self.machine.cpu.regs.pc;
                    let is_running_ref = &mut self.machine.is_running;

                    let mut step_requested = false;
                    let mut reset_requested = false;
                    DisasmPanel::render(
                        ui,
                        &instructions,
                        current_pc,
                        is_running_ref,
                        || {
                            step_requested = true;
                        },
                        || {
                            reset_requested = true;
                        },
                    );

                    if step_requested {
                        self.machine.step();
                    }
                    if reset_requested {
                        self.machine.reset();
                    }

                    ui.separator();

                    // Hex Memory Viewer
                    MemoryPanel::render(
                        ui,
                        &mut self.machine.bus,
                        &mut self.machine.periph,
                        &self.machine.cpu.nvic,
                        &mut self.hex_base_addr,
                    );

                    ui.separator();

                    // UART Console
                    ConsolePanel::render(ui, &mut self.machine.periph.uart);
                });
        }

        // 5. Central Virtual Device Display & Controls
        CentralPanel::default().show(ctx, |ui| {
            let available_rect = ui.available_rect_before_wrap();

            let mut btn_a_pressed = key_a;
            let mut btn_b_pressed = key_b;
            let mut btn_c_pressed = key_c;
            let mut dial_delta = 0;

            LcdPanel::render(
                ui,
                available_rect,
                &self.machine.periph.display,
                self.shell_color,
                |p| {
                    if p {
                        btn_a_pressed = true;
                    }
                },
                |p| {
                    if p {
                        btn_b_pressed = true;
                    }
                },
                |p| {
                    if p {
                        btn_c_pressed = true;
                    }
                },
                |d| {
                    dial_delta = d;
                },
            );

            // Inject controls into GPIO peripheral
            self.machine.periph.gpio.set_button_a(btn_a_pressed);
            self.machine.periph.gpio.set_button_b(btn_b_pressed);
            self.machine.periph.gpio.set_button_c(btn_c_pressed);

            if dial_delta != 0 {
                self.machine.periph.gpio.step_dial(dial_delta);
                self.audio.play(SoundEffect::DialTick);
            }
        });
    }
}
