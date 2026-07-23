use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::i18n::Locale;
use crate::ui::theme::ThemeMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub download_dir: PathBuf,
    pub max_concurrent: u32,
    pub download_limit_kb: u64,
    pub upload_limit_kb: u64,
    pub split: u16,
    pub enable_dht: bool,
    pub bt_listen_port: u16,
    #[serde(default)]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub locale: Locale,
}

impl Default for Settings {
    fn default() -> Self {
        let download_dir = directories::UserDirs::new()
            .and_then(|d| d.download_dir().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            download_dir,
            max_concurrent: 5,
            download_limit_kb: 0,
            upload_limit_kb: 0,
            split: 16,
            enable_dht: true,
            bt_listen_port: 6881,
            theme_mode: ThemeMode::System,
            locale: Locale::default(),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
    let dir = proj.config_dir().to_path_buf();
    Some(dir.join("settings.json"))
}

fn settings_file_path() -> Option<PathBuf> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    Some(path)
}

pub fn load() -> Settings {
    let Some(path) = settings_file_path() else {
        return Settings::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) {
    let Some(path) = settings_file_path() else {
        return;
    };
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(&path, json);
    }
}

pub fn log_dir() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
    let dir = proj.data_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

pub fn announce() {
    if let Some(p) = config_path() {
        tracing::info!(?p, "config path");
    }
    if let Some(p) = log_dir() {
        tracing::info!(?p, "log dir");
    }
}
