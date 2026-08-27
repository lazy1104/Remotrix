//! In-process HTTP / HTTPS fixtures for the e2e tests.
//!
//! `HttpFixture` boots an axum server on `127.0.0.1:0` so aria2 can
//! connect without port coordination. `HttpsFixture` wraps the same
//! router in a rustls acceptor driven by a self-signed `rcgen` cert and
//! writes the CA bundle to disk so tests can pass it to aria2 via the
//! global `ca-certificate` option. Both fixtures:
//!   * serve `<fixture_dir>/<name>` on `GET /file/<name>`,
//!   * serve the same files on `GET /flaky/<name>?fail_n=N` with the
//!     first `N` requests returning 503 (used to exercise aria2's
//!     mirror-fallback retry path),
//!   * return 503 forever on `GET /always500`,
//!   * accept-and-hang on `GET /hang` until the test drops the fixture
//!     (used together with `connect-timeout=1`).
//!
//! Files dropped into the fixture dir become available via
//! `/file/<basename>` as soon as `start` returns. Each fixture owns a
//! graceful-shutdown channel; calling `shutdown().await` ends the
//! server task cleanly.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path as AxPath, Query, State};
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rcgen::generate_simple_self_signed;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub struct HttpFixture {
    /// Bound address; surfaced for tests that need to assert on the
    /// listening port (none yet, kept for Phase 2-3 diagnostics).
    #[allow(dead_code)]
    pub addr: SocketAddr,
    pub base_url: String,
    /// Path the fixture serves files from — tests write source
    /// fixtures into this directory before starting downloads.
    #[allow(dead_code)]
    pub fixture_dir: PathBuf,
    /// Counter incremented on every flaky request — lets tests assert
    /// the retry path was actually exercised.
    #[allow(dead_code)]
    pub flaky_hits: Arc<AtomicUsize>,
    cancel: Option<oneshot::Sender<()>>,
    serve: Option<tokio::task::JoinHandle<()>>,
}

impl HttpFixture {
    pub async fn start(fixture_dir: PathBuf) -> Self {
        let flaky_hits = Arc::new(AtomicUsize::new(0));
        let state = AppState {
            fixture_dir: fixture_dir.clone(),
            flaky_hits: flaky_hits.clone(),
        };
        let app = Router::new()
            .route("/file/:name", get(serve_file))
            .route("/flaky/:name", get(serve_flaky))
            .route("/always500", get(always_500))
            .route("/hang", get(hang))
            .layer(middleware::from_fn(log_request))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind http");
        let addr = listener.local_addr().expect("local_addr");
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let serve = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = cancel_rx.await;
                })
                .await;
        });
        let base_url = format!("http://{addr}");
        HttpFixture {
            addr,
            base_url,
            fixture_dir,
            flaky_hits,
            cancel: Some(cancel_tx),
            serve: Some(serve),
        }
    }

    pub fn file_url(&self, name: &str) -> String {
        format!("{}/file/{}", self.base_url, name)
    }

    /// Returns a multi-mirror URL list where the first URL is
    /// `/flaky/<name>?fail_n=2` (the same logical file via the flaky
    /// handler, which 503s twice before succeeding on the 3rd try) and
    /// the remaining URLs are the normal `/file/<name>` mirror. aria2
    /// hits the first URL, sees two 503s, then falls back to the next.
    #[allow(dead_code)]
    pub fn flaky_then_normal(&self, name: &str) -> Vec<String> {
        vec![
            format!("{}/flaky/{}?fail_n=2", self.base_url, name),
            self.file_url(name),
            self.file_url(name),
        ]
    }

    /// Single flaky URL — the fixture 503s the first `fail_n` requests,
    /// then serves the file normally. Used to verify aria2 retries
    /// 5xx responses within a single task (the per-task retry path;
    /// the mirror-fallback path requires a single task with multiple
    /// URLs which `add_download` does not emit).
    #[allow(dead_code)]
    pub fn flaky_url(&self, name: &str, fail_n: u32) -> String {
        format!("{}/flaky/{}?fail_n={fail_n}", self.base_url, name)
    }

    /// URL that returns 503 forever — used to exercise aria2's
    /// max-tries exhaustion path (`http_always500_yields_error_with_code`).
    #[allow(dead_code)]
    pub fn always_500_url(&self) -> String {
        format!("{}/always500", self.base_url)
    }

    /// URL that accepts the connection but never sends a response —
    /// combined with a low `connect-timeout`, exercises aria2's
    /// timeout-retry path (`http_hang_endpoint_triggers_retry_until_timeout`).
    #[allow(dead_code)]
    pub fn hang_url(&self) -> String {
        format!("{}/hang", self.base_url)
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.cancel.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.serve.take() {
            let _ = handle.await;
        }
    }
}

#[derive(Clone)]
struct AppState {
    fixture_dir: PathBuf,
    flaky_hits: Arc<AtomicUsize>,
}

async fn serve_file(
    AxPath(name): AxPath<String>,
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
) -> Response {
    let path = state.fixture_dir.join(&name);
    serve_path(&path, headers.get(header::RANGE)).await
}

async fn serve_flaky(
    AxPath(name): AxPath<String>,
    Query(q): Query<FlakyQuery>,
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
) -> Response {
    let hit = state.flaky_hits.fetch_add(1, Ordering::Relaxed);
    if hit < q.fail_n as usize {
        return (StatusCode::SERVICE_UNAVAILABLE, "flaky").into_response();
    }
    let path = state.fixture_dir.join(&name);
    serve_path(&path, headers.get(header::RANGE)).await
}

