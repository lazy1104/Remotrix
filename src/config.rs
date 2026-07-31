use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aria2_ws::TaskOptions;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::i18n::Locale;
use crate::ui::theme::ThemeMode;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aria2Options {
    #[serde(default = "default_max_connection_per_server")]
    pub max_connection_per_server: u32,
    #[serde(default = "default_min_split_size_mb")]
    pub min_split_size_mb: u64,
    #[serde(default = "default_true")]
    pub auto_file_renaming: bool,
    #[serde(default)]
    pub allow_overwrite: bool,
    #[serde(default = "default_true")]
    pub r#continue: bool,
    #[serde(default)]
    pub check_integrity: bool,
    #[serde(default)]
    pub max_download_limit_kb: u64,
    #[serde(default)]
    pub max_upload_limit_kb: u64,
    #[serde(default)]
    pub lowest_speed_limit_kb: u64,
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    #[serde(default)]
    pub all_proxy: String,
    #[serde(default = "default_max_tries")]
    pub max_tries: u32,
    #[serde(default)]
    pub retry_wait: u32,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u32,
    #[serde(default)]
    pub bt_tracker: String,
    #[serde(default = "default_seed_ratio")]
    pub seed_ratio: f64,
    #[serde(default)]
    pub seed_time: u32,
    #[serde(default = "default_true")]
    pub enable_dht: bool,
    #[serde(default)]
    pub bt_require_crypto: bool,
    #[serde(default)]
    pub proxy_enabled: bool,
}

fn default_max_connection_per_server() -> u32 {
    16
}
fn default_min_split_size_mb() -> u64 {
    1
}
fn default_user_agent() -> String {
    format!("Remotrix/{}", env!("CARGO_PKG_VERSION"))
}
fn default_max_tries() -> u32 {
    5
}
fn default_connect_timeout() -> u32 {
    60
}
fn default_seed_ratio() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}
fn default_light_theme() -> String {
    "silkcircuit-dawn".into()
}
fn default_dark_theme() -> String {
    "silkcircuit-neon".into()
}
fn default_window_width() -> f32 {
    1040.0
}
fn default_window_height() -> f32 {
    720.0
}

impl Default for Aria2Options {
    fn default() -> Self {
        Self {
            max_connection_per_server: 16,
            min_split_size_mb: 1,
            auto_file_renaming: true,
            allow_overwrite: false,
            r#continue: true,
            check_integrity: false,
            max_download_limit_kb: 0,
            max_upload_limit_kb: 0,
            lowest_speed_limit_kb: 0,
            user_agent: default_user_agent(),
            all_proxy: String::new(),
            max_tries: 5,
            retry_wait: 0,
            connect_timeout: 60,
            bt_tracker: String::new(),
            seed_ratio: 1.0,
            seed_time: 0,
            enable_dht: true,
            bt_require_crypto: false,
            proxy_enabled: false,
        }
    }
}

