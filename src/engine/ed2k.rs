use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use aria2_ws::Client;
use tokio::time::timeout;

use super::{is_safe_gid, EngineEvent, EventTx, RPC_TIMEOUT};

/// Root directory for per-search ED2K temp directories.
fn ed2k_search_root() -> Option<PathBuf> {
    crate::config::db_path().and_then(|p| p.parent().map(|d| d.join("ed2k-search")))
}

/// Per-search temp directory used by an aria2 `ed2kSearch` group.
fn ed2k_search_temp_dir() -> Result<PathBuf, String> {
    let root = ed2k_search_root().ok_or_else(|| "no data dir".to_string())?;
    std::fs::create_dir_all(&root).map_err(|e| format!("create ed2k-search root: {e}"))?;
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = root.join(format!("remotrix-ed2k-{pid}-{nanos:x}"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create ed2k temp dir: {e}"))?;
    Ok(dir)
}

/// Remove every stale `remotrix-ed2k-*` directory left in the root by
/// earlier crashed runs. Called at boot so a hard exit doesn't leak temp
/// directories across sessions.
pub(crate) fn cleanup_stale_ed2k_search_dirs() {
    let Some(root) = ed2k_search_root() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("remotrix-ed2k-") {
            if let Err(e) = std::fs::remove_dir_all(&path) {
                tracing::debug!(?path, error = %e, "ed2k stale dir cleanup skipped");
            }
        }
    }
}

/// Best-effort cleanup of an ED2K search temp directory.
fn cleanup_ed2k_search_dir(path: &Path) {
    match std::fs::remove_dir_all(path) {
        Ok(()) => tracing::debug!(?path, "ed2k search temp dir removed"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::debug!(?path, error = %e, "ed2k search temp dir cleanup skipped"),
    }
}

pub(crate) async fn ed2k_search_start(
    client: &Client,
    keyword: &str,
    options: serde_json::Map<String, serde_json::Value>,
    timeout_secs: u32,
    event_tx: &EventTx,
) {
    let keyword_trim = keyword.trim();
    if keyword_trim.is_empty() {
        let _ = event_tx.send(EngineEvent::Ed2kSearchFailed {
            gid: String::new(),
            error: "empty keyword".to_string(),
        });
        return;
    }
    let search_dir = match ed2k_search_temp_dir() {
        Ok(d) => d,
        Err(e) => {
            let _ = event_tx.send(EngineEvent::Ed2kSearchFailed {
                gid: String::new(),
                error: e,
            });
            return;
        }
    };
    let mut options = options;
    let aria2 = crate::config::load();
    if !aria2.aria2.ed2k_server.trim().is_empty() {
        options
            .entry("ed2k-server".to_string())
            .or_insert(serde_json::Value::String(
                aria2.aria2.ed2k_server.trim().to_string(),
            ));
    }
    if let Err(e) =
        crate::ed2k_bootstrap::inject_managed_bootstrap_options(&mut options, &aria2.aria2)
    {
        tracing::warn!(error = %e, "ed2k search: managed bootstrap injection failed");
    }
    let cache_status = crate::ed2k_bootstrap::bootstrap_status();
    tracing::info!(
        keyword = %keyword_trim,
        ?options,
        server_met_modified = ?cache_status.0,
        nodes_dat_modified = ?cache_status.1,
        "ed2k: dispatching ed2kSearch RPC"
    );
    let params = vec![
        serde_json::Value::String(keyword_trim.to_string()),
        serde_json::Value::Object(options),
    ];
    match timeout(
        RPC_TIMEOUT,
        client.call_and_wait::<String>("ed2kSearch", params),
    )
    .await
    {
        Ok(Ok(gid)) => {
            if !is_safe_gid(&gid) {
                cleanup_ed2k_search_dir(&search_dir);
                let _ = event_tx.send(EngineEvent::Ed2kSearchFailed {
                    gid: String::new(),
                    error: format!("unsafe gid from ed2kSearch: {gid:?}"),
                });
                return;
            }
            crate::ed2k_bootstrap::record_search_dir(&gid, search_dir);
            let _ = event_tx.send(EngineEvent::Ed2kSearchStarted { gid: gid.clone() });
            let poll_client = client.clone();
            let poll_event_tx = event_tx.clone();
            let poll_gid = gid.clone();
            let poll_timeout_secs = timeout_secs.max(1);
            tokio::spawn(async move {
                let deadline = Instant::now() + Duration::from_secs(poll_timeout_secs as u64);
                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    if !is_safe_gid(&poll_gid) {
                        break;
                    }
                    if Instant::now() >= deadline {
                        tracing::info!(
                            ?poll_gid,
                            timeout_secs = poll_timeout_secs,
                            "ed2k search poll deadline reached"
                        );
                        break;
                    }
                    match timeout(
                        RPC_TIMEOUT,
                        poll_client.call_and_wait::<serde_json::Value>(
                            "getEd2kSearchResults",
                            vec![serde_json::Value::String(poll_gid.clone())],
                        ),
                    )
                    .await
                    {
                        Ok(Ok(results)) => {
                            let is_complete = results
                                .get("status")
                                .and_then(|s| s.as_str())
                                .map(|s| s == "complete" || s == "removed")
                                .unwrap_or(false);
                            let _ = poll_event_tx.send(EngineEvent::Ed2kSearchResults {
                                gid: poll_gid.clone(),
                                results,
                            });
                            if is_complete {
                                break;
                            }
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(?poll_gid, error = ?e, "getEd2kSearchResults failed");
                            break;
                        }
                        Err(_) => {
                            tracing::warn!(?poll_gid, "getEd2kSearchResults timed out");
                        }
                    }
                }
            });
        }
        Ok(Err(e)) => {
            tracing::warn!(error = ?e, keyword = %keyword_trim, "ed2k: ed2kSearch RPC returned error");
            cleanup_ed2k_search_dir(&search_dir);
            let _ = event_tx.send(EngineEvent::Ed2kSearchFailed {
                gid: String::new(),
                error: format!("ed2kSearch: {e}"),
            });
        }
        Err(_) => {
            cleanup_ed2k_search_dir(&search_dir);
            let _ = event_tx.send(EngineEvent::Ed2kSearchFailed {
                gid: String::new(),
                error: "ed2kSearch timed out".to_string(),
            });
        }
    }
}

pub(crate) async fn ed2k_search_cleanup(client: &Client, gid: &str, event_tx: &EventTx) {
    if !is_safe_gid(gid) {
        return;
    }
    let _ = client.force_remove(gid).await;
    let _ = client.remove_download_result(gid).await;
    if let Some(dir) = crate::ed2k_bootstrap::take_search_dir(gid) {
        cleanup_ed2k_search_dir(&dir);
    }
    let _ = event_tx;
}
