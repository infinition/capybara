#![allow(dead_code)]

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::PathBuf;

use super::garden::ParadiseIsland;
use super::pet::Pet;
use crate::i18n::Language;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveState {
    pub pet: Pet,
    pub island: ParadiseIsland,
    pub language: Language,
    pub sound_volume: f32,
    pub shell_color_index: usize,
    pub always_on_top: bool,
    pub window_scale: f32,
}

impl Default for SaveState {
    fn default() -> Self {
        Self {
            pet: Pet::default(),
            island: ParadiseIsland::default(),
            language: Language::Fr,
            sound_volume: 0.5,
            shell_color_index: 0,
            always_on_top: false,
            window_scale: 1.0,
        }
    }
}

pub struct SaveManager;

impl SaveManager {
    fn get_save_path() -> Option<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "infinition", "tamagotchi-paradise") {
            let config_dir = proj_dirs.config_dir();
            let _ = create_dir_all(config_dir);
            Some(config_dir.join("save.json"))
        } else {
            None
        }
    }

    pub fn load() -> SaveState {
        if let Some(path) = Self::get_save_path() {
            if path.exists() {
                if let Ok(mut file) = File::open(path) {
                    let mut content = String::new();
                    if file.read_to_string(&mut content).is_ok() {
                        if let Ok(state) = serde_json::from_str::<SaveState>(&content) {
                            return state;
                        }
                    }
                }
            }
        }
        SaveState::default()
    }

    pub fn save(state: &SaveState) -> bool {
        if let Some(path) = Self::get_save_path() {
            if let Ok(content) = serde_json::to_string_pretty(state) {
                if let Ok(mut file) = File::create(path) {
                    return file.write_all(content.as_bytes()).is_ok();
                }
            }
        }
        false
    }
}
