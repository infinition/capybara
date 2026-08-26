use eframe::egui;
use egui::{CentralPanel, Context, Key, TopBottomPanel};

use crate::audio::{AudioEngine, SoundEffect};
use crate::core::minigames::{BerryCatchState, ParadiseWheelState};
use crate::core::pet::PetActionFeedback;
use crate::core::{BiomeType, ParadiseIsland, Pet, SaveManager, SaveState};
use crate::gui::widgets::ActionButtonAction;
use crate::gui::{
    ActiveModal, GuiWidgets, ShellColor, VirtualScreen, VirtualShell, ZoomLevel,
};
use crate::hw_bridge::FlashInspector;
use crate::i18n::I18n;

pub struct TamagotchiApp {
    pub pet: Pet,
    pub island: ParadiseIsland,
    pub screen: VirtualScreen,
    pub shell: VirtualShell,
    pub audio: AudioEngine,
    pub i18n: I18n,
    pub flash_inspector: FlashInspector,
    pub active_modal: ActiveModal,
    pub berry_game: Option<BerryCatchState>,
    pub wheel_game: Option<ParadiseWheelState>,
    pub code_input: String,
    pub status_feedback: Option<String>,
    pub save_timer: f32,
    pub last_frame_time: std::time::Instant,
    pub shell_color_index: usize,
    pub always_on_top: bool,
    pub zoom_accumulator: f32,
}

impl TamagotchiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let loaded = SaveManager::load();
        let mut audio = AudioEngine::new();
        audio.volume = loaded.sound_volume;

        let shell_color = match loaded.shell_color_index {
            1 => ShellColor::JungleGreen,
            2 => ShellColor::SunsetPink,
            3 => ShellColor::CyberGrey,
            _ => ShellColor::OceanBlue,
        };

        let mut shell = VirtualShell::default();
        shell.color_theme = shell_color;

        let i18n = I18n::new(loaded.language);

        Self {
            pet: loaded.pet,
            island: loaded.island,
            screen: VirtualScreen::default(),
            shell,
            audio,
            i18n,
            flash_inspector: FlashInspector::default(),
            active_modal: ActiveModal::None,
            berry_game: None,
            wheel_game: None,
            code_input: String::new(),
            status_feedback: None,
            save_timer: 0.0,
            last_frame_time: std::time::Instant::now(),
            shell_color_index: loaded.shell_color_index,
            always_on_top: loaded.always_on_top,
            zoom_accumulator: 1.0, // starts at Normal
        }
    }

    fn persist_state(&self) {
        let state = SaveState {
            pet: self.pet.clone(),
            island: self.island.clone(),
            language: self.i18n.language(),
            sound_volume: self.audio.volume,
            shell_color_index: self.shell_color_index,
            always_on_top: self.always_on_top,
            window_scale: 1.0,
        };
        SaveManager::save(&state);
    }
}

