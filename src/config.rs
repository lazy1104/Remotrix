use std::collections::HashMap;
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
    #[serde(default)]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub locale: Locale,
    #[serde(default)]
    pub update: UpdatePrefs,
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
            theme_mode: ThemeMode::System,
            locale: Locale::default(),
            update: UpdatePrefs::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdatePrefs {
    #[serde(default)]
    pub components: HashMap<String, ComponentUpdatePrefs>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComponentUpdatePrefs {
    #[serde(default)]
    pub ignored: bool,
    #[serde(default)]
    pub skipped: Vec<String>,
}

impl UpdatePrefs {
    pub fn should_auto_check(&self, component: &str) -> bool {
        self.components
            .get(component)
            .map(|c| !c.ignored)
            .unwrap_or(true)
    }

    pub fn is_skipped(&self, component: &str, version: &str) -> bool {
        self.components
            .get(component)
            .map(|c| c.skipped.contains(&version.to_string()))
            .unwrap_or(false)
    }

    pub fn skip_version(&mut self, component: &str, version: &str) {
        self.components
            .entry(component.to_string())
            .or_default()
            .skipped
            .push(version.to_string());
    }

    pub fn set_ignored(&mut self, component: &str, ignored: bool) {
        self.components
            .entry(component.to_string())
            .or_default()
            .ignored = ignored;
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

pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(ToOwned::to_owned)
}

fn aria2_dir() -> Option<PathBuf> {
    if let Some(exe) = exe_dir() {
        let dir = exe.join("aria2");
        if let Ok(true) = std::fs::create_dir_all(&dir)
            .and_then(|_| {
                let test = dir.join(".wtest");
                std::fs::write(&test, "").and_then(|_| std::fs::remove_file(&test))
            })
            .map(|_| true)
        {
            return Some(dir);
        }
    }
    let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
    let dir = proj.data_dir().join("aria2");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

pub fn log_dir() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
    let dir = proj.data_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

pub fn session_dir() -> Option<PathBuf> {
    aria2_dir()
}

pub fn aria2_bin_dir() -> Option<PathBuf> {
    aria2_dir()
}

pub fn announce() {
    if let Some(p) = config_path() {
        tracing::info!(?p, "config path");
    }
    if let Some(p) = log_dir() {
        tracing::info!(?p, "log dir");
    }
}
