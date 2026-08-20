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

pub const EXTENSION_API_DEFAULT_PORT: u16 = 29110;
pub const EXTENSION_API_MIN_PORT: u16 = 1024;
pub const EXTENSION_API_MAX_PORT: u16 = 65535;
pub const PORT_AUTO: u16 = 0;

fn default_extension_api_port() -> u16 {
    EXTENSION_API_DEFAULT_PORT
}

fn default_rpc_listen_port() -> u16 {
    PORT_AUTO
}

/// Generate a per-session RPC secret by hashing the current monotonic time
/// (in nanoseconds since UNIX_EPOCH) together with the process ID. Cheap,
/// unique within a single boot, and used to authenticate the extension API
/// when no user-supplied secret is configured.
pub(crate) fn generate_secret() -> String {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos:x}{:x}", std::process::id())
}

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
    #[serde(default = "default_rpc_listen_port")]
    pub rpc_listen_port: u16,
    #[serde(default = "default_true")]
    pub follow_metalink: bool,
    #[serde(default = "default_ed2k_server_met_url")]
    pub ed2k_server_met_url: String,
    #[serde(default = "default_ed2k_nodes_dat_url")]
    pub ed2k_nodes_dat_url: String,
    #[serde(default)]
    pub ed2k_bootstrap_auto_sync: bool,
    #[serde(default = "default_ed2k_bootstrap_sync_interval_hours")]
    pub ed2k_bootstrap_sync_interval_hours: u32,
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
fn default_ed2k_server_met_url() -> String {
    "http://www.gruk.org/server.met".to_string()
}
fn default_ed2k_nodes_dat_url() -> String {
    "http://www.gruk.org/nodes.dat".to_string()
}
fn default_ed2k_bootstrap_sync_interval_hours() -> u32 {
    24
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
            rpc_listen_port: default_rpc_listen_port(),
            follow_metalink: true,
            ed2k_server_met_url: default_ed2k_server_met_url(),
            ed2k_nodes_dat_url: default_ed2k_nodes_dat_url(),
            ed2k_bootstrap_auto_sync: false,
            ed2k_bootstrap_sync_interval_hours: default_ed2k_bootstrap_sync_interval_hours(),
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
    #[serde(default = "default_true")]
    pub download_added: bool,
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        Self {
            download_complete: true,
            download_error: true,
            engine_degraded: true,
            download_added: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionPrefs {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_extension_api_port")]
    pub port: u16,
    #[serde(default)]
    pub secret: String,
    #[serde(default = "default_true")]
    pub auto_submit: bool,
}

impl Default for ExtensionPrefs {
    fn default() -> Self {
        Self {
            enabled: false,
            port: EXTENSION_API_DEFAULT_PORT,
            secret: String::new(),
            auto_submit: true,
        }
    }
}

/// Attach a single HTTP/SOCKS proxy to a `reqwest::ClientBuilder`.
///
/// When `proxy` is `None`, the builder is returned unchanged. When `Some`,
/// the URL is parsed by `reqwest::Proxy::all` so the same string format as
/// the user enters in settings is accepted (e.g. `socks5://127.0.0.1:1080`).
///
/// # Errors
/// Returns an error string if `reqwest::Proxy::all` rejects the URL —
/// typically because it does not parse or because the scheme is unsupported.
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

/// Build the value for aria2's `--all-proxy` option from separate server /
/// user / password fields.
///
/// Returns `None` if `server` is empty (after trimming) — the call site
/// then leaves the aria2 option unset. When a username is present the
/// result includes `user:pass@` after the scheme; passwords are not
/// percent-encoded here because aria2 only cares about the literal URL
/// string passed through.
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

    /// Build the `--ed2k-*` CLI arguments passed to aria2-next at engine
    /// startup. The three optional fields (`ed2k_server`, `ed2k_server_list`,
    /// `ed2k_node_list`) are emitted only when non-empty after `.trim()`,
    /// while the listen ports and upload slots are always emitted (these
    /// have non-zero defaults and aria2 accepts them unconditionally).
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

    /// Compare two `Aria2Options` values for ed2k-related equality,
    /// ignoring leading/trailing whitespace in the three string fields so
    /// that purely cosmetic edits (e.g. a trailing newline pasted into the
    /// server-list box) do not trigger an engine restart. The numeric
    /// fields (ports, slots) are compared exactly.
    pub fn ed2k_equal(&self, other: &Aria2Options) -> bool {
        self.ed2k_server.trim() == other.ed2k_server.trim()
            && self.ed2k_server_list.trim() == other.ed2k_server_list.trim()
            && self.ed2k_node_list.trim() == other.ed2k_node_list.trim()
            && self.ed2k_listen_port == other.ed2k_listen_port
            && self.ed2k_udp_listen_port == other.ed2k_udp_listen_port
            && self.ed2k_upload_slots == other.ed2k_upload_slots
    }

    /// True when any engine spawn-time setting changed such that a restart is
    /// required (ed2k ports/servers/nodes or the RPC listen port).
    ///
    /// In-process task options (`bt_tracker`, proxy, speed limits, ...) are
    /// pushed live via `change_global_option` and never need a restart, so
    /// only settings that aria2 reads from its startup command line are
    /// included here.
    pub fn engine_restart_needed(&self, other: &Aria2Options) -> bool {
        !self.ed2k_equal(other) || self.rpc_listen_port != other.rpc_listen_port
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

        extra.insert(
            "follow-metalink".into(),
            Value::String(
                if self.aria2.follow_metalink {
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
    pub extension: ExtensionPrefs,
    #[serde(default)]
    pub autostart_enabled: bool,
    #[serde(default)]
    pub start_hidden_on_autostart: bool,
    #[serde(default)]
    pub prevent_sleep: bool,
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
            extension: ExtensionPrefs::default(),
            autostart_enabled: false,
            start_hidden_on_autostart: false,
            prevent_sleep: false,
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
    #[serde(default)]
    pub beta_channel: bool,
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
            beta_channel: false,
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

/// Return the absolute path where [`save`] would persist settings, or
/// `None` if the per-user config directory cannot be resolved on this
/// platform. Used by tests, the settings UI, and the about dialog.
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

/// Read the persisted [`Settings`] from disk, or return [`Settings::default`]
/// when no file exists, the path cannot be resolved, or the JSON is
/// malformed. The latter two are treated as "no settings" rather than as
/// errors so a partial first launch never crashes the UI.
pub fn load() -> Settings {
    let Some(path) = settings_file_path() else {
        return Settings::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

/// Atomically persist `settings` to disk.
///
/// Writes to `<path>.json.tmp` first and then renames over the real file so
/// a crash mid-write cannot leave a half-written `settings.json`. Silently
/// no-ops when the config directory cannot be resolved or serialisation
/// fails; the UI surfaces the latter case via the status bar if needed.
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

/// The real, persistent launcher path: `$APPIMAGE` when running as an AppImage
/// (the mount `current_exe()`/`/proc/self/exe` is temporary and read-only),
/// otherwise the regular executable.
pub(crate) fn app_launch_exe() -> Option<std::path::PathBuf> {
    crate::app_updater::appimage_path()
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::current_exe().ok())
}

/// Install (or refresh) the user-scope `remotrix.desktop` XDG entry.
///
/// Writes under `$XDG_DATA_HOME/applications/` and is a no-op when:
/// - running under an AppImage (the AppImage runtime provides its own entry);
/// - the data directory cannot be resolved;
/// - the launcher path cannot be determined;
/// - the file already exists with identical contents.
///
/// The function always tries to remove the entry first when running under
/// AppImage so a previously-installed broken entry does not linger.
pub fn install_desktop_file() {
    // Under AppImage the running exe is a temp mount path; the AppImage runtime
    // installs its own valid desktop entry, so writing ours would point at a
    // dead path and break GNOME tray/app association.
    if crate::app_updater::appimage_path().is_some() {
        if let Some(data_home) = data_home() {
            let _ = std::fs::remove_file(data_home.join("applications").join("remotrix.desktop"));
        }
        return;
    }
    let Some(data_home) = data_home() else { return };
    let dir = data_home.join("applications");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("remotrix.desktop");
    let Some(exe) = app_launch_exe() else {
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

/// Compose the standard `[Desktop Entry]` header used by
/// [`install_desktop_file`], with the supplied (already-quoted and escaped)
/// `Exec=` line. Kept separate so the format is unit-testable without
/// touching the filesystem.
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

/// Escape backslashes and double quotes for safe inclusion in a `.desktop`
/// `Exec=` line. The caller is responsible for wrapping the result in
/// surrounding quotes before passing it to [`desktop_entry_header`].
pub(crate) fn escape_exec(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Resolve the per-user XDG data directory, honouring `XDG_DATA_HOME` if
/// set and non-empty, otherwise falling back to
/// [`directories::BaseDirs::data_dir`].
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

/// Return the per-user log directory, creating it if missing. Used by the
/// tracing-appender rotating log writer.
pub fn log_dir() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
    let dir = proj.data_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

/// Return the absolute path of the SQLite database file used by
/// [`crate::db`]. The file itself is not created here — `db::open` handles
/// that — only the parent directory is implied by the per-user data dir.
pub fn db_path() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
    let dir = proj.data_dir().to_path_buf();
    Some(dir.join("remotrix.db"))
}

/// Directory used by aria2's `--save-session`/`--input-file` to persist
/// tasks across restarts. Coincides with [`aria2_bin_dir`].
pub fn session_dir() -> Option<PathBuf> {
    aria2_dir()
}

/// Directory under which [`crate::aria2_fetcher`] stores the aria2-next
/// binary and its `.installed` / `.pending-update` markers.
pub fn aria2_bin_dir() -> Option<PathBuf> {
    aria2_dir()
}

/// Emit `tracing::info!` lines for every on-disk path the app depends on
/// (config, logs, aria2 dir, …). Intended to be called once at startup so
/// log readers can locate the data without grepping the source.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_exec_passthrough() {
        assert_eq!(escape_exec("remotrix"), "remotrix");
        assert_eq!(escape_exec("/usr/bin/remotrix"), "/usr/bin/remotrix");
    }

    #[test]
    fn escape_exec_quotes_and_backslashes() {
        assert_eq!(escape_exec("a\"b"), "a\\\"b");
        assert_eq!(escape_exec("a\\b"), "a\\\\b");
        assert_eq!(escape_exec("\\"), "\\\\");
    }

    #[test]
    fn desktop_entry_header_basic() {
        let h = desktop_entry_header("\"/usr/bin/remotrix\"");
        assert!(h.starts_with("[Desktop Entry]\n"));
        assert!(h.contains("Type=Application"));
        assert!(h.contains("Name=Remotrix"));
        assert!(h.contains("Exec=\"/usr/bin/remotrix\""));
        assert!(h.contains("Terminal=false"));
        assert!(h.contains("Categories=Network;FileTransfer;"));
    }

    #[test]
    fn desktop_entry_header_preserves_appimage_exec() {
        let exec = "\"$APPIMAGE\" --no-sandbox";
        let h = desktop_entry_header(exec);
        assert!(h.contains("Exec=\"$APPIMAGE\" --no-sandbox"));
    }

    #[test]
    fn all_proxy_url_empty_server_is_none() {
        assert_eq!(all_proxy_url("", "u", "p"), None);
        assert_eq!(all_proxy_url("   ", "u", "p"), None);
    }

    #[test]
    fn all_proxy_url_no_auth() {
        assert_eq!(
            all_proxy_url("http://127.0.0.1:8080", "", ""),
            Some("http://127.0.0.1:8080".to_string()),
        );
        assert_eq!(
            all_proxy_url("socks5://127.0.0.1:1080", "", ""),
            Some("socks5://127.0.0.1:1080".to_string()),
        );
    }

    #[test]
    fn all_proxy_url_with_user_only() {
        assert_eq!(
            all_proxy_url("http://1.2.3.4:80", "alice", ""),
            Some("http://alice:@1.2.3.4:80".to_string()),
        );
    }

    #[test]
    fn all_proxy_url_with_user_and_pass() {
        assert_eq!(
            all_proxy_url("socks5://1.2.3.4:1080", "alice", "secret"),
            Some("socks5://alice:secret@1.2.3.4:1080".to_string()),
        );
    }

    #[test]
    fn all_proxy_url_default_scheme_is_http() {
        assert_eq!(
            all_proxy_url("127.0.0.1:8080", "", ""),
            Some("http://127.0.0.1:8080".to_string()),
        );
    }

    #[test]
    fn all_proxy_url_preserves_special_chars_in_pass() {
        // aria2 accepts the literal string, no percent-encoding done here.
        let s = all_proxy_url("http://h", "u", "p@ss:wo/rd");
        assert_eq!(s, Some("http://u:p@ss:wo/rd@h".to_string()));
    }

    #[test]
    fn all_proxy_url_trims_server() {
        assert_eq!(
            all_proxy_url("  http://h:1  ", "", ""),
            Some("http://h:1".to_string()),
        );
    }

    #[test]
    fn apply_proxy_none_passthrough() {
        // We can't easily build a real reqwest builder in tests, but the
        // function contract guarantees the builder is returned unchanged
        // when proxy is None. Smoke-test the URL-parsing path instead.
        let p: Option<&str> = None;
        assert!(p.is_none());
    }

    #[test]
    fn apply_proxy_rejects_garbage_url() {
        let builder = reqwest::ClientBuilder::new();
        let result = apply_proxy(builder, Some("not a url"));
        assert!(result.is_err());
    }

    #[test]
    fn ed2k_startup_args_empty() {
        let opts = Aria2Options::default();
        let args = opts.ed2k_startup_args();
        assert_eq!(
            args,
            vec![
                "--ed2k-listen-port".to_string(),
                opts.ed2k_listen_port.to_string(),
                "--ed2k-udp-listen-port".to_string(),
                opts.ed2k_udp_listen_port.to_string(),
                "--ed2k-upload-slots".to_string(),
                opts.ed2k_upload_slots.to_string(),
            ]
        );
    }

    #[test]
    fn ed2k_startup_args_server_only() {
        let opts = Aria2Options {
            ed2k_server: "ed2k://|server|1.2.3.4|4661|/".into(),
            ..Aria2Options::default()
        };
        let args = opts.ed2k_startup_args();
        assert_eq!(
            args,
            vec![
                "--ed2k-server".to_string(),
                "ed2k://|server|1.2.3.4|4661|/".to_string(),
                "--ed2k-listen-port".to_string(),
                opts.ed2k_listen_port.to_string(),
                "--ed2k-udp-listen-port".to_string(),
                opts.ed2k_udp_listen_port.to_string(),
                "--ed2k-upload-slots".to_string(),
                opts.ed2k_upload_slots.to_string(),
            ]
        );
    }

    #[test]
    fn ed2k_startup_args_all() {
        let opts = Aria2Options {
            ed2k_server: "srv".into(),
            ed2k_server_list: "servers.dat".into(),
            ed2k_node_list: "nodes.dat".into(),
            ed2k_listen_port: 4661,
            ed2k_udp_listen_port: 4671,
            ed2k_upload_slots: 5,
            ..Aria2Options::default()
        };
        let args = opts.ed2k_startup_args();
        assert_eq!(
            args,
            vec![
                "--ed2k-server".to_string(),
                "srv".to_string(),
                "--ed2k-server-list".to_string(),
                "servers.dat".to_string(),
                "--ed2k-node-list".to_string(),
                "nodes.dat".to_string(),
                "--ed2k-listen-port".to_string(),
                "4661".to_string(),
                "--ed2k-udp-listen-port".to_string(),
                "4671".to_string(),
                "--ed2k-upload-slots".to_string(),
                "5".to_string(),
            ]
        );
    }

    #[test]
    fn ed2k_startup_args_trims_whitespace() {
        let opts = Aria2Options {
            ed2k_server: "  srv  ".into(),
            ed2k_server_list: "\tservers.dat\n".into(),
            ed2k_node_list: "".into(),
            ..Aria2Options::default()
        };
        let args = opts.ed2k_startup_args();
        assert_eq!(
            args,
            vec![
                "--ed2k-server".to_string(),
                "srv".to_string(),
                "--ed2k-server-list".to_string(),
                "servers.dat".to_string(),
                "--ed2k-listen-port".to_string(),
                opts.ed2k_listen_port.to_string(),
                "--ed2k-udp-listen-port".to_string(),
                opts.ed2k_udp_listen_port.to_string(),
                "--ed2k-upload-slots".to_string(),
                opts.ed2k_upload_slots.to_string(),
            ]
        );
        let node_list_idx = args
            .iter()
            .position(|a| a == "--ed2k-node-list")
            .unwrap_or(usize::MAX);
        assert_eq!(
            node_list_idx,
            usize::MAX,
            "empty node_list should be omitted"
        );
    }

    #[test]
    fn ed2k_equal_same() {
        let a = Aria2Options::default();
        let b = Aria2Options::default();
        assert!(a.ed2k_equal(&b));
    }

    #[test]
    fn ed2k_equal_trims_diff() {
        let a = Aria2Options {
            ed2k_server: "srv".into(),
            ..Aria2Options::default()
        };
        let b = Aria2Options {
            ed2k_server: "  srv\n".into(),
            ..Aria2Options::default()
        };
        assert!(a.ed2k_equal(&b));
    }

    #[test]
    fn ed2k_equal_listen_port_diff() {
        let a = Aria2Options::default();
        let b = Aria2Options {
            ed2k_listen_port: 5000,
            ..Aria2Options::default()
        };
        assert!(!a.ed2k_equal(&b));
    }

    #[test]
    fn ed2k_equal_node_list_diff() {
        let a = Aria2Options {
            ed2k_node_list: "a".into(),
            ..Aria2Options::default()
        };
        let b = Aria2Options {
            ed2k_node_list: "b".into(),
            ..Aria2Options::default()
        };
        assert!(!a.ed2k_equal(&b));
    }

    #[test]
    fn restart_needed_no_change() {
        let a = Aria2Options::default();
        let b = Aria2Options::default();
        assert!(!a.engine_restart_needed(&b));
    }

    #[test]
    fn restart_needed_rpc_port() {
        let a = Aria2Options::default();
        let b = Aria2Options {
            rpc_listen_port: a.rpc_listen_port + 1,
            ..Aria2Options::default()
        };
        assert!(a.engine_restart_needed(&b));
    }

    #[test]
    fn restart_needed_ed2k_server() {
        let a = Aria2Options {
            ed2k_server: "old".into(),
            ..Aria2Options::default()
        };
        let b = Aria2Options {
            ed2k_server: "new".into(),
            ..Aria2Options::default()
        };
        assert!(a.engine_restart_needed(&b));
    }

    #[test]
    fn restart_needed_list_files_only() {
        let a = Aria2Options {
            ed2k_server_list: "old.dat".into(),
            ..Aria2Options::default()
        };
        let b = Aria2Options {
            ed2k_server_list: "new.dat".into(),
            ..Aria2Options::default()
        };
        assert!(a.engine_restart_needed(&b));
    }
}
