use std::path::{Path, PathBuf};

use crate::config::aria2_bin_dir;
use crate::download::{self, DownloadOpts, ProgressFn};
use crate::engine::{EngineEvent, EventTx};
use crate::updater;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct InstalledInfo {
    version: String,
    slug: String,
    sha256: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PendingInfo {
    version: String,
    slug: String,
    sha256: String,
}

/// If `dir/.pending-update` points at a valid staged binary, promote it to
/// the active `aria2-next` install: remove the previous binary, write
/// `.installed`, and delete the marker. Returns `Some(version)` on success
/// or `None` when no pending update exists / is invalid.
///
/// # Errors
/// Returns an error only when the post-promotion `.installed` write fails;
/// every other failure mode (missing file, bad json, sha mismatch) is
/// treated as "no pending update" so the caller falls through to a fresh
/// download.
pub fn apply_pending_update(dir: &Path) -> Result<Option<String>, String> {
    let pending_path = dir.join(".pending-update");
    let Ok(content) = std::fs::read_to_string(&pending_path) else {
        return Ok(None);
    };
    let pending: PendingInfo = match serde_json::from_str(&content) {
        Ok(p) => p,
        Err(_) => {
            let _ = std::fs::remove_file(&pending_path);
            return Ok(None);
        }
    };
    let bin_name = format!("aria2-next-{}-{}", pending.version, pending.slug);
    let bin_path = dir.join(&bin_name);
    if !bin_path.exists() {
        let _ = std::fs::remove_file(&pending_path);
        return Ok(None);
    }
    match sha256_file(&bin_path) {
        Ok(digest) if digest == pending.sha256 => {}
        _ => {
            let _ = std::fs::remove_file(&bin_path);
            let _ = std::fs::remove_file(&pending_path);
            return Ok(None);
        }
    }
    if let Ok(Some(old)) = read_installed_opt(dir) {
        let old_name = format!("aria2-next-{}-{}", old.version, old.slug);
        let old_path = dir.join(&old_name);
        let _ = std::fs::remove_file(&old_path);
    }
    let installed = InstalledInfo {
        version: pending.version.clone(),
        slug: pending.slug.clone(),
        sha256: pending.sha256.clone(),
    };
    write_installed(dir, &installed)?;
    let _ = std::fs::remove_file(&pending_path);
    Ok(Some(pending.version))
}

/// Return the staged version string from `dir/.pending-update` if the
/// marker points at a valid, verified binary. Used by the update check
/// loop and the startup task to avoid re-offering downloads the user has
/// already pulled.
pub fn pending_update(dir: &Path) -> Option<String> {
    let pending_path = dir.join(".pending-update");
    let content = std::fs::read_to_string(&pending_path).ok()?;
    let pending: PendingInfo = serde_json::from_str(&content).ok()?;
    let bin_name = format!("aria2-next-{}-{}", pending.version, pending.slug);
    let bin_path = dir.join(&bin_name);
    if !bin_path.exists() {
        return None;
    }
    let digest = sha256_file(&bin_path).ok()?;
    if digest != pending.sha256 {
        return None;
    }
    Some(pending.version)
}

/// Ensure a working `aria2-next` binary is available, downloading from
/// GitHub Releases on first launch and reusing the cached version
/// thereafter. Progress and status are forwarded through `event_tx` so
/// the UI can show a status card during the initial fetch.
///
/// Honours the `ARIA2_BIN` env var as a fully-resolved override and
/// applies any staged `.pending-update` before consulting the cache.
///
/// # Errors
/// Returns a string error if the data directory cannot be resolved, the
/// GitHub request fails, or the downloaded binary's sha256 mismatches
/// the release manifest.
pub async fn ensure_aria2_next(
    event_tx: &EventTx,
    proxy: Option<String>,
) -> Result<(PathBuf, Option<String>), String> {
    if let Ok(bin) = std::env::var("ARIA2_BIN") {
        let path = PathBuf::from(&bin);
        if path.exists() {
            tracing::info!(?path, "using ARIA2_BIN env");
            return Ok((path, None));
        }
        return Err(format!("ARIA2_BIN={bin} does not exist"));
    }

    let dir = aria2_bin_dir().ok_or("cannot determine data directory")?;

    let slug = updater::platform_slug();
    let applied = apply_pending_update(&dir).unwrap_or(None);

    if let Some(installed) = read_installed(&dir) {
        let bin_name = format!("aria2-next-{}-{}", installed.version, installed.slug);
        let bin_path = dir.join(&bin_name);
        if bin_path.exists() {
            match sha256_file(&bin_path) {
                Ok(digest) if digest == installed.sha256 => {
                    tracing::info!(version = %installed.version, ?bin_path, "aria2-next cache hit");
                    return Ok((bin_path, applied));
                }
                Ok(digest) => {
                    tracing::warn!(expected = %installed.sha256, got = %digest, "sha256 mismatch, re-downloading");
                }
                Err(e) => {
                    tracing::warn!("sha256 read error: {e}, re-downloading");
                }
            }
        }
    }

    if let Some((bin_path, version)) = scan_for_binary(&dir, slug) {
        tracing::info!(%version, ?bin_path, "aria2-next found via directory scan, self-healing .installed");
        self_heal_installed(&dir, &bin_path, &version, slug)?;
        set_perms(&bin_path)?;
        emit_status(event_tx, "ready", &format!("aria2-next {version} ready"));
        return Ok((bin_path, applied));
    }

    emit_status(event_tx, "downloading", "Looking up latest release...");

    let release = updater::fetch_latest_release(
        "AnInsomniacy/aria2-next",
        "aria2-next",
        slug,
        true,
        proxy.clone(),
        false,
    )
    .await?;
    let bin_name = format!("aria2-next-{}-{}", release.version, slug);
    let bin_path = dir.join(&bin_name);

    emit_status(
        event_tx,
        "downloading",
        &format!("Downloading aria2-next {}...", release.version),
    );

    download_verified(
        &release.download_url,
        &bin_path,
        release.sha256.as_deref(),
        proxy.as_deref(),
        None,
    )
    .await?;

    let installed = InstalledInfo {
        version: release.version.clone(),
        slug: slug.to_string(),
        sha256: release.sha256.unwrap_or_default(),
    };
    write_installed(&dir, &installed)?;

    emit_status(
        event_tx,
        "ready",
        &format!("aria2-next {} ready", release.version),
    );

    Ok((bin_path, applied))
}

/// Write the `.pending-update` marker for a staged aria2-next binary that has
/// already been downloaded (and verified) to `dir`. The engine applies it on
/// next restart.
/// Write a `.pending-update` marker next to a newly downloaded (and
/// verified) aria2-next binary so the engine promotes it to the active
/// install on next restart.
///
/// # Errors
/// Returns an error if JSON serialisation or writing the marker file
/// fails.
pub fn stage_pending(
    dir: &Path,
    version: &str,
    slug: &str,
    sha256: Option<&str>,
) -> Result<(), String> {
    let pending = PendingInfo {
        version: version.to_string(),
        slug: slug.to_string(),
        sha256: sha256.unwrap_or("").to_string(),
    };
    let json =
        serde_json::to_string_pretty(&pending).map_err(|e| format!("serialize pending: {e}"))?;
    std::fs::write(dir.join(".pending-update"), &json)
        .map_err(|e| format!("write .pending-update: {e}"))
}

/// Return the version of the locally installed aria2-next binary, or
/// `None` if none is present. Used by the about dialog and to decide
/// whether the update notification should mention a pending upgrade.
pub fn installed_version() -> Option<String> {
    let dir = aria2_bin_dir()?;
    if let Some(info) = read_installed(&dir) {
        return Some(info.version);
    }
    let slug = updater::platform_slug();
    scan_for_binary(&dir, slug).map(|(_, v)| v)
}

fn read_installed(dir: &Path) -> Option<InstalledInfo> {
    read_installed_opt(dir).ok().flatten()
}

fn read_installed_opt(dir: &Path) -> Result<Option<InstalledInfo>, String> {
    let path = dir.join(".installed");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|e| format!("parse .installed: {e}"))
}

