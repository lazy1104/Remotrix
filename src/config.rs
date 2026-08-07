use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aria2_ws::TaskOptions;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::clipboard_watch::ClipboardLinkTypes;
use crate::i18n::Locale;
use crate::scheduler::{in_speed_window, weekday_active};
use crate::ui::theme::ThemeMode;

pub const MAX_CONCURRENT_DOWNLOADS: u32 = 32;

pub const TRACKER_SOURCE_OPTIONS: &[(&str, &str, &str)] = &[
    (
        "ngosang",
        "trackerslist",
        "https://ngosang.github.io/trackerslist/trackers_best.txt",
    ),
    (
        "XIU2",
        "TrackersListCollection",
        "https://cf.trackerslist.com/best.txt",
    ),
];

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
    #[serde(default, alias = "all_proxy")]
    pub proxy_server: String,
    #[serde(default)]
    pub proxy_username: String,
    #[serde(default)]
    pub proxy_password: String,
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
    #[serde(default = "default_true")]
    pub bt_enable_lpd: bool,
    #[serde(default = "default_true")]
    pub enable_peer_exchange: bool,
    #[serde(default)]
    pub bt_auto_download: bool,
    #[serde(default = "default_file_allocation")]
    pub file_allocation: String,
    #[serde(default = "default_disk_cache_mb")]
    pub disk_cache_mb: u64,
    #[serde(default)]
    pub proxy_enabled: bool,
    #[serde(default)]
    pub ed2k_server: String,
    #[serde(default)]
    pub ed2k_server_list: String,
    #[serde(default)]
    pub ed2k_node_list: String,
    #[serde(default = "default_ed2k_listen_port")]
    pub ed2k_listen_port: u16,
    #[serde(default = "default_ed2k_udp_listen_port")]
    pub ed2k_udp_listen_port: u16,
    #[serde(default = "default_ed2k_upload_slots")]
    pub ed2k_upload_slots: u16,
}

