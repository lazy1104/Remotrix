use std::path::Path;
use std::time::Duration;

use aria2_ws::response::TaskStatus as Aria2TaskStatus;
use aria2_ws::Client;
use tokio::time::timeout;

use super::{
    EngineCmd, EngineEvent, EventTx, RPC_TIMEOUT, UPDATE_DOWNLOAD_MAX_POLL_FAILURES,
    UPDATE_DOWNLOAD_MAX_WAIT, UPDATE_DOWNLOAD_POLL_INTERVAL,
};

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

pub(crate) async fn download_via_engine(
    client: &Client,
    url: &str,
    dest: &Path,
    sha256: Option<&str>,
) -> Result<(), String> {
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("invalid download destination parent")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("create download dir: {e}"))?;

    let filename = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let _ = std::fs::remove_file(dest);
    let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));

    let mut extra = serde_json::Map::new();
    extra.insert(
        "allow-overwrite".to_string(),
        serde_json::Value::String("false".to_string()),
    );
    let opts = aria2_ws::TaskOptions {
        dir: Some(parent.to_string_lossy().into_owned()),
        out: Some(filename.to_string()),
        split: Some(8),
        max_connection_per_server: Some(8),
        r#continue: Some(true),
        auto_file_renaming: Some(false),
        extra_options: extra,
        ..Default::default()
    };
    let gid = timeout(
        RPC_TIMEOUT,
        client.add_uri(vec![url.to_string()], Some(opts), None, None),
    )
    .await
    .map_err(|_| "update add_uri timed out".to_string())?
    .map_err(|e| format!("update add_uri: {e}"))?;

    let mut last_progress = tokio::time::Instant::now();
    let mut last_completed: u64 = 0;
    let mut consecutive_failures: u32 = 0;
    loop {
        let status = match timeout(RPC_TIMEOUT, client.tell_status(&gid)).await {
            Ok(Ok(s)) => {
                consecutive_failures = 0;
                s
            }
            Ok(Err(e)) => {
                tracing::warn!(?gid, error = ?e, "update tell_status failed");
                consecutive_failures += 1;
                if consecutive_failures >= UPDATE_DOWNLOAD_MAX_POLL_FAILURES {
                    let _ = client.force_remove(&gid).await;
                    let _ = client.remove_download_result(&gid).await;
                    let _ = client.save_session().await;
                    let _ = std::fs::remove_file(dest);
                    let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
                    return Err("update download connection lost".to_string());
                }
                tokio::time::sleep(UPDATE_DOWNLOAD_POLL_INTERVAL).await;
                continue;
            }
            Err(_) => {
                tracing::warn!(?gid, "update tell_status timed out");
                consecutive_failures += 1;
                if consecutive_failures >= UPDATE_DOWNLOAD_MAX_POLL_FAILURES {
                    let _ = client.force_remove(&gid).await;
                    let _ = client.remove_download_result(&gid).await;
                    let _ = client.save_session().await;
                    let _ = std::fs::remove_file(dest);
                    let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
                    return Err("update download connection lost".to_string());
                }
                tokio::time::sleep(UPDATE_DOWNLOAD_POLL_INTERVAL).await;
                continue;
            }
        };
        match status.status {
            Aria2TaskStatus::Complete => break,
            Aria2TaskStatus::Removed => {
                let _ = client.remove_download_result(&gid).await;
                let _ = client.save_session().await;
                let _ = std::fs::remove_file(dest);
                let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
                return Err("update download removed".to_string());
            }
            Aria2TaskStatus::Error => {
                let msg = status
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "unknown error".to_string());
                let _ = client.remove_download_result(&gid).await;
                let _ = client.save_session().await;
                let _ = std::fs::remove_file(dest);
                let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
                return Err(format!("update download failed: {msg}"));
            }
            Aria2TaskStatus::Paused | Aria2TaskStatus::Waiting => {
                consecutive_failures = 0;
                last_progress = tokio::time::Instant::now();
                tokio::time::sleep(UPDATE_DOWNLOAD_POLL_INTERVAL).await;
            }
            _ => {
                let making_progress =
                    status.completed_length > last_completed || status.download_speed > 0;
                if making_progress {
                    last_progress = tokio::time::Instant::now();
                    last_completed = status.completed_length;
                }
                if !making_progress && last_progress.elapsed() >= UPDATE_DOWNLOAD_MAX_WAIT {
                    let _ = client.force_remove(&gid).await;
                    let _ = client.remove_download_result(&gid).await;
                    let _ = client.save_session().await;
                    let _ = std::fs::remove_file(dest);
                    let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
                    return Err("update download stalled".to_string());
                }
                tokio::time::sleep(UPDATE_DOWNLOAD_POLL_INTERVAL).await;
            }
        }
    }

    if let Some(expected) = sha256 {
        let dest_clone = dest.to_path_buf();
        let digest =
            tokio::task::spawn_blocking(move || crate::aria2_fetcher::sha256_file(&dest_clone))
                .await
                .map_err(|e| format!("sha256 task: {e}"))?
                .map_err(|e| format!("sha256: {e}"))?;
        if digest != expected {
            let _ = std::fs::remove_file(dest);
            let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
            let _ = client.remove_download_result(&gid).await;
            let _ = client.save_session().await;
            return Err(format!(
                "sha256 mismatch: expected {expected}, got {digest}"
            ));
        }
    }

    crate::aria2_fetcher::set_perms(dest)?;
    let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
    let _ = client.remove_download_result(&gid).await;
    let _ = client.save_session().await;
    Ok(())
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
    let prog: crate::aria2_fetcher::ProgressFn = Box::new(move |downloaded, total| {
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

pub(crate) async fn handle_download_app_update_via_engine(
    client: &Client,
    cmd: EngineCmd,
    event_tx: &EventTx,
) {
    let EngineCmd::DownloadAppUpdate {
        kind,
        version,
        url,
        asset_name,
        sha256,
        download_dir,
    } = cmd
    else {
        return;
    };
    tracing::info!(?version, "download app update via engine");
    let proxy = crate::config::load().aria2.all_proxy_value();
    let sha256 = resolve_sha256(
        crate::updater::APP_REPO,
        &version,
        &asset_name,
        proxy.clone(),
        sha256,
    )
    .await;
    let dest = match crate::app_updater::app_update_dest(kind, &asset_name, Some(&download_dir)) {
        Ok(d) => d,
        Err(e) => {
            let _ = event_tx.send(EngineEvent::AppUpdateDownloadFailed { error: e });
            return;
        }
    };
    match download_via_engine(client, &url, &dest, sha256.as_deref()).await {
        Ok(()) => match crate::app_updater::apply_after_download(kind, &dest) {
            Ok(outcome) => {
                let _ = event_tx.send(EngineEvent::AppUpdateDownloaded {
                    kind: outcome.kind,
                    path: outcome.path,
                });
            }
            Err(e) => {
                let _ = event_tx.send(EngineEvent::AppUpdateDownloadFailed { error: e });
            }
        },
        Err(e) => {
            let _ = event_tx.send(EngineEvent::AppUpdateDownloadFailed { error: e });
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
