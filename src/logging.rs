use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;
use tracing_subscriber::reload;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Fallback `tracing` level used when an invalid app log level is supplied.
pub const DEFAULT_APP_LEVEL: &str = "warn";
/// Fallback level used when an invalid engine log level is supplied.
pub const DEFAULT_ENGINE_LEVEL: &str = "warn";
/// Filename prefix for the rotating app log (`remotrix.<date>.log`).
pub const APP_LOG_FILENAME: &str = "remotrix.log";
/// Filename prefix for the engine (aria2) log (`aria2.<date>.log`).
pub const ENGINE_LOG_FILENAME: &str = "aria2.log";

static APP_FILTER: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

fn build_env_filter(level: &str) -> EnvFilter {
    let app = normalize_app_level(level);
    EnvFilter::new(format!("remotrix={app},error"))
}

/// Initialise the global `tracing` subscriber, writing to a daily-rolled
/// file under [`crate::config::log_dir`] and a console layer. Returns the
/// background writer guard, which must be kept alive for the lifetime of
/// the app to avoid dropping log lines.
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

/// Reload the app `tracing` filter at runtime. Safe to call before
/// [`init`] — the request is silently dropped in that case.
pub fn set_app_level(level: &str) {
    if let Some(handle) = APP_FILTER.get() {
        let level = normalize_app_level(level);
        let _ = handle.reload(build_env_filter(&level));
        tracing::info!(app_level = %level, "app log level reloaded");
    }
}

/// Absolute path of the engine (aria2) log file inside the log directory,
/// or `None` if the log directory is not resolvable.
pub fn engine_log_path() -> Option<PathBuf> {
    crate::config::log_dir().map(|d| d.join(ENGINE_LOG_FILENAME))
}

/// Truncate every app/engine log file under the log directory in-place.
///
/// Returns the number of files that were truncated; when the log
/// directory is not resolvable, returns `Ok(0)` rather than an error.
///
/// # Errors
/// Returns the underlying I/O error when reading the directory or
/// truncating an individual file fails.
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

/// Ordered list of legal `tracing` level names exposed in the settings
/// UI for the app log.
pub fn app_level_options() -> &'static [&'static str] {
    &["error", "warn", "info", "debug", "trace"]
}

/// Ordered list of legal aria2 log levels exposed in the settings UI.
pub fn engine_level_options() -> &'static [&'static str] {
    &["error", "warn", "notice", "info", "debug"]
}

/// Normalise a user-supplied app log level: lower-cases, trims, and
/// falls back to [`DEFAULT_APP_LEVEL`] for any value not in
/// [`app_level_options`].
pub fn normalize_app_level(level: &str) -> String {
    let lower = level.trim().to_ascii_lowercase();
    if app_level_options().contains(&lower.as_str()) {
        lower
    } else {
        DEFAULT_APP_LEVEL.to_string()
    }
}

/// Normalise a user-supplied engine (aria2) log level: lower-cases,
/// trims, and falls back to [`DEFAULT_ENGINE_LEVEL`] for any value not
/// in [`engine_level_options`].
pub fn normalize_engine_level(level: &str) -> String {
    let lower = level.trim().to_ascii_lowercase();
    if engine_level_options().contains(&lower.as_str()) {
        lower
    } else {
        DEFAULT_ENGINE_LEVEL.to_string()
    }
}

/// `Write` adapter that opens a new file per local-date under `dir`,
/// named `"{prefix}.YYYY-MM-DD"`. Cheap to construct, thread-unsafe by
/// design (single-writer per instance).
pub struct DailyRollingWriter {
    dir: PathBuf,
    prefix: &'static str,
    current_date: Option<String>,
    file: Option<File>,
}

impl DailyRollingWriter {
    /// Build a writer that opens `{dir}/{prefix}.YYYY-MM-DD` on the first
    /// write of each local date. The directory must already exist.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_app_level_valid() {
        assert_eq!(normalize_app_level("error"), "error");
        assert_eq!(normalize_app_level("INFO"), "info");
        assert_eq!(normalize_app_level("  debug  "), "debug");
        assert_eq!(normalize_app_level("trace"), "trace");
    }

    #[test]
    fn normalize_app_level_unknown_falls_back() {
        assert_eq!(normalize_app_level("verbose"), DEFAULT_APP_LEVEL);
        assert_eq!(normalize_app_level(""), DEFAULT_APP_LEVEL);
        assert_eq!(normalize_app_level("notice"), DEFAULT_APP_LEVEL);
    }

    #[test]
    fn normalize_engine_level_valid() {
        assert_eq!(normalize_engine_level("error"), "error");
        assert_eq!(normalize_engine_level("Notice"), "notice");
        assert_eq!(normalize_engine_level("  debug  "), "debug");
    }

    #[test]
    fn normalize_engine_level_unknown_falls_back() {
        assert_eq!(normalize_engine_level("verbose"), DEFAULT_ENGINE_LEVEL);
        assert_eq!(normalize_engine_level("trace"), DEFAULT_ENGINE_LEVEL);
    }

    #[test]
    fn app_level_options_contains_defaults() {
        let opts = app_level_options();
        assert!(opts.contains(&"info"));
        assert!(opts.contains(&"warn"));
        assert!(!opts.contains(&"notice")); // notice is engine-only
    }

    #[test]
    fn engine_level_options_contains_notice() {
        let opts = engine_level_options();
        assert!(opts.contains(&"notice"));
    }
}