fn default_max_connection_per_server() -> u32 {
    16
}
fn default_min_split_size_mb() -> u64 {
    1
}
fn default_user_agent() -> String {
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
        .to_string()
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
fn default_file_allocation() -> String {
    "prealloc".into()
}
fn default_disk_cache_mb() -> u64 {
    16
}
fn default_ed2k_listen_port() -> u16 {
    4662
}
fn default_ed2k_udp_listen_port() -> u16 {
    4672
}
fn default_ed2k_upload_slots() -> u16 {
    3
}
fn default_tracker_sources() -> Vec<String> {
    TRACKER_SOURCE_OPTIONS
        .iter()
        .map(|(_, _, url)| url.to_string())
        .collect()
}
fn default_tracker_sync_interval() -> u32 {
    24
}
fn default_theme_color() -> String {
    crate::ui::theme::color_to_hex(crate::ui::theme::DEFAULT_THEME_COLOR)
}
fn default_font_family() -> String {
    crate::ui::theme::BUNDLED_FONT_NAME.into()
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
            proxy_server: String::new(),
            proxy_username: String::new(),
            proxy_password: String::new(),
            max_tries: 5,
            retry_wait: 0,
            connect_timeout: 60,
            bt_tracker: String::new(),
            seed_ratio: 1.0,
            seed_time: 0,
            enable_dht: true,
            bt_require_crypto: false,
            bt_enable_lpd: true,
            enable_peer_exchange: true,
            bt_auto_download: false,
            file_allocation: default_file_allocation(),
            disk_cache_mb: default_disk_cache_mb(),
            proxy_enabled: false,
            ed2k_server: String::new(),
            ed2k_server_list: String::new(),
            ed2k_node_list: String::new(),
            ed2k_listen_port: default_ed2k_listen_port(),
            ed2k_udp_listen_port: default_ed2k_udp_listen_port(),
            ed2k_upload_slots: default_ed2k_upload_slots(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeedLimitSchedule {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_schedule_start")]
    pub start: String,
    #[serde(default = "default_schedule_end")]
    pub end: String,
    #[serde(default = "default_schedule_weekdays")]
    pub weekdays: Vec<u8>,
}

impl Default for SpeedLimitSchedule {
    fn default() -> Self {
        Self {
            enabled: false,
            start: default_schedule_start(),
            end: default_schedule_end(),
            weekdays: default_schedule_weekdays(),
        }
    }
}

fn default_schedule_start() -> String {
    "23:00".into()
}

fn default_schedule_end() -> String {
    "07:00".into()
}

fn default_schedule_weekdays() -> Vec<u8> {
    vec![1, 2, 3, 4, 5, 6, 7]
}

impl SpeedLimitSchedule {
    pub fn active_at(&self, now: &chrono::DateTime<chrono::Local>) -> bool {
        self.enabled
            && in_speed_window(&self.start, &self.end, now)
            && weekday_active(&self.weekdays, now)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogPrefs {
    #[serde(default = "default_app_log_level")]
    pub app_level: String,
    #[serde(default = "default_engine_log_level")]
    pub engine_level: String,
}

impl Default for LogPrefs {
    fn default() -> Self {
        Self {
            app_level: default_app_log_level(),
            engine_level: default_engine_log_level(),
        }
    }
}

fn default_app_log_level() -> String {
    crate::logging::DEFAULT_APP_LEVEL.into()
}

fn default_engine_log_level() -> String {
    crate::logging::DEFAULT_ENGINE_LEVEL.into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackerPrefs {
    #[serde(default = "default_tracker_sources")]
    pub sources: Vec<String>,
    #[serde(default)]
    pub custom_urls: Vec<String>,
    #[serde(default = "default_true")]
    pub auto_sync: bool,
    #[serde(default = "default_tracker_sync_interval")]
    pub sync_interval_hours: u32,
    #[serde(default)]
    pub last_sync_time: Option<i64>,
}

impl Default for TrackerPrefs {
    fn default() -> Self {
        Self {
            sources: default_tracker_sources(),
            custom_urls: Vec::new(),
            auto_sync: true,
            sync_interval_hours: default_tracker_sync_interval(),
            last_sync_time: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationPrefs {
    #[serde(default = "default_true")]
    pub download_complete: bool,
    #[serde(default = "default_true")]
    pub download_error: bool,
    #[serde(default = "default_true")]
    pub engine_degraded: bool,
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        Self {
            download_complete: true,
            download_error: true,
            engine_degraded: true,
        }
    }
}

pub fn apply_proxy(
    builder: reqwest::ClientBuilder,
    proxy: Option<&str>,
) -> Result<reqwest::ClientBuilder, String> {
    match proxy {
        Some(p) => {
            let proxy = reqwest::Proxy::all(p).map_err(|e| format!("proxy: {e}"))?;
            Ok(builder.proxy(proxy))
        }
        None => Ok(builder),
    }
}

pub fn all_proxy_url(server: &str, username: &str, password: &str) -> Option<String> {
    if server.trim().is_empty() {
        return None;
    }
    let server = server.trim();
    let auth = if username.is_empty() {
        String::new()
    } else {
        format!("{}:{}@", username, password)
    };
    if let Some((scheme, rest)) = server.split_once("://") {
        Some(format!("{scheme}://{auth}{rest}"))
    } else {
        Some(format!("http://{auth}{server}"))
    }
}

impl Aria2Options {
    pub fn all_proxy_value(&self) -> Option<String> {
        if !self.proxy_enabled {
            return None;
        }
        all_proxy_url(
            &self.proxy_server,
            &self.proxy_username,
            &self.proxy_password,
        )
    }

    pub fn ed2k_startup_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if !self.ed2k_server.trim().is_empty() {
            args.push("--ed2k-server".into());
            args.push(self.ed2k_server.trim().to_string());
        }
        if !self.ed2k_server_list.trim().is_empty() {
            args.push("--ed2k-server-list".into());
            args.push(self.ed2k_server_list.trim().to_string());
        }
        if !self.ed2k_node_list.trim().is_empty() {
            args.push("--ed2k-node-list".into());
            args.push(self.ed2k_node_list.trim().to_string());
        }
        args.push("--ed2k-listen-port".into());
        args.push(self.ed2k_listen_port.to_string());
        args.push("--ed2k-udp-listen-port".into());
        args.push(self.ed2k_udp_listen_port.to_string());
        args.push("--ed2k-upload-slots".into());
        args.push(self.ed2k_upload_slots.to_string());
        args
    }

    pub fn ed2k_equal(&self, other: &Aria2Options) -> bool {
        self.ed2k_server.trim() == other.ed2k_server.trim()
            && self.ed2k_server_list.trim() == other.ed2k_server_list.trim()
            && self.ed2k_node_list.trim() == other.ed2k_node_list.trim()
            && self.ed2k_listen_port == other.ed2k_listen_port
            && self.ed2k_udp_listen_port == other.ed2k_udp_listen_port
            && self.ed2k_upload_slots == other.ed2k_upload_slots
    }
}

impl Settings {
    pub fn to_aria2_task_options(&self) -> TaskOptions {
        let mut extra = Map::new();

        extra.insert(
            "max-concurrent-downloads".into(),
            Value::String(
                self.max_concurrent
                    .min(MAX_CONCURRENT_DOWNLOADS)
                    .to_string(),
            ),
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

        let bt_tracker = crate::trackers::to_comma(&self.aria2.bt_tracker);
        if !bt_tracker.is_empty() {
            extra.insert(
                "bt-tracker".into(),
                Value::String(crate::trackers::reduce(bt_tracker)),
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

        extra.insert(
            "bt-enable-lpd".into(),
            Value::String(
                if self.aria2.bt_enable_lpd {
                    "true"
                } else {
                    "false"
                }
                .into(),
            ),
        );

        extra.insert(
            "enable-peer-exchange".into(),
            Value::String(
                if self.aria2.enable_peer_exchange {
                    "true"
                } else {
                    "false"
                }
                .into(),
            ),
        );

        extra.insert(
            "file-allocation".into(),
            Value::String(self.aria2.file_allocation.clone()),
        );

        extra.insert(
            "disk-cache".into(),
            Value::String(format!("{}M", self.aria2.disk_cache_mb)),
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
            all_proxy: Some(self.aria2.all_proxy_value().unwrap_or_default()),
            max_tries: Some(self.aria2.max_tries as i32),
            timeout: Some(self.aria2.connect_timeout as i32),
            extra_options: extra,
            ..Default::default()
        }
    }

    pub fn effective_task_options(&self) -> TaskOptions {
        let mut options = self.to_aria2_task_options();
        if self.speed_limit_schedule.enabled
            && !self.speed_limit_schedule.active_at(&chrono::Local::now())
        {
            options.extra_options.insert(
                "max-overall-download-limit".into(),
                Value::String("0".into()),
            );
            options
                .extra_options
                .insert("max-overall-upload-limit".into(), Value::String("0".into()));
        }
        options
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub download_dir: PathBuf,
    pub max_concurrent: u32,
    pub download_limit_kb: u64,
    pub upload_limit_kb: u64,
    pub split: u16,
    #[serde(default = "default_theme_color")]
    pub theme_color: String,
    #[serde(default)]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub locale: Locale,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default)]
    pub update: UpdatePrefs,
    #[serde(default)]
    pub aria2: Aria2Options,
    #[serde(default = "default_true")]
    pub nav_to_tasks_after_add: bool,
    #[serde(default)]
    pub close_to_tray: bool,
    #[serde(default)]
    pub delete_torrent_after_complete: bool,
    #[serde(default)]
    pub cleanup_completed_on_close: bool,
    #[serde(default)]
    pub remove_task_if_files_missing: bool,
    #[serde(default = "default_true")]
    pub detect_clipboard_on_start: bool,
    #[serde(default)]
    pub clipboard_types: ClipboardLinkTypes,
    #[serde(default)]
    pub last_clipboard_hash: String,
    #[serde(default = "default_window_width")]
    pub window_width: f32,
    #[serde(default = "default_window_height")]
    pub window_height: f32,
    #[serde(default)]
    pub window_maximized: bool,
    #[serde(default)]
    pub path_history: std::collections::HashMap<String, Vec<String>>,
    #[serde(default)]
    pub speed_limit_schedule: SpeedLimitSchedule,
    #[serde(default)]
    pub log: LogPrefs,
    #[serde(default)]
    pub tracker: TrackerPrefs,
    #[serde(default)]
    pub notifications: NotificationPrefs,
    #[serde(default)]
    pub autostart_enabled: bool,
    #[serde(default)]
    pub start_hidden_on_autostart: bool,
}

impl Settings {
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
            theme_color: default_theme_color(),
            theme_mode: ThemeMode::System,
            locale: Locale::default(),
            font_family: default_font_family(),
            update: UpdatePrefs::default(),
            aria2: Aria2Options::default(),
            nav_to_tasks_after_add: true,
            close_to_tray: false,
            delete_torrent_after_complete: false,
            cleanup_completed_on_close: false,
            remove_task_if_files_missing: false,
            detect_clipboard_on_start: true,
            clipboard_types: ClipboardLinkTypes::default(),
            last_clipboard_hash: String::new(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            window_maximized: false,
            path_history: std::collections::HashMap::new(),
            speed_limit_schedule: SpeedLimitSchedule::default(),
            log: LogPrefs::default(),
            tracker: TrackerPrefs::default(),
            notifications: NotificationPrefs::default(),
            autostart_enabled: false,
            start_hidden_on_autostart: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UpdateScope {
    #[serde(rename = "app")]
    App,
    #[serde(rename = "engine")]
    Engine,
    #[default]
    #[serde(rename = "both")]
    Both,
}

impl UpdateScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Engine => "engine",
            Self::Both => "both",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "app" => Some(Self::App),
            "engine" => Some(Self::Engine),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    pub fn covers(self, component: &str) -> bool {
        matches!(
            (component, self),
            ("aria2-next", Self::Engine | Self::Both) | ("remotrix", Self::App | Self::Both)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdatePrefs {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub scope: UpdateScope,
    #[serde(default)]
    pub interval_hours: u32,
    #[serde(default)]
    pub last_check_time: Option<i64>,
    #[serde(default)]
    pub components: HashMap<String, ComponentUpdatePrefs>,
    #[serde(default = "default_true")]
    pub aria2_silent_update: bool,
}

impl Default for UpdatePrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            scope: UpdateScope::Both,
            interval_hours: 0,
            last_check_time: None,
            components: HashMap::new(),
            aria2_silent_update: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ComponentUpdatePrefs {
    #[serde(default)]
    pub skipped: Vec<String>,
}

impl UpdatePrefs {
    pub fn check_due(&self, startup: bool, now_ms: i64) -> bool {
        if !self.enabled {
            return false;
        }
        if self.interval_hours == 0 {
            return startup;
        }
        let last = self.last_check_time.unwrap_or(0);
        if last <= 0 {
            return true;
        }
        now_ms - last >= self.interval_hours as i64 * 3600 * 1000
    }

    pub fn is_skipped(&self, component: &str, version: &str) -> bool {
        self.components
            .get(component)
            .map(|c| c.skipped.contains(&version.to_string()))
            .unwrap_or(false)
    }
}

fn config_path() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
    let dir = proj.config_dir().to_path_buf();
    Some(dir.join("settings.json"))
}

pub fn config_file_path() -> Option<PathBuf> {
    config_path()
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
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, &json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn install_desktop_file() {
    let Some(data_home) = data_home() else { return };
    let dir = data_home.join("applications");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("remotrix.desktop");
    let Some(exe) = std::env::current_exe().ok() else {
        return;
    };
    let content = format!(
        "{}StartupWMClass=remotrix\n",
        desktop_entry_header(&format!("\"{}\"", escape_exec(&exe.display().to_string())))
    );
    if path.exists() {
        if let Ok(existing) = std::fs::read_to_string(&path) {
            if existing == content {
                return;
            }
        }
    }
    let tmp = path.with_extension("desktop.tmp");
    if std::fs::write(&tmp, content).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub(crate) fn desktop_entry_header(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Remotrix\n\
         Comment=Download manager\n\
         Exec={exec}\n\
         Terminal=false\n\
         Categories=Network;FileTransfer;\n"
    )
}

pub(crate) fn escape_exec(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn data_home() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    directories::BaseDirs::new().map(|b| b.data_dir().to_path_buf())
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
    if let Some(p) = crate::logging::engine_log_path() {
        tracing::info!(?p, "engine log path");
    }
    if let Some(p) = aria2_dir() {
        tracing::info!(?p, "aria2 dir");
    }
}