impl eframe::App for TamagotchiApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let now = std::time::Instant::now();
        let dt = (now - self.last_frame_time).as_secs_f32().min(0.1);
        self.last_frame_time = now;

        // Auto request repaint for 60 FPS animations
        ctx.request_repaint();

        // 1. Tick simulation & screen
        let events = self.pet.tick(dt, &mut self.island);
        for event in events {
            match event.as_str() {
                "egg_hatched" | "evolved" | "birthday" => {
                    self.audio.play(SoundEffect::Happy);
                    self.screen.show_message(self.i18n.t("dialog_heal_success"));
                }
                "hungry_alert" | "hygiene_alert" => {
                    self.audio.play(SoundEffect::Alert);
                }
                _ => {}
            }
        }

        self.screen.update(dt);

        // Update active minigames
        if let Some(game) = &mut self.berry_game {
            if let Some(won) = game.update(dt) {
                if won {
                    self.pet.happiness = 4;
                    self.pet.coins += 25;
                    self.audio.play(SoundEffect::Happy);
                    self.screen.show_message(self.i18n.t("game_win"));
                } else {
                    self.screen.show_message(self.i18n.t("game_lose"));
                }
                self.berry_game = None;
            }
        }

        if let Some(game) = &mut self.wheel_game {
            game.update(dt);
        }

        // Auto save timer
        self.save_timer += dt;
        if self.save_timer >= 10.0 {
            self.save_timer = 0.0;
            self.persist_state();
        }

        // Sync shell color
        self.shell.color_theme = match self.shell_color_index {
            1 => ShellColor::JungleGreen,
            2 => ShellColor::SunsetPink,
            3 => ShellColor::CyberGrey,
            _ => ShellColor::OceanBlue,
        };

        // 2. Keyboard Inputs
        let key_a = ctx.input(|i| i.key_pressed(Key::A) || i.key_pressed(Key::ArrowLeft));
        let key_b = ctx.input(|i| {
            i.key_pressed(Key::B) || i.key_pressed(Key::Space) || i.key_pressed(Key::Enter)
        });
        let key_c = ctx.input(|i| {
            i.key_pressed(Key::C) || i.key_pressed(Key::Escape) || i.key_pressed(Key::ArrowRight)
        });

        // 3. UI Layout
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            GuiWidgets::render_hud(ui, &self.i18n, &self.pet);
            ui.add_space(4.0);
        });

        let mut triggered_action = ActionButtonAction::None;
        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.add_space(6.0);
            triggered_action =
                GuiWidgets::render_action_buttons(ui, &self.i18n, &mut self.active_modal);
            ui.add_space(6.0);
        });

        // Process bottom bar actions
        match triggered_action {
            ActionButtonAction::Clean => {
                let res = self.pet.clean_room();
                match res {
                    PetActionFeedback::Success => {
                        self.audio.play(SoundEffect::ButtonClick);
                        self.screen
                            .show_message(self.i18n.t("dialog_clean_success"));
                    }
                    PetActionFeedback::AlreadyClean => {
                        self.screen.show_message(self.i18n.t("dialog_clean_none"));
                    }
                    _ => {}
                }
            }
            ActionButtonAction::Heal => {
                let res = self.pet.heal();
                match res {
                    PetActionFeedback::Success => {
                        self.audio.play(SoundEffect::Cure);
                        self.screen
                            .show_message(self.i18n.t("dialog_heal_success"));
                    }
                    PetActionFeedback::AlreadyHealthy => {
                        self.screen.show_message(self.i18n.t("dialog_heal_none"));
                    }
                    _ => {}
                }
            }
            ActionButtonAction::Discipline => {
                let res = self.pet.train_discipline();
                if matches!(res, PetActionFeedback::Success) {
                    self.audio.play(SoundEffect::Happy);
                    self.screen
                        .show_message(self.i18n.t("dialog_discipline_success"));
                }
            }
            ActionButtonAction::PlayBerry => {
                self.berry_game = Some(BerryCatchState::new());
                self.audio.play(SoundEffect::ButtonClick);
            }
            ActionButtonAction::PlayWheel => {
                self.wheel_game = Some(ParadiseWheelState::new());
                self.audio.play(SoundEffect::ButtonClick);
            }
            ActionButtonAction::None => {}
        }

        CentralPanel::default().show(ctx, |ui| {
            let available_rect = ui.available_rect_before_wrap();
            let (screen_rect, controls) = self.shell.render(ui, available_rect);

            // Handle Dial rotation for Zoom
            let dial_movement = controls.dial_delta;
            if dial_movement.abs() > 0.1 {
                self.zoom_accumulator += dial_movement * 0.05;
                self.zoom_accumulator = self.zoom_accumulator.clamp(0.0, 2.0);
                let new_zoom_idx = self.zoom_accumulator.round() as usize;
                let new_zoom = ZoomLevel::from_index(new_zoom_idx);
                if new_zoom != self.screen.zoom {
                    self.screen.zoom = new_zoom;
                    self.audio.play(SoundEffect::DialTick);
                    self.screen.show_message(self.i18n.t(new_zoom.title_key()));
                }
            }

            // Handle Buttons A, B, C (from GUI clicks or Keyboard)
            let btn_a = controls.btn_a_clicked || key_a;
            let btn_b = controls.btn_b_clicked || key_b;
            let btn_c = controls.btn_c_clicked || key_c;

            if btn_a {
                self.audio.play(SoundEffect::ButtonClick);
                if let Some(game) = &mut self.berry_game {
                    game.move_left(0.1);
                } else {
                    match self.screen.zoom {
                        ZoomLevel::Micro => {
                            self.island.cleanse_micro_cells();
                            self.screen
                                .show_message("Micro-cellules purifiées.".to_string());
                        }
                        ZoomLevel::Paradise => {
                            let (watered, level) = self.island.water_plants();
                            if watered {
                                self.screen
                                    .show_message(format!("Plantes arrosées (Niveau {}).", level));
                            } else {
                                self.pet.coins += 10;
                                self.screen.show_message(format!(
                                    "Fruit récolté (+10 G) ! Total: {}.",
                                    level
                                ));
                            }
                        }
                        ZoomLevel::Normal => {
                            self.pet.happiness = (self.pet.happiness + 1).min(4);
                            self.screen.show_message("Caresses données.".to_string());
                        }
                    }
                }
            }

            if btn_b {
                self.audio.play(SoundEffect::ButtonClick);
                if let Some(game) = &mut self.wheel_game {
                    let won = game.stop();
                    if won {
                        self.pet.happiness = 4;
                        self.pet.coins += 40;
                        self.audio.play(SoundEffect::Happy);
                        self.screen.show_message(self.i18n.t("game_win"));
                    } else {
                        self.screen.show_message(self.i18n.t("game_lose"));
                    }
                    self.wheel_game = None;
                } else if self.berry_game.is_none() {
                    match self.screen.zoom {
                        ZoomLevel::Paradise => {
                            // Cycle unlocked biomes
                            let next_biome = match self.island.active_biome {
                                BiomeType::Garden => {
                                    if self.island.ocean_unlocked {
                                        BiomeType::Ocean
                                    } else if self.island.sky_unlocked {
                                        BiomeType::Sky
                                    } else {
                                        BiomeType::Garden
                                    }
                                }
                                BiomeType::Ocean => {
                                    if self.island.sky_unlocked {
                                        BiomeType::Sky
                                    } else {
                                        BiomeType::Garden
                                    }
                                }
                                BiomeType::Sky => BiomeType::Garden,
                            };
                            self.island.active_biome = next_biome;
                            self.screen
                                .show_message(self.i18n.t(next_biome.title_key()));
                        }
                        _ => {
                            // Wake up or toggle sleep
                            self.pet.toggle_sleep();
                            let msg = if self.pet.is_sleeping {
                                self.i18n.t("dialog_sleeping")
                            } else {
                                "Réveil en pleine forme.".to_string()
                            };
                            self.screen.show_message(msg);
                        }
                    }
                }
            }

            if btn_c {
                self.audio.play(SoundEffect::ButtonClick);
                if let Some(game) = &mut self.berry_game {
                    game.move_right(0.1);
                } else if self.wheel_game.is_some() {
                    self.wheel_game = None;
                } else {
                    let _ = self.pet.clean_room();
                    self.screen
                        .show_message(self.i18n.t("dialog_clean_success"));
                }
            }

            // Render virtual LCD screen
            let painter = ui.painter();
            self.screen.render(
                painter,
                screen_rect,
                &self.pet,
                &self.island,
                self.berry_game.as_ref(),
                self.wheel_game.as_ref(),
            );
        });

        // 4. Modals
        let mut food_eaten = None;
        GuiWidgets::render_feed_modal(
            ctx,
            &self.i18n,
            &mut self.active_modal,
            &mut self.pet,
            |item_name| {
                food_eaten = Some(item_name.to_string());
            },
        );
        if food_eaten.is_some() {
            self.audio.play(SoundEffect::Eat);
            self.screen
                .show_message(self.i18n.t("dialog_feed_success"));
        }

        GuiWidgets::render_shop_modal(
            ctx,
            &self.i18n,
            &mut self.active_modal,
            &mut self.pet,
            &mut self.island,
        );

        GuiWidgets::render_secret_code_modal(
            ctx,
            &self.i18n,
            &mut self.active_modal,
            &mut self.code_input,
            &mut self.pet,
            &mut self.island,
            &mut self.status_feedback,
        );

        GuiWidgets::render_flash_inspector_modal(
            ctx,
            &self.i18n,
            &mut self.active_modal,
            &self.flash_inspector,
        );

        GuiWidgets::render_settings_modal(
            ctx,
            &mut self.i18n,
            &mut self.active_modal,
            &mut self.audio.volume,
            &mut self.shell_color_index,
            &mut self.always_on_top,
        );
    }
}
