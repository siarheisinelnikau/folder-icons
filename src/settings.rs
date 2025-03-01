use crate::platform;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub check_on_startup: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            check_on_startup: true,
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
