use egui::{Color32, ProgressBar, RichText, ScrollArea, Slider, Ui, Window};

use crate::core::items::get_default_catalog;
use crate::core::pet::PetActionFeedback;
use crate::core::{ParadiseIsland, Pet, SecretCodeManager, SecretReward};
use crate::hw_bridge::FlashInspector;
use crate::i18n::{I18n, Language};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    FeedMenu,
    Shop,
    SecretCode,
    Settings,
    FlashInspector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionButtonAction {
    None,
    Clean,
    Heal,
    Discipline,
    PlayBerry,
    PlayWheel,
}

pub struct GuiWidgets;

impl GuiWidgets {
    pub fn render_hud(ui: &mut Ui, i18n: &I18n, pet: &Pet) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{}: {}",
                    pet.species.name(),
                    i18n.t(&format!(
                        "stage_{}",
                        format!("{:?}", pet.stage).to_lowercase()
                    ))
                ))
                .strong(),
            );
            ui.label(
                RichText::new(format!("💰 {} G", pet.coins))
                    .color(Color32::from_rgb(255, 200, 50)),
            );
        });

        ui.add_space(4.0);

        // Stats grid
        egui::Grid::new("stats_grid")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                // Hunger
                ui.label(i18n.t("status_hunger"));
                ui.horizontal(|ui| {
                    for i in 0..4 {
                        if i < pet.hunger {
                            ui.colored_label(Color32::from_rgb(255, 80, 80), "🍗");
                        } else {
                            ui.colored_label(Color32::from_rgb(100, 100, 100), "⚪");
                        }
                    }
                });
                ui.end_row();

                // Happiness
                ui.label(i18n.t("status_happiness"));
                ui.horizontal(|ui| {
                    for i in 0..4 {
                        if i < pet.happiness {
                            ui.colored_label(Color32::from_rgb(255, 215, 0), "⭐");
                        } else {
                            ui.colored_label(Color32::from_rgb(100, 100, 100), "⚪");
                        }
                    }
                });
                ui.end_row();

                // Energy
                ui.label(i18n.t("status_energy"));
                ui.add(
                    ProgressBar::new(pet.energy / 100.0)
                        .text(format!("{:.0}%", pet.energy))
                        .fill(Color32::from_rgb(80, 180, 240)),
                );
                ui.end_row();

                // Hygiene
                ui.label(i18n.t("status_hygiene"));
                ui.add(
                    ProgressBar::new(pet.hygiene / 100.0)
                        .text(format!("{:.0}%", pet.hygiene))
                        .fill(Color32::from_rgb(120, 220, 120)),
                );
                ui.end_row();

                // Discipline
                ui.label(i18n.t("status_discipline"));
                ui.add(
                    ProgressBar::new(pet.discipline as f32 / 100.0)
                        .text(format!("{}%", pet.discipline))
                        .fill(Color32::from_rgb(200, 140, 240)),
                );
                ui.end_row();

                // Age & Weight
                ui.label(i18n.t("status_age"));
                ui.label(i18n.t_args(
                    "status_days",
                    &[("count", &pet.age_days.to_string())],
                ));
                ui.end_row();

                ui.label(i18n.t("status_weight"));
                ui.label(i18n.t_args(
                    "status_grams",
                    &[("count", &pet.weight_g.to_string())],
                ));
                ui.end_row();
            });
    }

    pub fn render_action_buttons(
        ui: &mut Ui,
        i18n: &I18n,
        active_modal: &mut ActiveModal,
    ) -> ActionButtonAction {
        let mut action = ActionButtonAction::None;
        ui.horizontal_wrapped(|ui| {
            if ui.button(format!("🍲 {}", i18n.t("btn_feed"))).clicked() {
                *active_modal = ActiveModal::FeedMenu;
            }
            if ui.button(format!("🧹 {}", i18n.t("btn_clean"))).clicked() {
                action = ActionButtonAction::Clean;
            }
            if ui.button(format!("💊 {}", i18n.t("btn_heal"))).clicked() {
                action = ActionButtonAction::Heal;
            }
            if ui.button(format!("🎓 {}", i18n.t("btn_discipline"))).clicked() {
                action = ActionButtonAction::Discipline;
            }
            if ui.button(format!("🍓 {}", i18n.t("game_berry_title"))).clicked() {
                action = ActionButtonAction::PlayBerry;
            }
            if ui.button(format!("🎡 {}", i18n.t("game_wheel_title"))).clicked() {
                action = ActionButtonAction::PlayWheel;
            }
            if ui.button(format!("🛒 {}", i18n.t("btn_shop"))).clicked() {
                *active_modal = ActiveModal::Shop;
            }
            if ui.button(format!("🎁 {}", i18n.t("btn_secret"))).clicked() {
                *active_modal = ActiveModal::SecretCode;
            }
            if ui.button(format!("⚙ {}", i18n.t("btn_settings"))).clicked() {
                *active_modal = ActiveModal::Settings;
            }
            if ui.button(format!("💾 {}", i18n.t("btn_hw_dump"))).clicked() {
                *active_modal = ActiveModal::FlashInspector;
            }
        });
        action
    }

    pub fn render_feed_modal(
        ctx: &egui::Context,
        i18n: &I18n,
        active_modal: &mut ActiveModal,
        pet: &mut Pet,
        mut on_feed: impl FnMut(&str),
    ) {
        if *active_modal != ActiveModal::FeedMenu {
            return;
        }

        let mut open = true;
        Window::new(i18n.t("btn_feed"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let catalog = get_default_catalog();
                for item in catalog {
                    ui.horizontal(|ui| {
                        ui.label(i18n.t(item.name_key));
                        ui.label(format!(
                            "(+{} faim, +{} joie)",
                            item.hunger_restore, item.happiness_restore
                        ));
                        if ui.button(format!("{} G", item.price)).clicked() {
                            if pet.coins >= item.price {
                                let feedback = pet.feed(&item);
                                if matches!(feedback, PetActionFeedback::Success) {
                                    pet.coins -= item.price;
                                    on_feed(item.name_key);
                                }
                            }
                        }
                    });
                }
            });

        if !open {
            *active_modal = ActiveModal::None;
        }
    }

    pub fn render_shop_modal(
        ctx: &egui::Context,
        i18n: &I18n,
        active_modal: &mut ActiveModal,
        pet: &mut Pet,
        island: &mut ParadiseIsland,
    ) {
        if *active_modal != ActiveModal::Shop {
            return;
        }

        let mut open = true;
        Window::new(i18n.t("shop_title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(i18n.t_args("shop_coins", &[("coins", &pet.coins.to_string())]));
                ui.separator();

                // Ocean key
                ui.horizontal(|ui| {
                    ui.label(i18n.t("biome_ocean"));
                    if island.ocean_unlocked {
                        ui.colored_label(
                            Color32::GREEN,
                            i18n.choisir("✔ Debloque", "✔ Unlocked"),
                        );
                    } else if ui.button(i18n.t_args("shop_buy", &[("price", "80")])).clicked() {
                        if pet.coins >= 80 {
                            pet.coins -= 80;
                            island.ocean_unlocked = true;
                        }
                    }
                });

                // Sky key
                ui.horizontal(|ui| {
                    ui.label(i18n.t("biome_sky"));
                    if island.sky_unlocked {
                        ui.colored_label(
                            Color32::GREEN,
                            i18n.choisir("✔ Debloque", "✔ Unlocked"),
                        );
                    } else if ui.button(i18n.t_args("shop_buy", &[("price", "150")])).clicked() {
                        if pet.coins >= 150 {
                            pet.coins -= 150;
                            island.sky_unlocked = true;
                        }
                    }
                });
            });

        if !open {
            *active_modal = ActiveModal::None;
        }
    }

    pub fn render_secret_code_modal(
        ctx: &egui::Context,
        i18n: &I18n,
        active_modal: &mut ActiveModal,
        code_input: &mut String,
        pet: &mut Pet,
        island: &mut ParadiseIsland,
        status_feedback: &mut Option<String>,
    ) {
        if *active_modal != ActiveModal::SecretCode {
            return;
        }

        let mut open = true;
        Window::new(i18n.t("secret_title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.text_edit_singleline(code_input);
                ui.add_space(4.0);
                if ui.button(i18n.t("secret_submit")).clicked() {
                    if let Some(reward) = SecretCodeManager::redeem(code_input) {
                        match reward {
                            SecretReward::Coins(c) => {
                                pet.coins += c;
                                *status_feedback = Some(i18n.t_args(
                                    "secret_success",
                                    &[("reward", &format!("+{} Gotchi-Coins", c))],
                                ));
                            }
                            SecretReward::UnlockOcean => {
                                island.ocean_unlocked = true;
                                *status_feedback = Some(i18n.t_args(
                                    "secret_success",
                                    &[("reward", "Biome Océan")],
                                ));
                            }
                            SecretReward::UnlockSky => {
                                island.sky_unlocked = true;
                                *status_feedback = Some(i18n.t_args(
                                    "secret_success",
                                    &[("reward", "Biome Ciel")],
                                ));
                            }
                            SecretReward::GoldenApples(n) => {
                                pet.hunger = (pet.hunger + n).min(4);
                                pet.happiness = 4;
                                *status_feedback = Some(i18n.t_args(
                                    "secret_success",
                                    &[("reward", "Pommes Dorées")],
                                ));
                            }
                            SecretReward::FullHeal => {
                                pet.is_sick = false;
                                pet.hygiene = 100.0;
                                *status_feedback = Some(i18n.t_args(
                                    "secret_success",
                                    &[("reward", "Soin Total")],
                                ));
                            }
                        }
                    } else {
                        *status_feedback = Some(i18n.t("secret_invalid"));
                    }
                    code_input.clear();
                }

                if let Some(feedback) = status_feedback {
                    ui.add_space(4.0);
                    ui.colored_label(Color32::from_rgb(255, 230, 80), feedback.as_str());
                }
            });

        if !open {
            *active_modal = ActiveModal::None;
        }
    }

    pub fn render_flash_inspector_modal(
        ctx: &egui::Context,
        i18n: &I18n,
        active_modal: &mut ActiveModal,
        inspector: &FlashInspector,
    ) {
        if *active_modal != ActiveModal::FlashInspector {
            return;
        }

        let mut open = true;
        Window::new(i18n.t("hw_title"))
            .open(&mut open)
            .collapsible(false)
            .default_width(480.0)
            .show(ctx, |ui| {
                ui.label(RichText::new(i18n.t("hw_desc")).italics());
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(i18n.choisir("Edition detectee :", "Detected edition:"));
                    ui.label(RichText::new(&inspector.detected_edition).color(Color32::from_rgb(100, 220, 255)).strong());
                });

                ui.horizontal(|ui| {
                    ui.label(i18n.choisir("Taille de la flash :", "Flash size:"));
                    ui.label(
                        RichText::new(format!(
                            "{} MB (128 Mbit)",
                            inspector.file_size / (1024 * 1024)
                        ))
                        .strong(),
                    );
                });

                ui.label(format!("Magic / Header: {}", inspector.header_magic));
                ui.label(format!(
                    "ARC2 Container: {} tables ({} KB / {} octets)",
                    inspector.arc2_assets_count,
                    inspector.arc2_total_bytes / 1024,
                    inspector.arc2_total_bytes
                ));
                ui.label(i18n.t("hw_uart_status"));

                ui.add_space(8.0);
                ui.label(
                    RichText::new(i18n.choisir(
                        "Organisation memoire et partitions :",
                        "Memory layout and partitions:",
                    ))
                    .strong(),
                );

                ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for section in &inspector.sections {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&section.name).strong());
                                ui.label(format!(
                                    "0x{:06X} - 0x{:06X} ({} KB)",
                                    section.offset_start,
                                    section.offset_end,
                                    section.size_bytes / 1024
                                ));
                                ui.colored_label(Color32::GREEN, &section.status);
                            });
                            ui.label(RichText::new(&section.description).small());
                        });
                    }
                });
            });

        if !open {
            *active_modal = ActiveModal::None;
        }
    }

    pub fn render_settings_modal(
        ctx: &egui::Context,
        i18n: &mut I18n,
        active_modal: &mut ActiveModal,
        sound_volume: &mut f32,
        shell_color_idx: &mut usize,
        always_on_top: &mut bool,
    ) {
        if *active_modal != ActiveModal::Settings {
            return;
        }

        let mut open = true;
        Window::new(i18n.t("settings_title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                // Language
                ui.horizontal(|ui| {
                    ui.label(i18n.t("settings_lang"));
                    if ui
                        .selectable_label(i18n.language() == Language::Fr, "Français")
                        .clicked()
                    {
                        i18n.set_language(Language::Fr);
                    }
                    if ui
                        .selectable_label(i18n.language() == Language::En, "English")
                        .clicked()
                    {
                        i18n.set_language(Language::En);
                    }
                });

                ui.add_space(8.0);

                // Sound Volume
                ui.horizontal(|ui| {
                    ui.label(i18n.t("settings_sound"));
                    ui.add(Slider::new(sound_volume, 0.0..=1.0).show_value(false));
                });

                ui.add_space(8.0);

                // Shell Color
                ui.label(i18n.t("settings_shell_color"));
                ui.horizontal(|ui| {
                    let colors = [
                        (0, "shell_ocean"),
                        (1, "shell_jungle"),
                        (2, "shell_sunset"),
                        (3, "shell_cyber"),
                    ];
                    for (idx, key) in colors {
                        if ui
                            .selectable_label(*shell_color_idx == idx, i18n.t(key))
                            .clicked()
                        {
                            *shell_color_idx = idx;
                        }
                    }
                });

                ui.add_space(8.0);

                // Always on top
                ui.checkbox(always_on_top, i18n.t("settings_always_on_top"));
            });

        if !open {
            *active_modal = ActiveModal::None;
        }
    }
}
