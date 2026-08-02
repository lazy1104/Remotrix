use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;
use tracing_subscriber::reload;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub const DEFAULT_APP_LEVEL: &str = "warn";
pub const DEFAULT_ENGINE_LEVEL: &str = "warn";
pub const APP_LOG_FILENAME: &str = "remotrix.log";
pub const ENGINE_LOG_FILENAME: &str = "aria2.log";

static APP_FILTER: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

fn build_env_filter(level: &str) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(normalize_app_level(level)))
}

pub fn init() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::fmt;

    let cfg = crate::config::load();
    let level = normalize_app_level(&cfg.log.app_level);
    let (filter_layer, filter_handle) = reload::Layer::new(build_env_filter(&level));
    let _ = APP_FILTER.set(filter_handle);

    match crate::config::log_dir() {
        Some(dir) => {
            let writer = DailyRollingWriter::new(dir, APP_LOG_FILENAME);
            let (non_blocking, guard) = tracing_appender::non_blocking(writer);
            let _ = tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt::layer().with_target(false))
                .with(
                    fmt::layer()
                        .with_ansi(false)
                        .with_target(true)
                        .with_writer(non_blocking),
                )
                .try_init();
            tracing::info!(app_level = %level, "remotrix logging initialized");
            Some(guard)
        }
        None => {
            let _ = tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt::layer().with_target(false))
                .try_init();
            tracing::warn!("log file disabled: log_dir unavailable");
            None
        }
    }
}

pub fn set_app_level(level: &str) {
    if let Some(handle) = APP_FILTER.get() {
        let level = normalize_app_level(level);
        let _ = handle.reload(build_env_filter(&level));
        tracing::info!(app_level = %level, "app log level reloaded");
    }
}

pub fn engine_log_path() -> Option<PathBuf> {
    crate::config::log_dir().map(|d| d.join(ENGINE_LOG_FILENAME))
}

pub fn clear_logs() -> Result<usize, String> {
    let Some(dir) = crate::config::log_dir() else {
        return Ok(0);
    };
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("read log dir: {e}"))?;
    let mut cleared = 0usize;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(APP_LOG_FILENAME) || name.starts_with(ENGINE_LOG_FILENAME) {
            std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(entry.path())
                .map_err(|e| format!("truncate {name}: {e}"))?;
            cleared += 1;
        }
    }
    Ok(cleared)
}

pub fn app_level_options() -> &'static [&'static str] {
    &["error", "warn", "info", "debug", "trace"]
}

pub fn engine_level_options() -> &'static [&'static str] {
    &["error", "warn", "notice", "info", "debug"]
}

pub fn normalize_app_level(level: &str) -> String {
    let lower = level.trim().to_ascii_lowercase();
    if app_level_options().contains(&lower.as_str()) {
        lower
    } else {
        DEFAULT_APP_LEVEL.to_string()
    }
}

pub fn normalize_engine_level(level: &str) -> String {
    let lower = level.trim().to_ascii_lowercase();
    if engine_level_options().contains(&lower.as_str()) {
        lower
    } else {
        DEFAULT_ENGINE_LEVEL.to_string()
    }
}

pub struct DailyRollingWriter {
    dir: PathBuf,
    prefix: &'static str,
    current_date: Option<String>,
    file: Option<File>,
}

impl DailyRollingWriter {
    pub fn new(dir: PathBuf, prefix: &'static str) -> Self {
        Self {
            dir,
            prefix,
            current_date: None,
            file: None,
        }
    }
}

impl Write for DailyRollingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        if self.current_date.as_deref() != Some(date.as_str()) {
            self.current_date = Some(date.clone());
            self.file = None;
        }
        if self.file.is_none() {
            let path = self.dir.join(format!("{}.{}", self.prefix, date));
            self.file = Some(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?,
            );
        }
        let Some(file) = self.file.as_mut() else {
            return Err(std::io::Error::other("log file unavailable"));
        };
        file.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(file) = self.file.as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}
