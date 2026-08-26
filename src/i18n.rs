#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    Fr,
    En,
}

impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Language::Fr => "fr",
            Language::En => "en",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::Fr => "Français",
            Language::En => "English",
        }
    }
}

pub struct I18n {
    current_lang: Language,
    fr_dict: HashMap<String, String>,
    en_dict: HashMap<String, String>,
}

impl Default for I18n {
    fn default() -> Self {
        Self::new(Language::Fr)
    }
}

impl I18n {
    pub fn new(lang: Language) -> Self {
        let fr_raw = include_str!("../i18n/fr.json");
        let en_raw = include_str!("../i18n/en.json");

        let fr_dict: HashMap<String, String> =
            serde_json::from_str(fr_raw).unwrap_or_default();
        let en_dict: HashMap<String, String> =
            serde_json::from_str(en_raw).unwrap_or_default();

        Self {
            current_lang: lang,
            fr_dict,
            en_dict,
        }
    }

    pub fn set_language(&mut self, lang: Language) {
        self.current_lang = lang;
    }

    pub fn language(&self) -> Language {
        self.current_lang
    }

    pub fn t(&self, key: &str) -> String {
        let dict = match self.current_lang {
            Language::Fr => &self.fr_dict,
            Language::En => &self.en_dict,
        };

        if let Some(val) = dict.get(key) {
            return val.clone();
        }

        // Fallback to English if not found
        if let Some(val) = self.en_dict.get(key) {
            return val.clone();
        }

        key.to_string()
    }

    pub fn t_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut template = self.t(key);
        for (k, v) in args {
            let placeholder = format!("{{{}}}", k);
            template = template.replace(&placeholder, v);
        }
        template
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i18n_translation() {
        let i18n = I18n::new(Language::Fr);
        assert_eq!(i18n.t("stage_baby"), "Bébé");
        assert_eq!(
            i18n.t_args("dialog_feed_success", &[("name", "Mametchi")]),
            "Mametchi a bien mangé."
        );
    }
}