async fn always_500() -> Response {
    (StatusCode::SERVICE_UNAVAILABLE, "always500").into_response()
}

async fn hang() -> Response {
    std::future::pending::<()>().await;
    (StatusCode::OK, "never").into_response()
}

async fn serve_path(path: &Path, range_header: Option<&axum::http::HeaderValue>) -> Response {
    let Ok(bytes) = tokio::fs::read(path).await else {
        return (StatusCode::NOT_FOUND, "missing").into_response();
    };
    let total = bytes.len();
    // Parse `Range: bytes=START-END` (one range only — sufficient for the
    // e2e tests; aria2 never asks for multi-range responses).
    let (start, end, status) = if let Some(hv) = range_header {
        let s = hv.to_str().unwrap_or("");
        if let Some(rest) = s.strip_prefix("bytes=") {
            if let Some((a, b)) = rest.split_once('-') {
                let start: usize = a.parse().unwrap_or(0);
                let end: usize = if b.is_empty() {
                    total.saturating_sub(1)
                } else {
                    b.parse().unwrap_or(total.saturating_sub(1)).min(total - 1)
                };
                if start <= end && start < total {
                    (start, end, StatusCode::PARTIAL_CONTENT)
                } else {
                    return (StatusCode::RANGE_NOT_SATISFIABLE, "bad range").into_response();
                }
            } else {
                (0, total.saturating_sub(1), StatusCode::OK)
            }
        } else {
            (0, total.saturating_sub(1), StatusCode::OK)
        }
    } else {
        (0, total.saturating_sub(1), StatusCode::OK)
    };
    let slice = bytes[start..=end].to_vec();
    let len = slice.len();
    let mut resp = (status, Body::from(slice)).into_response();
    resp.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&len.to_string()).unwrap(),
    );
    resp.headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if status == StatusCode::PARTIAL_CONTENT {
        let cr = format!("bytes {start}-{end}/{total}");
        resp.headers_mut()
            .insert(header::CONTENT_RANGE, HeaderValue::from_str(&cr).unwrap());
    }
    resp
}

async fn log_request(req: Request<Body>, next: Next) -> Result<Response, Infallible> {
    Ok::<_, Infallible>(next.run(req).await)
}

#[derive(Deserialize)]
struct FlakyQuery {
    #[serde(default)]
    fail_n: u32,
}

/// HTTPS variant of `HttpFixture`: same routes, but served via rustls
/// using a self-signed cert generated by `rcgen`. The CA bundle is
/// written to `cert_dir/ca.pem` so the test can pass it to aria2 via
/// the global `ca-certificate` option (and verify aria2 actually
/// validates the chain instead of trusting `--insecure`).
pub struct HttpsFixture {
    /// Inner plain HTTP fixture, kept so tests can inspect flaky_hits
    /// / fixture_dir; currently unused since HTTPS tests don't touch
    /// the HTTP port.
    #[allow(dead_code)]
    pub http: HttpFixture,
    pub ca_cert_path: PathBuf,
    pub base_url: String,
    cancel: Option<oneshot::Sender<()>>,
    serve: Option<tokio::task::JoinHandle<()>>,
}

impl HttpsFixture {
    pub async fn start(fixture_dir: PathBuf, cert_dir: PathBuf) -> Self {
        // SAN must cover every host the test URL will use — fixture URLs
        // bind to 127.0.0.1, not localhost, so include both.
        let cert =
            generate_simple_self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()])
                .expect("cert");
        let cert_pem = cert.cert.pem();
        let key_pem = cert.key_pair.serialize_pem();
        std::fs::create_dir_all(&cert_dir).expect("cert_dir");
        let ca_cert_path = cert_dir.join("ca.pem");
        std::fs::write(&ca_cert_path, &cert_pem).expect("write cert");

        let state = AppState {
            fixture_dir: fixture_dir.clone(),
            flaky_hits: Arc::new(AtomicUsize::new(0)),
        };
        let app = Router::new()
            .route("/file/:name", get(serve_file))
            .route("/always500", get(always_500))
            .route("/hang", get(hang))
            .with_state(state);

        let cfg = axum_server::tls_rustls::RustlsConfig::from_pem(
            cert_pem.into_bytes(),
            key_pem.into_bytes(),
        )
        .await
        .expect("rustls config");

        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind https");
        std_listener.set_nonblocking(true).expect("nonblocking");
        let addr = std_listener.local_addr().expect("local_addr");

        let handle = axum_server::Handle::new();
        let handle_for_shutdown = handle.clone();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let serve = tokio::spawn(async move {
            let server = axum_server::from_tcp_rustls(std_listener, cfg).handle(handle);
            tokio::spawn(async move {
                let _ = cancel_rx.await;
                handle_for_shutdown.shutdown();
            });
            let _ = server.serve(app.into_make_service()).await;
        });

        let base_url = format!("https://{addr}");
        let http = HttpFixture {
            addr,
            base_url: base_url.clone(),
            fixture_dir,
            flaky_hits: Arc::new(AtomicUsize::new(0)),
            cancel: None,
            serve: None,
        };
        let _ = http;

        HttpsFixture {
            http,
            ca_cert_path,
            base_url,
            cancel: Some(cancel_tx),
            serve: Some(serve),
        }
    }

    pub fn file_url(&self, name: &str) -> String {
        format!("{}/file/{}", self.base_url, name)
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.cancel.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.serve.take() {
            let _ = handle.await;
        }
    }
}
