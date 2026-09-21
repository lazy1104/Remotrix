use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::config::Aria2Options;
use crate::engine::EngineEvent;

const MAX_BOOTSTRAP_FILE_SIZE: u64 = 4 * 1024 * 1024;

const BUNDLED_SERVER_MET: &[u8] = include_bytes!("../../assets/ed2k-bootstrap/server.met");
const BUNDLED_NODES_DAT: &[u8] = include_bytes!("../../assets/ed2k-bootstrap/nodes.dat");

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

fn copy_default_if_missing(target: &Path, bytes: &[u8]) -> Result<(), String> {
    if target.is_file() {
        return Ok(());
    }
    write_cache_file(target, bytes)
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

fn ensure_cache_with(base: &Path) -> Result<(PathBuf, PathBuf), String> {
    let dir = base.join("ed2k-bootstrap");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create bootstrap dir: {e}"))?;
    let server_met = dir.join("server.met");
    let nodes_dat = dir.join("nodes.dat");
    copy_default_if_missing(&server_met, BUNDLED_SERVER_MET)?;
    copy_default_if_missing(&nodes_dat, BUNDLED_NODES_DAT)?;
    if std::fs::metadata(&server_met)
        .map(|m| m.len() == 0)
        .unwrap_or(true)
    {
        return Err(format!(
            "bundled server.met is empty at {}",
            server_met.display()
        ));
    }
    if std::fs::metadata(&nodes_dat)
        .map(|m| m.len() == 0)
        .unwrap_or(true)
    {
        return Err(format!(
            "bundled nodes.dat is empty at {}",
            nodes_dat.display()
        ));
    }
    Ok((server_met, nodes_dat))
}

pub fn ensure_cache() -> Result<(PathBuf, PathBuf), String> {
    let base = crate::config::db_path()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .ok_or_else(|| "no data dir".to_string())?;
    ensure_cache_with(&base)
}

pub fn inject_managed_bootstrap_args(
    args: &mut Vec<String>,
    opts: &Aria2Options,
) -> Result<(), String> {
    let base = crate::config::db_path()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .ok_or_else(|| "no data dir".to_string())?;
    inject_managed_bootstrap_args_with(&base, args, opts)
}

fn inject_managed_bootstrap_args_with(
    base: &Path,
    args: &mut Vec<String>,
    opts: &Aria2Options,
) -> Result<(), String> {
    let (server_met, nodes_dat) = match ensure_cache_with(base) {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!(error = %e, "ed2k: ensure_cache failed; skipping managed bootstrap injection");
            return Ok(());
        }
    };
    if opts.ed2k_server_list.trim().is_empty() {
        args.push("--ed2k-server-list".to_string());
        args.push(server_met.to_string_lossy().into_owned());
    }
    if opts.ed2k_node_list.trim().is_empty() {
        args.push("--ed2k-node-list".to_string());
        args.push(nodes_dat.to_string_lossy().into_owned());
    }
    Ok(())
}

pub fn inject_managed_bootstrap_options(
    options: &mut serde_json::Map<String, serde_json::Value>,
    opts: &Aria2Options,
) -> Result<(), String> {
    let base = crate::config::db_path()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .ok_or_else(|| "no data dir".to_string())?;
    inject_managed_bootstrap_options_with(&base, options, opts)
}

fn inject_managed_bootstrap_options_with(
    base: &Path,
    options: &mut serde_json::Map<String, serde_json::Value>,
    opts: &Aria2Options,
) -> Result<(), String> {
    let (server_met, nodes_dat) = match ensure_cache_with(base) {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!(error = %e, "ed2k: ensure_cache failed; skipping managed bootstrap injection");
            return Ok(());
        }
    };
    if opts.ed2k_server_list.trim().is_empty() {
        options
            .entry("ed2k-server-list".to_string())
            .or_insert(serde_json::Value::String(
                server_met.to_string_lossy().into_owned(),
            ));
    }
    if opts.ed2k_node_list.trim().is_empty() {
        options
            .entry("ed2k-node-list".to_string())
            .or_insert(serde_json::Value::String(
                nodes_dat.to_string_lossy().into_owned(),
            ));
    }
    Ok(())
}

