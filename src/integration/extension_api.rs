use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use salvo::cors::{AllowOrigin, Cors};
use salvo::http::{HeaderValue, Method, StatusCode};
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::config::{EXTENSION_API_MAX_PORT, EXTENSION_API_MIN_PORT};
use crate::engine::{CmdTx, EngineCmd};
use crate::task::TaskAdvancedOptions;

/// Shared, app-maintained counters + speeds consumed by `GET /stat`.
///
/// Mirrors aria2 `getGlobalStat`: every field is exposed as a string with
/// camelCase JSON keys. The app refreshes it on `EngineEvent::GlobalSpeed`
/// and on task status transitions.
#[derive(Debug, Clone, Default)]
pub struct GlobalStatCache {
    pub download_speed: u64,
    pub upload_speed: u64,
    pub num_active: usize,
    pub num_waiting: usize,
    pub num_stopped: usize,
    pub num_stopped_total: usize,
}

/// A download handed over by the browser extension.
///
/// Used for the dialog-mode path (`/add` when auto-submit is disabled).
#[derive(Debug, Clone, Default)]
pub struct ExternalDownload {
    pub urls: Vec<String>,
    pub referer: Option<String>,
    pub cookie: Option<String>,
    pub filename: Option<String>,
    pub user_agent: Option<String>,
    pub request_headers: Vec<(String, String)>,
    pub dir: Option<PathBuf>,
    pub out_basename: Option<String>,
}

struct Shared {
    cmd_tx: CmdTx,
    stat_cache: Arc<Mutex<GlobalStatCache>>,
    on_dialog: Option<Box<dyn Fn(ExternalDownload) + Send + Sync + 'static>>,
}

static SHARED: Mutex<Option<Arc<Shared>>> = Mutex::new(None);
static TASK: Mutex<Option<tokio::task::JoinHandle<()>>> = Mutex::new(None);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddDownloadRequest {
    url: String,
    #[serde(default)]
    final_url: Option<String>,
    #[serde(default)]
    referer: Option<String>,
    #[serde(default)]
    cookie: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    user_agent: Option<String>,
    #[serde(default)]
    request_headers: Option<Vec<HeaderItem>>,
    #[serde(default)]
    dir: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct HeaderItem {
    name: String,
    value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatResponse {
    download_speed: String,
    upload_speed: String,
    num_active: String,
    num_waiting: String,
    num_stopped: String,
    num_stopped_total: String,
}

impl GlobalStatCache {
    fn to_stat_response(&self) -> StatResponse {
        StatResponse {
            download_speed: self.download_speed.to_string(),
            upload_speed: self.upload_speed.to_string(),
            num_active: self.num_active.to_string(),
            num_waiting: self.num_waiting.to_string(),
            num_stopped: self.num_stopped.to_string(),
            num_stopped_total: self.num_stopped_total.to_string(),
        }
    }
}

impl ExternalDownload {
    fn from_request(body: &AddDownloadRequest) -> Self {
        let url = body.url.clone();
        let primary = match &body.final_url {
            Some(final_url) if !final_url.is_empty() && final_url != &url => final_url.clone(),
            _ => url,
        };
        let request_headers = body
            .request_headers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|h| (h.name, h.value))
            .collect();
        let (dir, out_basename) = normalize_save_dir(body.dir.as_deref().or(body.path.as_deref()))
            .unwrap_or((None, None));
        Self {
            urls: vec![primary],
            referer: body.referer.clone(),
            cookie: body.cookie.clone(),
            filename: body.filename.clone(),
            user_agent: body.user_agent.clone(),
            request_headers,
            dir,
            out_basename,
        }
    }
}

fn normalize_save_dir(raw: Option<&str>) -> Option<(Option<PathBuf>, Option<String>)> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return None;
    }
    let meta = std::fs::metadata(&path).ok();
    match meta {
        Some(m) if m.is_file() => {
            let parent = path.parent().map(|p| p.to_path_buf());
            let stem = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());
            Some((parent, stem))
        }
        _ => Some((Some(path), None)),
    }
}

