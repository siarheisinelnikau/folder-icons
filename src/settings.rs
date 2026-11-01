use crate::{i18n::Language, platform};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub check_on_startup: bool,
    pub language: Language,
    pub theme: Theme,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            check_on_startup: true,
            language: Language::default(),
            theme: Theme::default(),
        }
    }
}
pub fn directory() -> Result<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .context("LOCALAPPDATA is unavailable; settings cannot be persisted")?;
    Ok(PathBuf::from(base).join("FolderIcons"))
}
pub fn load() -> Result<Settings> {
    let path = directory()?.join("settings.json");
    match platform::read_optional(&path, 64 * 1024)? {
        Some(bytes) => Ok(serde_json::from_slice(&bytes)?),
        None => Ok(Settings::default()),
    }
}
pub fn save(settings: &Settings) -> Result<()> {
    let dir = directory()?;
    std::fs::create_dir_all(&dir)?;
    platform::atomic_write(
        &dir.join("settings.json"),
        &serde_json::to_vec_pretty(settings)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_settings_keep_startup_choice_and_gain_safe_interface_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"check_on_startup":false}"#).unwrap();
        assert!(!settings.check_on_startup);
        assert_eq!(settings.language, Language::En);
        assert_eq!(settings.theme, Theme::Light);
        let settings = Settings {
            language: Language::It,
            theme: Theme::Dark,
            ..settings
        };
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert!(!restored.check_on_startup);
        assert_eq!(restored.language, Language::It);
        assert_eq!(restored.theme, Theme::Dark);
    }
}