fn write_installed(dir: &Path, info: &InstalledInfo) -> Result<(), String> {
    let path = dir.join(".installed");
    let json =
        serde_json::to_string_pretty(info).map_err(|e| format!("serialize installed: {e}"))?;
    std::fs::write(&path, &json).map_err(|e| format!("write .installed: {e}"))
}

pub(crate) use crate::download::{set_perms, sha256_file};

pub(crate) fn parse_version_from_filename(filename: &str, slug: &str) -> Option<String> {
    let prefix = "aria2-next-";
    let suffix = format!("-{slug}");
    let rest = filename.strip_prefix(prefix)?;
    let version = rest.strip_suffix(&suffix)?;
    if version.is_empty() {
        return None;
    }
    if updater::version_tuple(version).is_empty() {
        return None;
    }
    Some(version.to_string())
}

fn scan_for_binary(dir: &Path, slug: &str) -> Option<(PathBuf, String)> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(PathBuf, String, Vec<u64>)> = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.starts_with('.') || name_str.ends_with(".part") || name_str == "session.txt" {
            continue;
        }
        let Some(version) = parse_version_from_filename(name_str, slug) else {
            continue;
        };
        let path = entry.path();
        #[cfg(unix)]
        {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
        }
        #[cfg(not(unix))]
        {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
        }
        let tuple = updater::version_tuple(&version);
        if best.as_ref().is_none_or(|(_, _, bt)| tuple > *bt) {
            best = Some((path, version, tuple));
        }
    }
    best.map(|(p, v, _)| (p, v))
}

