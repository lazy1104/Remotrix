use std::path::{Path, PathBuf};

use super::settings::{PathOverrides, ResolvedPaths, Settings};

pub(crate) fn default_app_data_dir() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
    Some(proj.data_dir().to_path_buf())
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

/// Compute the currently effective paths honouring user overrides. Reads
/// `Settings::paths` from disk on every call so UI-driven edits (which
/// live only in the in-memory copy held by the UI state) are picked up
/// once `config::save` has persisted them. The resolver call sites are
/// infrequent (engine spawn, db open, log init), so a single `load()` per
/// call is acceptable.
///
/// The default `aria2` and `logs` locations nest under the effective app
/// data dir, so overriding only the app data dir re-homes them too. Once
/// an explicit override is provided for a sub-directory it wins.
fn resolved_paths() -> ResolvedPaths {
    let s = super::settings::load();
    resolve_from_override(&s.paths)
}

fn aria2_bin_dir_internal() -> Option<PathBuf> {
    let p = resolved_paths().aria2_bin_dir;
    let _ = std::fs::create_dir_all(&p);
    Some(p)
}

fn app_data_dir_internal() -> Option<PathBuf> {
    let p = resolved_paths().app_data_dir;
    let _ = std::fs::create_dir_all(&p);
    Some(p)
}

fn log_dir_internal() -> Option<PathBuf> {
    let p = resolved_paths().log_dir;
    let _ = std::fs::create_dir_all(&p);
    Some(p)
}

/// Return the per-user log directory, creating it if missing. Used by the
/// tracing-appender rotating log writer.
pub fn log_dir() -> Option<PathBuf> {
    log_dir_internal()
}

/// Return the absolute path of the SQLite database file used by
/// [`crate::db`]. The file itself is not created here — `db::open` handles
/// that — only the parent directory is implied by the per-user data dir.
pub fn db_path() -> Option<PathBuf> {
    app_data_dir_internal().map(|d| d.join("remotrix.db"))
}

/// Directory used by aria2's `--save-session`/`--input-file` to persist
/// tasks across restarts. Coincides with [`aria2_bin_dir`].
pub fn session_dir() -> Option<PathBuf> {
    aria2_bin_dir_internal()
}

/// Directory under which [`crate::aria2_fetcher`] stores the aria2-next
/// binary and its `.installed` / `.pending-update` markers.
pub fn aria2_bin_dir() -> Option<PathBuf> {
    aria2_bin_dir_internal()
}

/// Emit `tracing::info!` lines for every on-disk path the app depends on
/// (config, logs, aria2 dir, …). Intended to be called once at startup so
/// log readers can locate the data without grepping the source.
pub fn announce() {
    if let Some(p) = super::settings::config_path() {
        tracing::info!(?p, "config path");
    }
    if let Some(p) = log_dir() {
        tracing::info!(?p, "log dir");
    }
    if let Some(p) = crate::logging::engine_log_path() {
        tracing::info!(?p, "engine log path");
    }
    if let Some(p) = aria2_bin_dir() {
        tracing::info!(?p, "aria2 dir");
    }
}

/// Resolve the currently effective paths honouring user overrides. Public
/// alias used by callers that already hold a `Settings` value (e.g.
/// `migrate_paths` during boot) and want to compute the effective set
/// without re-reading from disk.
pub(crate) fn resolve_from_settings(settings: &Settings) -> ResolvedPaths {
    resolve_from_override(&settings.paths)
}

/// Shared resolver: app data dir is the override (or the default), and the
/// `aria2`/`logs` sub-directories default to being nested under it unless
/// overridden independently.
fn resolve_from_override(paths: &PathOverrides) -> ResolvedPaths {
    let app_data_dir = paths
        .app_data_dir
        .clone()
        .or_else(default_app_data_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    let aria2_bin_dir = match paths.aria2_bin_dir.clone() {
        Some(d) => d,
        None => app_data_dir.join("aria2"),
    };
    let log_dir = match paths.log_dir.clone() {
        Some(d) => d,
        None => app_data_dir.join("logs"),
    };
    ResolvedPaths {
        aria2_bin_dir,
        app_data_dir,
        log_dir,
    }
}

pub(crate) fn copy_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src, dst)?;
    set_file_perms_if_unix(dst);
    Ok(())
}

fn set_file_perms_if_unix(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&from, &to)?;
        } else if ty.is_symlink() || to.exists() {
            continue;
        } else {
            copy_file(&from, &to)?;
        }
    }
    Ok(())
}

pub(crate) fn copy_dir_contents(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    copy_dir(src, dst)
}