pub fn generate_secret() -> String {
    crate::config::generate_secret()
}

pub fn spawn_server(
    cmd_tx: CmdTx,
    stat_cache: Arc<Mutex<GlobalStatCache>>,
    on_dialog: Option<Box<dyn Fn(ExternalDownload) + Send + Sync + 'static>>,
    on_ready: Option<Box<dyn Fn(bool) + Send + Sync + 'static>>,
) -> Option<tokio::task::JoinHandle<()>> {
    if let Some(handle) = TASK.lock().unwrap_or_else(|e| e.into_inner()).take() {
        handle.abort();
    }
    *SHARED.lock().unwrap_or_else(|e| e.into_inner()) = None;

    let mut settings = crate::config::load();
    if !settings.extension.enabled {
        return None;
    }
    if settings.extension.secret.trim().is_empty() {
        settings.extension.secret = generate_secret();
        crate::config::save(&settings);
    }
    let port = settings
        .extension
        .port
        .clamp(EXTENSION_API_MIN_PORT, EXTENSION_API_MAX_PORT);
    let shared = Arc::new(Shared {
        cmd_tx,
        stat_cache,
        on_dialog,
    });
    *SHARED.lock().unwrap_or_else(|e| e.into_inner()) = Some(shared);
    let addr = format!("127.0.0.1:{port}");
    tracing::info!(%addr, "spawning extension API server");
    let handle = tokio::spawn(async move {
        let result = serve(addr).await;
        if let Some(on_ready) = on_ready {
            on_ready(result.is_ok());
        }
        if let Err(e) = result {
            tracing::error!(error = %e, "extension API server failed");
        }
    });
    *TASK.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    None
}

async fn serve(addr: String) -> Result<(), String> {
    let acceptor = TcpListener::new(addr)
        .try_bind()
        .await
        .map_err(|e| format!("bind: {e}"))?;
    let router = Router::new()
        .push(Router::with_path("ping").get(ping))
        .push(Router::with_path("stat").get(stat))
        .push(Router::with_path("add").post(add))
        .push(Router::with_path("pause-all").post(pause_all))
        .push(Router::with_path("resume-all").post(resume_all));
    let cors = Cors::new()
        .allow_origin(AllowOrigin::dynamic(extension_origin))
        .allow_methods(vec![Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(vec!["content-type", "authorization"])
        .allow_private_network(true)
        .into_handler();
    let service = Service::new(router).hoop(cors);
    Server::new(acceptor).serve(service).await;
    Ok(())
}

fn extension_origin(
    origin: Option<&HeaderValue>,
    _req: &Request,
    _depot: &Depot,
) -> Option<HeaderValue> {
    let origin = origin?.to_str().ok()?;
    if origin.starts_with("chrome-extension://") || origin.starts_with("moz-extension://") {
        HeaderValue::from_str(origin).ok()
    } else {
        None
    }
}

fn is_authorized(req: &Request, secret: &str) -> bool {
    if secret.is_empty() {
        return false;
    }
    let expected = format!("Bearer {secret}");
    req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == expected)
        .unwrap_or(false)
}

fn authorized(req: &Request) -> bool {
    is_authorized(req, &crate::config::load().extension.secret)
}

fn deny(res: &mut Response) {
    res.status_code(StatusCode::UNAUTHORIZED);
    res.render(Json(
        serde_json::json!({ "status": "error", "error": "unauthorized" }),
    ));
}

#[handler]
async fn ping(req: &mut Request, res: &mut Response) {
    if !authorized(req) {
        deny(res);
        return;
    }
    res.render(Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    })));
}

#[handler]
async fn stat(req: &mut Request, res: &mut Response) {
    if !authorized(req) {
        deny(res);
        return;
    }
    let Some(shared) = SHARED.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
        res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
        return;
    };
    let cache = shared.stat_cache.lock().unwrap_or_else(|e| e.into_inner());
    res.render(Json(cache.to_stat_response()));
}