pub async fn sync_once(event_tx: crate::engine::EventTx) {
    if let Err(e) = ensure_cache() {
        let _ = event_tx.send(EngineEvent::Ed2kBootstrapSyncFailed { error: e });
        return;
    }
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

    fn fresh_base(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "remotrix-bootstrap-cache-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn ensure_cache_with_writes_defaults_when_missing() {
        let base = fresh_base("missing");
        let (server_met, nodes_dat) = ensure_cache_with(&base).unwrap();
        assert!(server_met.is_file());
        assert!(nodes_dat.is_file());
        assert!(!std::fs::read(&server_met).unwrap().is_empty());
        assert!(!std::fs::read(&nodes_dat).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ensure_cache_with_does_not_overwrite() {
        let base = fresh_base("nooverwrite");
        let (server_met, nodes_dat) = ensure_cache_with(&base).unwrap();
        std::fs::write(&server_met, b"user-server").unwrap();
        std::fs::write(&nodes_dat, b"user-nodes").unwrap();
        ensure_cache_with(&base).unwrap();
        assert_eq!(std::fs::read(&server_met).unwrap(), b"user-server".to_vec());
        assert_eq!(std::fs::read(&nodes_dat).unwrap(), b"user-nodes".to_vec());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ensure_cache_with_rejects_empty_cache_files() {
        let base = fresh_base("empty");
        let dir = base.join("ed2k-bootstrap");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("server.met"), b"").unwrap();
        std::fs::write(dir.join("nodes.dat"), b"valid").unwrap();
        let err = ensure_cache_with(&base).unwrap_err();
        assert!(err.contains("server.met"), "got: {err}");
        std::fs::write(dir.join("server.met"), b"valid").unwrap();
        std::fs::write(dir.join("nodes.dat"), b"").unwrap();
        let err = ensure_cache_with(&base).unwrap_err();
        assert!(err.contains("nodes.dat"), "got: {err}");
        let _ = std::fs::remove_dir_all(&base);
    }

    fn opts_with(server_list: &str, node_list: &str) -> Aria2Options {
        let mut opts = Aria2Options::default();
        opts.ed2k_server_list = server_list.into();
        opts.ed2k_node_list = node_list.into();
        opts
    }

    #[test]
    fn inject_managed_bootstrap_args_uses_cache_when_user_empty() {
        let base = fresh_base("inject-empty");
        let (server_met, nodes_dat) = ensure_cache_with(&base).unwrap();
        let opts = opts_with("", "");
        let mut args = Vec::new();
        inject_managed_bootstrap_args_with(&base, &mut args, &opts).unwrap();
        assert_eq!(
            args,
            vec![
                "--ed2k-server-list".to_string(),
                server_met.to_string_lossy().into_owned(),
                "--ed2k-node-list".to_string(),
                nodes_dat.to_string_lossy().into_owned(),
            ]
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn inject_managed_bootstrap_args_skips_when_user_set() {
        let base = fresh_base("inject-set");
        ensure_cache_with(&base).unwrap();
        let opts = opts_with("/explicit/server.met", "/explicit/nodes.dat");
        let mut args = Vec::new();
        inject_managed_bootstrap_args_with(&base, &mut args, &opts).unwrap();
        assert!(args.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn inject_managed_bootstrap_args_trims_whitespace_only_as_empty() {
        let base = fresh_base("inject-ws");
        let (server_met, nodes_dat) = ensure_cache_with(&base).unwrap();
        let opts = opts_with("   ", "\n\t  ");
        let mut args = Vec::new();
        inject_managed_bootstrap_args_with(&base, &mut args, &opts).unwrap();
        assert_eq!(
            args,
            vec![
                "--ed2k-server-list".to_string(),
                server_met.to_string_lossy().into_owned(),
                "--ed2k-node-list".to_string(),
                nodes_dat.to_string_lossy().into_owned(),
            ]
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn inject_managed_bootstrap_options_uses_cache_when_empty() {
        let base = fresh_base("opts-empty");
        let (server_met, nodes_dat) = ensure_cache_with(&base).unwrap();
        let opts = opts_with("", "");
        let mut options = serde_json::Map::new();
        inject_managed_bootstrap_options_with(&base, &mut options, &opts).unwrap();
        assert_eq!(
            options.get("ed2k-server-list"),
            Some(&serde_json::Value::String(
                server_met.to_string_lossy().into_owned()
            ))
        );
        assert_eq!(
            options.get("ed2k-node-list"),
            Some(&serde_json::Value::String(
                nodes_dat.to_string_lossy().into_owned()
            ))
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn inject_managed_bootstrap_options_skips_when_present() {
        let base = fresh_base("opts-skip");
        ensure_cache_with(&base).unwrap();
        let opts = opts_with("/explicit/server.met", "/explicit/nodes.dat");
        let mut options = serde_json::Map::new();
        options.insert(
            "ed2k-server-list".to_string(),
            serde_json::Value::String("/user/server.met".into()),
        );
        options.insert(
            "ed2k-node-list".to_string(),
            serde_json::Value::String("/user/nodes.dat".into()),
        );
        inject_managed_bootstrap_options_with(&base, &mut options, &opts).unwrap();
        assert_eq!(
            options.get("ed2k-server-list"),
            Some(&serde_json::Value::String("/user/server.met".into()))
        );
        assert_eq!(
            options.get("ed2k-node-list"),
            Some(&serde_json::Value::String("/user/nodes.dat".into()))
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn inject_managed_bootstrap_options_trims_whitespace_as_empty() {
        let base = fresh_base("opts-ws");
        let (server_met, nodes_dat) = ensure_cache_with(&base).unwrap();
        let opts = opts_with("   ", "\n\t  ");
        let mut options = serde_json::Map::new();
        inject_managed_bootstrap_options_with(&base, &mut options, &opts).unwrap();
        assert_eq!(
            options.get("ed2k-server-list"),
            Some(&serde_json::Value::String(
                server_met.to_string_lossy().into_owned()
            ))
        );
        assert_eq!(
            options.get("ed2k-node-list"),
            Some(&serde_json::Value::String(
                nodes_dat.to_string_lossy().into_owned()
            ))
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
