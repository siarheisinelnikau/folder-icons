//! Complete built-in UI catalogs; user data and technical diagnostics are never translated.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Ru,
    Fr,
    De,
    Es,
    It,
}
impl Language {
    pub const ALL: [Self; 6] = [Self::En, Self::Ru, Self::Fr, Self::De, Self::Es, Self::It];
    pub fn name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Ru => "Русский",
            Self::Fr => "Français",
            Self::De => "Deutsch",
            Self::Es => "Español",
            Self::It => "Italiano",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ru => "ru",
            Self::Fr => "fr",
            Self::De => "de",
            Self::Es => "es",
            Self::It => "it",
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.code() == code)
    }
    pub fn text(self, key: &'static str) -> &'static str {
        let index = match self {
            Self::En => return key,
            Self::Ru => 0,
            Self::Fr => 1,
            Self::De => 2,
            Self::Es => 3,
            Self::It => 4,
        };
        catalog()
            .get(key)
            .map_or(key, |translations| translations[index].as_str())
    }
    pub fn format(self, key: &'static str, values: &[(&str, String)]) -> String {
        let mut text = self.text(key).to_owned();
        for (name, value) in values {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }
}
fn catalog() -> &'static BTreeMap<String, [String; 5]> {
    static CATALOG: OnceLock<BTreeMap<String, [String; 5]>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("locales/messages.json"))
            .expect("Validated built-in UI catalog")
    })
}

/// Keep message identity until painting so existing status text changes language immediately.
#[derive(Clone, Debug)]
pub struct Message {
    key: &'static str,
    values: Vec<(&'static str, String)>,
    detail: Option<String>,
}
impl Message {
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            values: Vec::new(),
            detail: None,
        }
    }
    pub fn with(mut self, name: &'static str, value: impl ToString) -> Self {
        self.values.push((name, value.to_string()));
        self
    }
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
    pub fn render(&self, language: Language) -> String {
        let mut text = language.format(self.key, &self.values);
        if let Some(detail) = &self.detail {
            text.push_str(": ");
            text.push_str(detail);
        }
        text
    }
}
impl From<&'static str> for Message {
    fn from(key: &'static str) -> Self {
        Self::new(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn placeholders(text: &str) -> Vec<&str> {
        let mut names: Vec<_> = text
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(name, _)| name))
            .collect();
        names.sort_unstable();
        names
    }
    #[test]
    fn all_catalog_translations_are_complete_and_preserve_parameters() {
        assert!(catalog().len() >= 100);
        for (key, translations) in catalog() {
            for translation in translations {
                assert!(!translation.trim().is_empty(), "Empty translation: {key}");
                assert_eq!(
                    placeholders(key),
                    placeholders(translation),
                    "Changed parameters: {key}"
                );
            }
        }
    }
    #[test]
    fn message_switches_language_without_changing_user_data() {
        let path = "D:/Photo Version {version} 旅行";
        let message = Message::new("Applied · {path}").with("path", path);
        assert_eq!(message.render(Language::En), format!("Applied · {path}"));
        assert_eq!(message.render(Language::Ru), format!("Применено · {path}"));
        assert_eq!(
            Language::Fr.text("Future English fallback"),
            "Future English fallback"
        );
    }
}