#[handler]
async fn pause_all(req: &mut Request, res: &mut Response) {
    if !authorized(req) {
        deny(res);
        return;
    }
    if let Some(shared) = SHARED.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        let _ = shared.cmd_tx.send(EngineCmd::PauseAll);
    }
    res.render(Json(serde_json::json!({ "status": "ok" })));
}

#[handler]
async fn resume_all(req: &mut Request, res: &mut Response) {
    if !authorized(req) {
        deny(res);
        return;
    }
    if let Some(shared) = SHARED.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        let _ = shared.cmd_tx.send(EngineCmd::ResumeAll);
    }
    res.render(Json(serde_json::json!({ "status": "ok" })));
}

fn url_scheme_allowed(url: &str) -> bool {
    let scheme = url
        .trim_start()
        .split_once(':')
        .map(|(s, _)| s.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        scheme.as_str(),
        "http" | "https" | "ftp" | "magnet" | "ed2k" | "thunder"
    )
}

#[handler]
async fn add(req: &mut Request, res: &mut Response) {
    if !authorized(req) {
        deny(res);
        return;
    }
    let body = match req.parse_body::<AddDownloadRequest>().await {
        Ok(b) => b,
        Err(_) => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Json(serde_json::json!({
                "action": "error",
                "message": "invalid JSON body",
            })));
            return;
        }
    };
    if body.url.trim().is_empty() {
        res.status_code(StatusCode::BAD_REQUEST);
        res.render(Json(serde_json::json!({
            "action": "error",
            "message": "missing url",
        })));
        return;
    }
    if !url_scheme_allowed(&body.url)
        || body
            .final_url
            .as_deref()
            .is_some_and(|u| !url_scheme_allowed(u))
    {
        res.status_code(StatusCode::BAD_REQUEST);
        res.render(Json(serde_json::json!({
            "action": "error",
            "message": "unsupported URL scheme",
        })));
        return;
    }

    let settings = crate::config::load();
    if !settings.extension.auto_submit {
        if let Some(shared) = SHARED.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            if let Some(on_dialog) = shared.on_dialog.as_ref() {
                on_dialog(ExternalDownload::from_request(&body));
                res.render(Json(serde_json::json!({ "action": "prompt" })));
                return;
            }
        }
    }

    let external = ExternalDownload::from_request(&body);
    let out = external
        .filename
        .clone()
        .or_else(|| external.out_basename.clone())
        .unwrap_or_default();
    let advanced = TaskAdvancedOptions {
        out,
        user_agent: external.user_agent.clone().unwrap_or_default(),
        referer: external.referer.clone().unwrap_or_default(),
        cookie: external.cookie.clone().unwrap_or_default(),
        ..Default::default()
    };
    let save_dir = external
        .dir
        .clone()
        .unwrap_or_else(|| settings.download_dir.clone());
    let request_headers = external.request_headers.clone();
    if let Some(shared) = SHARED.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        let _ = shared.cmd_tx.send(EngineCmd::AddExternalDownload {
            urls: external.urls,
            save_dir,
            split: settings.split,
            advanced,
            headers: request_headers,
            bt_metadata_only: false,
        });
    }
    res.render(Json(serde_json::json!({
        "action": "added",
        "received": {
            "dir": external.dir.is_some(),
            "filename": external.filename.is_some(),
            "referer": external.referer.is_some(),
            "cookie": external.cookie.is_some(),
            "userAgent": external.user_agent.is_some(),
            "requestHeaders": external.request_headers.len(),
        }
    })));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_with_auth(secret: &str) -> Request {
        let mut req = Request::default();
        if !secret.is_empty() {
            let value = HeaderValue::from_str(&format!("Bearer {secret}")).unwrap();
            req.headers_mut().insert("authorization", value);
        }
        req
    }

    #[test]
    fn deserialize_add_request_camel_case() {
        let json = r#"{
            "url": "http://example.com/a.zip",
            "finalUrl": "http://cdn.example.com/a.zip",
            "referer": "http://example.com/",
            "cookie": "a=b",
            "filename": "a.zip",
            "userAgent": "Mozilla/5.0",
            "requestHeaders": [{"name": "X-Foo", "value": "bar"}]
        }"#;
        let body: AddDownloadRequest = serde_json::from_str(json).unwrap();
        assert_eq!(body.url, "http://example.com/a.zip");
        assert_eq!(
            body.final_url.as_deref(),
            Some("http://cdn.example.com/a.zip")
        );
        assert_eq!(body.user_agent.as_deref(), Some("Mozilla/5.0"));
        let headers = body.request_headers.unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].name, "X-Foo");
    }

    #[test]
    fn auth_rejects_when_secret_unconfigured() {
        assert!(!is_authorized(&request_with_auth(""), ""));
        assert!(!is_authorized(&request_with_auth("anything"), ""));
    }

    #[test]
    fn auth_checks_bearer() {
        assert!(!is_authorized(&request_with_auth(""), "s3cret"));
        assert!(is_authorized(&request_with_auth("s3cret"), "s3cret"));
        assert!(!is_authorized(&request_with_auth("wrong"), "s3cret"));
        assert!(!is_authorized(&request_with_auth(""), "s3cret"));
    }

    #[test]
    fn external_from_request_uses_final_url_as_single_url() {
        let json = r#"{"url":"http://a.com/f","finalUrl":"http://cdn.a.com/f"}"#;
        let body: AddDownloadRequest = serde_json::from_str(json).unwrap();
        let ext = ExternalDownload::from_request(&body);
        assert_eq!(ext.urls.len(), 1);
        assert_eq!(ext.urls[0], "http://cdn.a.com/f");
    }

    #[test]
    fn external_from_request_falls_back_to_url() {
        let json = r#"{"url":"http://a.com/f"}"#;
        let body: AddDownloadRequest = serde_json::from_str(json).unwrap();
        let ext = ExternalDownload::from_request(&body);
        assert_eq!(ext.urls, vec!["http://a.com/f".to_string()]);
    }

    #[test]
    fn external_from_request_picks_dir_as_directory() {
        let json = r#"{"url":"http://a.com/f","dir":"/tmp"}"#;
        let body: AddDownloadRequest = serde_json::from_str(json).unwrap();
        let ext = ExternalDownload::from_request(&body);
        assert_eq!(ext.dir.as_deref(), Some(std::path::Path::new("/tmp")));
        assert_eq!(ext.out_basename, None);
    }

    #[test]
    fn external_from_request_picks_path_when_dir_missing() {
        let dir = std::env::temp_dir().join("remotrix-ext-api-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("x.bin");
        std::fs::write(&file, b"x").unwrap();
        let body: AddDownloadRequest = serde_json::from_str(&format!(
            r#"{{"url":"http://a.com/f","path":"{}"}}"#,
            file.display()
        ))
        .unwrap();
        let ext = ExternalDownload::from_request(&body);
        assert_eq!(ext.dir.as_deref(), Some(dir.as_path()));
        assert_eq!(ext.out_basename.as_deref(), Some("x.bin"));
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn external_from_request_rejects_relative_path() {
        let json = r#"{"url":"http://a.com/f","dir":"relative/x"}"#;
        let body: AddDownloadRequest = serde_json::from_str(json).unwrap();
        let ext = ExternalDownload::from_request(&body);
        assert!(ext.dir.is_none());
        assert!(ext.out_basename.is_none());
    }

    #[test]
    fn external_from_request_dir_preferred_over_path() {
        let json = r#"{"url":"http://a.com/f","dir":"/tmp","path":"/var/tmp/y.bin"}"#;
        let body: AddDownloadRequest = serde_json::from_str(json).unwrap();
        let ext = ExternalDownload::from_request(&body);
        assert_eq!(ext.dir.as_deref(), Some(std::path::Path::new("/tmp")));
        assert_eq!(ext.out_basename, None);
    }

    #[test]
    fn normalize_save_dir_returns_none_for_empty_or_relative() {
        assert!(normalize_save_dir(None).is_none());
        assert!(normalize_save_dir(Some("")).is_none());
        assert!(normalize_save_dir(Some("   ")).is_none());
        assert!(normalize_save_dir(Some("./relative")).is_none());
    }

    #[test]
    fn deserialize_add_request_accepts_dir_and_path() {
        let json = r#"{
            "url": "http://example.com/a.zip",
            "dir": "/home/u/Pictures",
            "path": "/home/u/Downloads/a.zip",
            "referer": "http://example.com/"
        }"#;
        let body: AddDownloadRequest = serde_json::from_str(json).unwrap();
        assert_eq!(body.dir.as_deref(), Some("/home/u/Pictures"));
        assert_eq!(body.path.as_deref(), Some("/home/u/Downloads/a.zip"));
        assert_eq!(body.referer.as_deref(), Some("http://example.com/"));
    }

    #[test]
    fn url_scheme_allowed_rejects_file_and_unknown() {
        assert!(url_scheme_allowed("https://example.com/f"));
        assert!(url_scheme_allowed("magnet:?xt=urn:btih:abc"));
        assert!(url_scheme_allowed("ed2k://|file|a|1|"));
        assert!(url_scheme_allowed("thunder://xxxx"));
        assert!(!url_scheme_allowed("file:///etc/passwd"));
        assert!(!url_scheme_allowed("gopher://example.com"));
        assert!(!url_scheme_allowed("data:text/plain;base64,xx"));
    }

    #[test]
    fn stat_serializes_to_camel_case_strings() {
        let resp = StatResponse {
            download_speed: "1024".into(),
            upload_speed: "512".into(),
            num_active: "2".into(),
            num_waiting: "3".into(),
            num_stopped: "4".into(),
            num_stopped_total: "9".into(),
        };
        let v: serde_json::Value = serde_json::to_value(resp).unwrap();
        assert_eq!(v["downloadSpeed"], "1024");
        assert_eq!(v["uploadSpeed"], "512");
        assert_eq!(v["numActive"], "2");
        assert_eq!(v["numWaiting"], "3");
        assert_eq!(v["numStopped"], "4");
        assert_eq!(v["numStoppedTotal"], "9");
    }

    #[test]
    fn add_response_shapes() {
        let added = serde_json::json!({ "action": "added" });
        assert_eq!(added["action"], "added");
        let paused = serde_json::json!({ "status": "ok" });
        assert_eq!(paused["status"], "ok");
    }

    #[test]
    fn add_response_received_map_reflects_fields() {
        let json = r#"{
            "url":"http://a.com/f",
            "dir":"/tmp",
            "referer":"http://a.com/",
            "userAgent":"Mozilla/5.0",
            "requestHeaders":[{"name":"X-Foo","value":"bar"}]
        }"#;
        let body: AddDownloadRequest = serde_json::from_str(json).unwrap();
        let ext = ExternalDownload::from_request(&body);
        let resp = serde_json::json!({
            "action": "added",
            "received": {
                "dir": ext.dir.is_some(),
                "filename": ext.filename.is_some(),
                "referer": ext.referer.is_some(),
                "cookie": ext.cookie.is_some(),
                "userAgent": ext.user_agent.is_some(),
                "requestHeaders": ext.request_headers.len(),
            }
        });
        assert_eq!(resp["action"], "added");
        assert_eq!(resp["received"]["dir"], true);
        assert_eq!(resp["received"]["referer"], true);
        assert_eq!(resp["received"]["userAgent"], true);
        assert_eq!(resp["received"]["requestHeaders"], 1);
        assert_eq!(resp["received"]["cookie"], false);
        assert_eq!(resp["received"]["filename"], false);
    }
}