fn self_heal_installed(
    dir: &Path,
    bin_path: &Path,
    version: &str,
    slug: &str,
) -> Result<(), String> {
    let sha256 = sha256_file(bin_path)?;
    let info = InstalledInfo {
        version: version.to_string(),
        slug: slug.to_string(),
        sha256,
    };
    write_installed(dir, &info)
}

pub(crate) async fn download_verified(
    url: &str,
    dest: &Path,
    sha256: Option<&str>,
    proxy: Option<&str>,
    on_progress: Option<&ProgressFn>,
) -> Result<(), String> {
    let opts = DownloadOpts {
        proxy: proxy.map(str::to_string),
        sha256: sha256.map(str::to_string),
        on_progress: on_progress.cloned(),
    };
    download::download(url, dest, &opts).await
}

fn emit_status(event_tx: &EventTx, stage: &str, message: &str) {
    let _ = event_tx.send(EngineEvent::Aria2Status {
        stage: stage.to_string(),
        message: message.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_from_filename_valid() {
        let slug = "linux-x86_64";
        assert_eq!(
            parse_version_from_filename("aria2-next-1.2.3-linux-x86_64", slug),
            Some("1.2.3".to_string()),
        );
        assert_eq!(
            parse_version_from_filename("aria2-next-0.1.0-linux-x86_64", slug),
            Some("0.1.0".to_string()),
        );
    }

    #[test]
    fn parse_version_from_filename_no_prefix() {
        assert_eq!(
            parse_version_from_filename("aria2c-1.2.3-linux-x86_64", "linux-x86_64"),
            None
        );
    }

    #[test]
    fn parse_version_from_filename_slug_mismatch() {
        assert_eq!(
            parse_version_from_filename("aria2-next-1.2.3-windows-x86_64", "linux-x86_64"),
            None,
        );
    }

    #[test]
    fn parse_version_from_filename_no_version() {
        // Empty version string is rejected.
        assert_eq!(
            parse_version_from_filename("aria2-next--linux-x86_64", "linux-x86_64"),
            None,
        );
    }

    #[test]
    fn parse_version_from_filename_non_numeric_version() {
        // The version_tuple guard rejects non-numeric tokens.
        assert_eq!(
            parse_version_from_filename("aria2-next-abc-linux-x86_64", "linux-x86_64"),
            None,
        );
    }
}
