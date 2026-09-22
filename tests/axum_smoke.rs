//! Runtime smoke test for the axum 0.8 + axum-server 0.8 upgrade.
//!
//! Independent of the `download_e2e` suite (which returns `Ok(())` early
//! when no `aria2-next` binary is on disk, leaving the axum code paths
//! unexercised). This test binds ephemeral ports and issues real
//! `reqwest` requests so the upgrade is verified end-to-end.

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn axum_0_8_serve_accepts_get() {
    use axum::{routing::get, Router};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind http");
    let addr = listener.local_addr().expect("local_addr");
    let app = Router::new().route("/ping", get(|| async { "pong" }));

    let serve = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("axum::serve");
    });

    let body = reqwest::get(format!("http://{addr}/ping"))
        .await
        .expect("http GET")
        .text()
        .await
        .expect("http body");

    assert_eq!(body, "pong");
    serve.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn axum_server_0_8_rustls_handshake_succeeds() {
    use axum::{routing::get, Router};
    use axum_server::tls_rustls::RustlsConfig;
    use rcgen::generate_simple_self_signed;
    use std::net::TcpListener as StdTcpListener;

    let cert = generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])
        .expect("generate cert");
    let cert_pem = cert.cert.pem();
    let key_pem = cert.signing_key.serialize_pem();

    let cfg = RustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.into_bytes())
        .await
        .expect("rustls config");

    let std_listener = StdTcpListener::bind("127.0.0.1:0").expect("bind https");
    std_listener.set_nonblocking(true).expect("nonblocking");
    let addr = std_listener.local_addr().expect("local_addr");

    let handle = axum_server::Handle::new();
    let app = Router::new().route("/ping", get(|| async { "pong" }));
    let server = axum_server::from_tcp_rustls(std_listener, cfg)
        .expect("axum_server tls bind")
        .handle(handle);

    let serve = tokio::spawn(async move {
        let _ = server.serve(app.into_make_service()).await;
    });

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(10))
        .build()
        .expect("tls client");
    let body = client
        .get(format!("https://{addr}/ping"))
        .send()
        .await
        .expect("https GET")
        .text()
        .await
        .expect("https body");

    assert_eq!(body, "pong");
    serve.abort();
}