impl Settings {
    pub fn to_aria2_task_options(&self) -> TaskOptions {
        let mut extra = Map::new();

        extra.insert(
            "max-concurrent-downloads".into(),
            Value::String(self.max_concurrent.to_string()),
        );

        extra.insert(
            "min-split-size".into(),
            Value::String((self.aria2.min_split_size_mb * 1024 * 1024).to_string()),
        );

        extra.insert(
            "allow-overwrite".into(),
            Value::String(
                if self.aria2.allow_overwrite {
                    "true"
                } else {
                    "false"
                }
                .into(),
            ),
        );

        extra.insert(
            "max-overall-download-limit".into(),
            Value::String((self.download_limit_kb * 1024).to_string()),
        );

        extra.insert(
            "max-overall-upload-limit".into(),
            Value::String((self.upload_limit_kb * 1024).to_string()),
        );

        extra.insert(
            "max-upload-limit".into(),
            Value::String((self.aria2.max_upload_limit_kb * 1024).to_string()),
        );

        if !self.aria2.user_agent.is_empty() {
            extra.insert(
                "user-agent".into(),
                Value::String(self.aria2.user_agent.clone()),
            );
        }

        extra.insert(
            "retry-wait".into(),
            Value::String(self.aria2.retry_wait.to_string()),
        );

        extra.insert(
            "connect-timeout".into(),
            Value::String(self.aria2.connect_timeout.to_string()),
        );

        if !self.aria2.bt_tracker.is_empty() {
            extra.insert(
                "bt-tracker".into(),
                Value::String(self.aria2.bt_tracker.clone()),
            );
        }

        extra.insert(
            "seed-ratio".into(),
            Value::String(self.aria2.seed_ratio.to_string()),
        );

        if self.aria2.seed_time > 0 {
            extra.insert(
                "seed-time".into(),
                Value::String(self.aria2.seed_time.to_string()),
            );
        }

        extra.insert(
            "enable-dht".into(),
            Value::String(
                if self.aria2.enable_dht {
                    "true"
                } else {
                    "false"
                }
                .into(),
            ),
        );

        extra.insert(
            "bt-require-crypto".into(),
            Value::String(
                if self.aria2.bt_require_crypto {
                    "true"
                } else {
                    "false"
                }
                .into(),
            ),
        );

        TaskOptions {
            split: Some(self.split as i32),
            max_connection_per_server: Some(self.aria2.max_connection_per_server as i32),
            auto_file_renaming: Some(self.aria2.auto_file_renaming),
            r#continue: Some(self.aria2.r#continue),
            check_integrity: Some(self.aria2.check_integrity),
            lowest_speed_limit: Some((self.aria2.lowest_speed_limit_kb * 1024).to_string()),
            max_download_limit: Some((self.aria2.max_download_limit_kb * 1024).to_string()),
            header: None,
            all_proxy: if self.aria2.proxy_enabled && !self.aria2.all_proxy.is_empty() {
                Some(self.aria2.all_proxy.clone())
            } else {
                Some(String::new())
            },
            max_tries: Some(self.aria2.max_tries as i32),
            timeout: Some(self.aria2.connect_timeout as i32),
            extra_options: extra,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub download_dir: PathBuf,
    pub max_concurrent: u32,
    pub download_limit_kb: u64,
    pub upload_limit_kb: u64,
    pub split: u16,
    #[serde(default = "default_light_theme")]
    pub light_theme: String,
    #[serde(default = "default_dark_theme")]
    pub dark_theme: String,
    #[serde(default)]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub locale: Locale,
    #[serde(default)]
    pub update: UpdatePrefs,
    #[serde(default)]
    pub aria2: Aria2Options,
    #[serde(default = "default_true")]
    pub nav_to_tasks_after_add: bool,
    #[serde(default)]
    pub delete_torrent_after_complete: bool,
    #[serde(default = "default_window_width")]
    pub window_width: f32,
    #[serde(default = "default_window_height")]
    pub window_height: f32,
    #[serde(default)]
    pub window_maximized: bool,
    #[serde(default)]
    pub path_history: std::collections::HashMap<String, Vec<String>>,
}

impl Settings {
    pub fn apply_fields_equal(&self, other: &Settings) -> bool {
        self.download_dir == other.download_dir
            && self.max_concurrent == other.max_concurrent
            && self.download_limit_kb == other.download_limit_kb
            && self.upload_limit_kb == other.upload_limit_kb
            && self.split == other.split
            && self.nav_to_tasks_after_add == other.nav_to_tasks_after_add
            && self.delete_torrent_after_complete == other.delete_torrent_after_complete
            && self.aria2 == other.aria2
    }

    pub fn record_path(&mut self, key: &str, path: &str) {
        let e = self.path_history.entry(key.to_string()).or_default();
        e.retain(|p| p != path);
        e.insert(0, path.to_string());
        if e.len() > 10 {
            e.truncate(10);
        }
    }
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
            light_theme: default_light_theme(),
            dark_theme: default_dark_theme(),
            theme_mode: ThemeMode::System,
            locale: Locale::default(),
            update: UpdatePrefs::default(),
            aria2: Aria2Options::default(),
            nav_to_tasks_after_add: true,
            delete_torrent_after_complete: false,
            window_width: default_window_width(),
            window_height: default_window_height(),
            window_maximized: false,
            path_history: std::collections::HashMap::new(),
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

fn aria2_dir() -> Option<PathBuf> {
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

pub fn db_path() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
    let dir = proj.data_dir().to_path_buf();
    Some(dir.join("remotrix.db"))
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
    if let Some(p) = aria2_dir() {
        tracing::info!(?p, "aria2 dir");
    }
}
