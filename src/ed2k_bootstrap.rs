use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::engine::EngineEvent;

const MAX_BOOTSTRAP_FILE_SIZE: u64 = 4 * 1024 * 1024;

static SEARCH_DIRS: OnceLock<std::sync::Mutex<HashMap<String, PathBuf>>> = OnceLock::new();

fn search_dirs() -> &'static std::sync::Mutex<HashMap<String, PathBuf>> {
    SEARCH_DIRS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

pub fn record_search_dir(gid: &str, dir: PathBuf) {
    if let Ok(mut dirs) = search_dirs().lock() {
        dirs.insert(gid.to_string(), dir);
    }
}

pub fn take_search_dir(gid: &str) -> Option<PathBuf> {
    search_dirs()
        .lock()
        .ok()
        .and_then(|mut dirs| dirs.remove(gid))
}

pub(crate) fn bootstrap_dir() -> Option<PathBuf> {
    crate::config::db_path().and_then(|p| p.parent().map(|d| d.join("ed2k-bootstrap")))
}

pub(crate) fn server_met_path() -> Option<PathBuf> {
    bootstrap_dir().map(|d| d.join("server.met"))
}

pub(crate) fn nodes_dat_path() -> Option<PathBuf> {
    bootstrap_dir().map(|d| d.join("nodes.dat"))
}

fn validate_bootstrap_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!("unsupported bootstrap URL scheme: {url:?}"));
    }
    reqwest::Url::parse(url).map_err(|e| format!("invalid bootstrap URL: {e}"))?;
    Ok(())
}

fn build_client(proxy: Option<String>) -> Result<reqwest::Client, String> {
    let builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5));
    crate::config::apply_proxy(builder, proxy.as_deref())
        .map_err(|e| format!("bootstrap client: {e}"))?
        .build()
        .map_err(|e| format!("bootstrap client build: {e}"))
}

async fn download_bootstrap_file(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    validate_bootstrap_url(url)?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("bootstrap fetch: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "bootstrap fetch returned HTTP {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("bootstrap read: {e}"))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_BOOTSTRAP_FILE_SIZE {
        return Err(format!("invalid bootstrap size: {}", bytes.len()));
    }
    Ok(bytes.to_vec())
}

fn write_cache_file(target: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }
    let tmp = target.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("write tmp: {e}"))?;
    if target.exists() {
        let _ = std::fs::remove_file(target);
    }
    std::fs::rename(&tmp, target).map_err(|e| format!("rename: {e}"))
}

fn file_modified_millis(path: &Path) -> Option<i64> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let dt: chrono::DateTime<chrono::Utc> = modified.into();
    Some(dt.timestamp_millis())
}

pub fn bootstrap_status() -> (Option<i64>, Option<i64>) {
    (
        server_met_path().as_deref().and_then(file_modified_millis),
        nodes_dat_path().as_deref().and_then(file_modified_millis),
    )
}

pub async fn sync_once(event_tx: crate::engine::EventTx) {
    let settings = crate::config::load();
    let server_met_url = settings.aria2.ed2k_server_met_url.clone();
    let nodes_dat_url = settings.aria2.ed2k_nodes_dat_url.clone();
    if server_met_url.trim().is_empty() && nodes_dat_url.trim().is_empty() {
        let _ = event_tx.send(EngineEvent::Ed2kBootstrapSyncFailed {
            error: "no bootstrap URLs configured".to_string(),
        });
        return;
    }
    let proxy = settings.aria2.all_proxy_value();
    let client = match build_client(proxy) {
        Ok(c) => c,
        Err(e) => {
            let _ = event_tx.send(EngineEvent::Ed2kBootstrapSyncFailed { error: e });
            return;
        }
    };
    let mut had_error = false;
    let mut last_err = String::new();
    if !server_met_url.trim().is_empty() {
        if let Some(path) = server_met_path() {
            match download_bootstrap_file(&client, &server_met_url).await {
                Ok(bytes) => {
                    if let Err(e) = write_cache_file(&path, &bytes) {
                        had_error = true;
                        last_err = format!("server.met: {e}");
                    }
                }
                Err(e) => {
                    had_error = true;
                    last_err = format!("server.met: {e}");
                }
            }
        }
    }
    if !nodes_dat_url.trim().is_empty() {
        if let Some(path) = nodes_dat_path() {
            match download_bootstrap_file(&client, &nodes_dat_url).await {
                Ok(bytes) => {
                    if let Err(e) = write_cache_file(&path, &bytes) {
                        had_error = true;
                        last_err = format!("nodes.dat: {e}");
                    }
                }
                Err(e) => {
                    had_error = true;
                    last_err = format!("nodes.dat: {e}");
                }
            }
        }
    }
    if had_error {
        let _ = event_tx.send(EngineEvent::Ed2kBootstrapSyncFailed { error: last_err });
        return;
    }
    let (sm, nd) = bootstrap_status();
    let _ = event_tx.send(EngineEvent::Ed2kBootstrapSynced {
        server_met_modified: sm,
        nodes_dat_modified: nd,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_bootstrap_url_https() {
        assert!(validate_bootstrap_url("https://example.com/server.met").is_ok());
        assert!(validate_bootstrap_url("http://example.com/server.met").is_ok());
    }

    #[test]
    fn validate_bootstrap_url_rejects_other_schemes() {
        assert!(validate_bootstrap_url("file:///etc/passwd").is_err());
        assert!(validate_bootstrap_url("ftp://example.com/x").is_err());
        assert!(validate_bootstrap_url("not a url").is_err());
    }

    #[test]
    fn search_dir_record_take() {
        let gid = "deadbeef";
        let dir = std::env::temp_dir().join("remotrix-test-search-dir");
        record_search_dir(gid, dir.clone());
        assert_eq!(take_search_dir(gid), Some(dir));
        assert_eq!(take_search_dir(gid), None);
    }

    #[test]
    fn write_cache_file_creates_parent() {
        let dir =
            std::env::temp_dir().join(format!("remotrix-bootstrap-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("nested").join("server.met");
        write_cache_file(&target, b"hello").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"hello".to_vec());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
