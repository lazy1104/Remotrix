use std::path::Path;
use std::time::Duration;

use super::{EngineCmd, EngineEvent, EventTx};

/// Reduce an externally-supplied value to a single safe path component,
/// rejecting anything that is empty, `.`, `..`, or contains path separators.
fn sanitize_component(input: &str, what: &str) -> Result<String, String> {
    let base = std::path::Path::new(input)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if base.is_empty() || base == "." || base == ".." {
        return Err(format!("invalid {what}: {input:?}"));
    }
    Ok(base.to_string())
}

/// Return `true` when `gid` is non-empty and contains only ASCII hex
/// digits. Used to validate aria2-supplied gids before joining them into
/// a filesystem path (e.g. an ED2K search temp dir).
pub(crate) fn is_safe_gid(gid: &str) -> bool {
    !gid.is_empty() && gid.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Resolve the platform slug for an aria2-next update asset, stripping any
/// path components from the externally-supplied `asset_name` so it cannot
/// escape the aria2 data directory. Falls back to the platform slug.
fn resolve_aria2_slug(version: &str, asset_name: &str) -> Result<String, String> {
    let slug = asset_name
        .strip_prefix(&format!("aria2-next-{version}-"))
        .unwrap_or(crate::updater::platform_slug());
    sanitize_component(slug, "aria2 update asset name")
}

/// Resolve the update asset's sha256, short-circuiting on an already-known
/// value and otherwise fetching it from the release's checksums.
async fn resolve_sha256(
    repo: &str,
    version: &str,
    asset_name: &str,
    proxy: Option<String>,
    existing: Option<String>,
) -> Option<String> {
    if let Some(e) = existing {
        Some(e)
    } else {
        crate::updater::fetch_asset_checksum(repo, version, asset_name, proxy).await
    }
}

pub(crate) async fn handle_download_aria2_update(cmd: EngineCmd, event_tx: &EventTx) {
    let EngineCmd::DownloadAria2Update {
        version,
        asset_name,
        download_url,
        sha256,
    } = cmd
    else {
        return;
    };
    tracing::info!(?version, "download aria2 update via engine");
    let proxy = crate::config::load().aria2.all_proxy_value();
    let version = match sanitize_component(&version, "aria2 update version") {
        Ok(v) => v,
        Err(e) => {
            let _ = event_tx.send(EngineEvent::Aria2UpdateFailed { error: e });
            return;
        }
    };
    let slug = match resolve_aria2_slug(&version, &asset_name) {
        Ok(s) => s,
        Err(e) => {
            let _ = event_tx.send(EngineEvent::Aria2UpdateFailed { error: e });
            return;
        }
    };
    let sha256 = resolve_sha256(
        "AnInsomniacy/aria2-next",
        &version,
        &asset_name,
        proxy.clone(),
        sha256,
    )
    .await;
    let dir = match crate::config::aria2_bin_dir() {
        Some(d) => d,
        None => {
            let _ = event_tx.send(EngineEvent::Aria2UpdateFailed {
                error: "cannot determine data directory".to_string(),
            });
            return;
        }
    };
    let dest = dir.join(format!("aria2-next-{version}-{slug}"));
    let tx = event_tx.clone();
    let prog: crate::download::ProgressFn = std::sync::Arc::new(move |downloaded, total| {
        let _ = tx.send(EngineEvent::Aria2UpdateProgress { downloaded, total });
    });
    match crate::aria2_fetcher::download_verified(
        &download_url,
        &dest,
        sha256.as_deref(),
        proxy.as_deref(),
        Some(&prog),
    )
    .await
    {
        Ok(()) => {
            match crate::aria2_fetcher::stage_pending(&dir, &version, &slug, sha256.as_deref()) {
                Ok(()) => {
                    let _ = event_tx.send(EngineEvent::Aria2UpdateStaged { version });
                }
                Err(e) => {
                    let _ = event_tx.send(EngineEvent::Aria2UpdateFailed { error: e });
                }
            }
        }
        Err(e) => {
            let _ = event_tx.send(EngineEvent::Aria2UpdateFailed { error: e });
        }
    }
}

#[cfg(not(unix))]
pub(crate) async fn cleanup_stale_aria2(_bin_path: &Path, _pid_path: &Path) {}

#[cfg(not(unix))]
pub(crate) fn kill_sidecar_by_pid(_pid_path: &Path) {}

#[cfg(unix)]
pub(crate) fn kill_sidecar_by_pid(pid_path: &Path) {
    let Ok(content) = std::fs::read_to_string(pid_path) else {
        return;
    };
    let Ok(pid) = content.trim().parse::<i32>() else {
        return;
    };
    tracing::warn!(%pid, "SIGKILL aria2-next by pid file");
    unsafe { libc::kill(pid, libc::SIGKILL) };
}

#[cfg(unix)]
pub(crate) async fn cleanup_stale_aria2(bin_path: &Path, pid_path: &Path) {
    let Ok(content) = std::fs::read_to_string(pid_path) else {
        return;
    };
    let Ok(pid) = content.trim().parse::<i32>() else {
        let _ = std::fs::remove_file(pid_path);
        return;
    };
    let alive = std::path::Path::new(&format!("/proc/{pid}")).exists();
    let is_ours = std::fs::read_link(format!("/proc/{pid}/exe"))
        .map(|p| p == bin_path)
        .unwrap_or(false);
    if alive && is_ours {
        tracing::warn!(%pid, "stale aria2-next from previous run detected, SIGTERM");
        unsafe { libc::kill(pid, libc::SIGTERM) };
        let mut waited = 0;
        while std::path::Path::new(&format!("/proc/{pid}")).exists() && waited < 50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            waited += 1;
        }
        let still_ours = std::path::Path::new(&format!("/proc/{pid}")).exists()
            && std::fs::read_link(format!("/proc/{pid}/exe"))
                .map(|p| p == bin_path)
                .unwrap_or(false);
        if still_ours {
            tracing::warn!(%pid, "stale aria2-next still alive, SIGKILL");
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    }
    let _ = std::fs::remove_file(pid_path);
}
