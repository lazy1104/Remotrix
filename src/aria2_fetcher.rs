use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::config::aria2_bin_dir;
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

fn parse_version_from_filename(filename: &str, slug: &str) -> Option<String> {
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

pub(crate) fn sha256_file(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| format!("open file for sha256: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read file for sha256: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub(crate) async fn download_file(
    url: &str,
    dest: &Path,
    proxy: Option<&str>,
) -> Result<(), String> {
    use std::io::Write;

    let builder = crate::config::apply_proxy(
        reqwest::Client::builder().user_agent("remotrix-updater"),
        proxy,
    )?;
    let client = builder
        .build()
        .map_err(|e| format!("create download client: {e}"))?;

    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download request: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }

    let parent = dest.parent().unwrap();
    std::fs::create_dir_all(parent).map_err(|e| format!("create download dir: {e}"))?;

    let mut file = std::fs::File::create(dest).map_err(|e| format!("create {dest:?}: {e}"))?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("read body: {e}"))?
    {
        file.write_all(&chunk)
            .map_err(|e| format!("write to {dest:?}: {e}"))?;
    }
    Ok(())
}

/// Download `url` to `dest` via a sibling `{dest}.part`, optionally verifying
/// an sha256, then atomically rename and set exec permissions. Shared by the
/// aria2-next fetch/update paths and app package downloads.
pub(crate) async fn download_verified(
    url: &str,
    dest: &Path,
    sha256: Option<&str>,
    proxy: Option<&str>,
) -> Result<(), String> {
    let part = std::path::PathBuf::from(format!("{}.part", dest.display()));
    download_file(url, &part, proxy).await?;

    if let Some(expected) = sha256 {
        let part_clone = part.clone();
        let digest = tokio::task::spawn_blocking(move || sha256_file(&part_clone))
            .await
            .map_err(|e| format!("sha256 task: {e}"))?
            .map_err(|e| format!("sha256: {e}"))?;
        if digest != expected {
            let _ = std::fs::remove_file(&part);
            return Err(format!(
                "sha256 mismatch: expected {expected}, got {digest}"
            ));
        }
    }

    std::fs::rename(&part, dest).map_err(|e| format!("rename: {e}"))?;
    set_perms(dest)?;
    Ok(())
}

pub(crate) fn set_perms(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("chmod: {e}"))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn emit_status(event_tx: &EventTx, stage: &str, message: &str) {
    let _ = event_tx.send(EngineEvent::Aria2Status {
        stage: stage.to_string(),
        message: message.to_string(),
    });
}
