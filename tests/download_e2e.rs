//! End-to-end regression tests for the download pipeline.
//!
//! Drives the real aria2-next sidecar via `remotrix::engine::spawn_engine`
//! against in-process HTTP/HTTPS fixtures and a local BT tracker + seeder.
//! All tests are gated on `#[cfg(any(target_os = "linux",
//! target_os = "macos"))]` because the `directories` crate on Windows
//! reads `SHGetKnownFolderPath` and ignores the `$HOME` redirect the
//! harness uses to isolate on-disk state per test.
//!
//! Skipping is graceful: when no aria2 binary is discoverable
//! (`ARIA2_BIN` unset and no `aria2-next`/`aria2c` on `$PATH`) every
//! test logs `skip: ARIA2_BIN not set` and returns `Ok(())` so the
//! suite stays green on machines without aria2 (CI, fresh dev boxes).
//!
//! Run with: `cargo test --test download_e2e -- --nocapture`

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::time::Duration;

use serial_test::serial;
use tokio::time::Instant;

mod common;

use crate::common::http::{HttpFixture, HttpsFixture};
use crate::common::{
    aria2_bin, dump_events_on_failure, sha256_file_sync, write_random_fixture, EventLog, Harness,
};

use remotrix::engine::EngineEvent;

const DEFAULT_TEST_TIMEOUT: Duration = Duration::from_secs(120);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(aria2)]
async fn http_smoke_1mb_sha256_matches() -> Result<(), String> {
    common::init_tracing();
    let _ = aria2_bin();
    let harness = match Harness::new().await {
        Some(h) => h,
        None => return Ok(()),
    };
    if !harness.wait_for_ready(Duration::from_secs(20)).await {
        dump_events_on_failure(&harness.events, "engine not ready").await;
        harness.shutdown().await;
        return Err("engine never sent EngineReady".into());
    }

    let http = HttpFixture::start(harness.fixture_dir.clone()).await;
    let name = "smoke_1mb.bin";
    let path = harness.fixture_dir.join(name);
    write_random_fixture(&path, 1024 * 1024);
    let expected_sha = sha256_file_sync(&path);

    let url = http.file_url(name);
    let save_dir = harness.download_dir.clone();
    harness
        .add_download(vec![url], save_dir.clone(), 1, Default::default(), false)
        .await;

    let gid = wait_for_added_gid(&harness.events, name, DEFAULT_TEST_TIMEOUT).await?;
    let status = wait_for_terminal(&harness.events, &gid, DEFAULT_TEST_TIMEOUT).await?;
    if status != "complete" {
        dump_events_on_failure(&harness.events, "http_smoke status").await;
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!("download did not complete: {status}"));
    }

    let downloaded = save_dir.join(name);
    let actual = sha256_file_sync(&downloaded);
    if actual != expected_sha {
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!(
            "sha256 mismatch for {}: expected {expected_sha}, got {actual}",
            downloaded.display()
        ));
    }

    http.shutdown().await;
    harness.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(aria2)]
async fn https_self_signed_ca_certificate_trusted() -> Result<(), String> {
    common::init_tracing();
    let _ = aria2_bin();
    let harness = match Harness::new().await {
        Some(h) => h,
        None => return Ok(()),
    };
    if !harness.wait_for_ready(Duration::from_secs(20)).await {
        dump_events_on_failure(&harness.events, "engine not ready").await;
        harness.shutdown().await;
        return Err("engine never sent EngineReady".into());
    }

    let cert_dir = harness.temp_path().join("certs");
    let https = HttpsFixture::start(harness.fixture_dir.clone(), cert_dir).await;
    let name = "https_smoke_512kb.bin";
    let path = harness.fixture_dir.join(name);
    write_random_fixture(&path, 512 * 1024);
    let expected_sha = sha256_file_sync(&path);

    let mut extra = serde_json::Map::new();
    extra.insert(
        "ca-certificate".into(),
        serde_json::Value::String(https.ca_cert_path.to_string_lossy().into_owned()),
    );
    harness.apply_options(extra).await;

    let url = https.file_url(name);
    let save_dir = harness.download_dir.clone();
    harness
        .add_download(vec![url], save_dir.clone(), 1, Default::default(), false)
        .await;

    let gid = wait_for_added_gid(&harness.events, name, DEFAULT_TEST_TIMEOUT).await?;
    let status = wait_for_terminal(&harness.events, &gid, DEFAULT_TEST_TIMEOUT).await?;
    if status != "complete" {
        dump_events_on_failure(&harness.events, "https status").await;
        https.shutdown().await;
        harness.shutdown().await;
        return Err(format!("https download did not complete: {status}"));
    }

    let downloaded = save_dir.join(name);
    let actual = sha256_file_sync(&downloaded);
    if actual != expected_sha {
        https.shutdown().await;
        harness.shutdown().await;
        return Err(format!(
            "https sha256 mismatch for {}: expected {expected_sha}, got {actual}",
            downloaded.display()
        ));
    }

    https.shutdown().await;
    harness.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(aria2)]
async fn harness_skips_when_aria2_bin_absent() -> Result<(), String> {
    common::init_tracing();
    // SAFETY: #[serial(aria2)] ensures no concurrent env mutation.
    let prev_bin = std::env::var_os("ARIA2_BIN");
    let prev_path = std::env::var_os("PATH");
    unsafe {
        std::env::remove_var("ARIA2_BIN");
        std::env::set_var("PATH", "");
    }
    let result = Harness::new().await;
    unsafe {
        match prev_bin {
            Some(v) => std::env::set_var("ARIA2_BIN", v),
            None => std::env::remove_var("ARIA2_BIN"),
        }
        match prev_path {
            Some(v) => std::env::set_var("PATH", v),
            None => std::env::remove_var("PATH"),
        }
    }
    if result.is_some() {
        if let Some(h) = result {
            h.shutdown().await;
        }
        return Err(
            "Harness::new() should return None when no aria2 binary is discoverable".into(),
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(aria2)]
async fn http_pause_then_resume_completes() -> Result<(), String> {
    common::init_tracing();
    let harness = match Harness::new().await {
        Some(h) => h,
        None => return Ok(()),
    };
    if !harness.wait_for_ready(Duration::from_secs(20)).await {
        harness.shutdown().await;
        return Err("engine not ready".into());
    }

    let http = HttpFixture::start(harness.fixture_dir.clone()).await;
    let name = "pause_resume_2mb.bin";
    write_random_fixture(&harness.fixture_dir.join(name), 2 * 1024 * 1024);

    let save_dir = harness.download_dir.clone();
    harness
        .add_download(vec![http.file_url(name)], save_dir.clone(), 1, Default::default(), false)
        .await;

    let gid = wait_for_added_gid(&harness.events, name, DEFAULT_TEST_TIMEOUT).await?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let paused = harness.pause_and_wait(&gid).await;
    if paused.as_deref() != Some("paused") {
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!("expected paused, got {paused:?}"));
    }
    let resumed = harness.resume_and_wait(&gid).await;
    if resumed.as_deref() != Some("active") && resumed.as_deref() != Some("complete") {
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!("expected active after resume, got {resumed:?}"));
    }
    let status = wait_for_terminal(&harness.events, &gid, DEFAULT_TEST_TIMEOUT).await?;
    if status != "complete" {
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!("download did not complete after pause/resume: {status}"));
    }
    http.shutdown().await;
    harness.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(aria2)]
async fn http_global_speed_limit_throttles() -> Result<(), String> {
    common::init_tracing();
    let harness = match Harness::new().await {
        Some(h) => h,
        None => return Ok(()),
    };
    if !harness.wait_for_ready(Duration::from_secs(20)).await {
        harness.shutdown().await;
        return Err("engine not ready".into());
    }

    // Cap the global download at 200 KB/s BEFORE adding the task.
    let mut extra = serde_json::Map::new();
    extra.insert(
        "max-overall-download-limit".into(),
        serde_json::Value::String("204800".into()),
    );
    harness.apply_options(extra.clone()).await;

    let http = HttpFixture::start(harness.fixture_dir.clone()).await;
    let name = "speed_limit_5mb.bin";
    write_random_fixture(&harness.fixture_dir.join(name), 5 * 1024 * 1024);

    let save_dir = harness.download_dir.clone();
    let start = Instant::now();
    harness
        .add_download(vec![http.file_url(name)], save_dir.clone(), 1, Default::default(), false)
        .await;
    let gid = wait_for_added_gid(&harness.events, name, DEFAULT_TEST_TIMEOUT).await?;
    let status = wait_for_terminal(&harness.events, &gid, DEFAULT_TEST_TIMEOUT).await?;
    let elapsed = start.elapsed();
    // 5 MB at 200 KB/s ≈ 25.6s; allow ±3s of jitter.
    if elapsed < Duration::from_secs(22) {
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!(
            "download finished too fast ({elapsed:?}); speed limit not throttling"
        ));
    }
    if status != "complete" {
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!("download did not complete: {status}"));
    }

    // Reset the limit so subsequent serial tests aren't throttled.
    let mut reset = serde_json::Map::new();
    reset.insert(
        "max-overall-download-limit".into(),
        serde_json::Value::String("0".into()),
    );
    reset.insert(
        "max-overall-upload-limit".into(),
        serde_json::Value::String("0".into()),
    );
    harness.apply_options(reset).await;

    http.shutdown().await;
    harness.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(aria2)]
async fn http_split_creates_multiple_connections() -> Result<(), String> {
    common::init_tracing();
    let harness = match Harness::new().await {
        Some(h) => h,
        None => return Ok(()),
    };
    if !harness.wait_for_ready(Duration::from_secs(20)).await {
        harness.shutdown().await;
        return Err("engine not ready".into());
    }

    let http = HttpFixture::start(harness.fixture_dir.clone()).await;
    // Default min-split-size is 1 MB; with split=8 and a 16 MB file
    // aria2 actually opens multiple connections.
    let name = "split_16mb.bin";
    write_random_fixture(&harness.fixture_dir.join(name), 16 * 1024 * 1024);

    let save_dir = harness.download_dir.clone();
    harness
        .add_download(vec![http.file_url(name)], save_dir.clone(), 8, Default::default(), false)
        .await;

    let gid = wait_for_added_gid(&harness.events, name, DEFAULT_TEST_TIMEOUT).await?;
    // Sample connections over the next few seconds while the download
    // is active. We just need to see `connections > 1` at any point.
    let mut max_seen = 0u64;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        let snap = harness.events.progress(&gid).await;
        if let Some(s) = snap {
            if s.status == "complete" || s.status == "error" {
                break;
            }
            if s.connections > max_seen {
                max_seen = s.connections;
            }
            if max_seen > 1 {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    let status = wait_for_terminal(&harness.events, &gid, DEFAULT_TEST_TIMEOUT).await?;
    if status != "complete" {
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!("download did not complete: {status}"));
    }
    if max_seen < 2 {
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!(
            "expected connections > 1 with split=8, only saw {max_seen}"
        ));
    }
    http.shutdown().await;
    harness.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial(aria2)]
async fn http_multi_source_falls_back_on_5xx() -> Result<(), String> {
    common::init_tracing();
    let harness = match Harness::new().await {
        Some(h) => h,
        None => return Ok(()),
    };
    if !harness.wait_for_ready(Duration::from_secs(20)).await {
        harness.shutdown().await;
        return Err("engine not ready".into());
    }

    let http = HttpFixture::start(harness.fixture_dir.clone()).await;
    let name = "multi_source_512kb.bin";
    write_random_fixture(&harness.fixture_dir.join(name), 512 * 1024);

    let urls = http.flaky_then_normal(name);
    let save_dir = harness.download_dir.clone();
    harness
        .add_download(urls, save_dir.clone(), 1, Default::default(), false)
        .await;

    let gid = wait_for_added_gid(&harness.events, name, DEFAULT_TEST_TIMEOUT).await?;
    let status = wait_for_terminal(&harness.events, &gid, DEFAULT_TEST_TIMEOUT).await?;
    if status != "complete" {
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!("multi-source download did not complete: {status}"));
    }
    let hits = http.flaky_hits.load(std::sync::atomic::Ordering::Relaxed);
    if hits < 2 {
        http.shutdown().await;
        harness.shutdown().await;
        return Err(format!(
            "expected the flaky mirror to be hit at least twice, only saw {hits}"
        ));
    }
    http.shutdown().await;
    harness.shutdown().await;
    Ok(())
}

async fn wait_for_added_gid(
    events: &EventLog,
    name: &str,
    timeout: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let ev = events
            .wait_for(
                |e| matches!(e, EngineEvent::Added { name: n, .. } if n == name),
                remaining,
            )
            .await;
        match ev {
            Some(EngineEvent::Added { gid, .. }) => return Ok(gid),
            Some(_) => continue,
            None => {
                dump_events_on_failure(events, "wait_for_added_gid").await;
                return Err(format!("timeout waiting for Added event for {name}"));
            }
        }
    }
}

async fn wait_for_terminal(
    events: &EventLog,
    gid: &str,
    timeout: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let ev = events
            .wait_for(
                |e| {
                    matches!(e, EngineEvent::Progress { gid: g, status, .. }
                        if g == gid && (status == "complete" || status == "error"))
                },
                remaining,
            )
            .await;
        match ev {
            Some(EngineEvent::Progress { status, .. }) => return Ok(status),
            Some(_) => continue,
            None => {
                dump_events_on_failure(events, "wait_for_terminal").await;
                return Err(format!("timeout waiting for terminal status on {gid}"));
            }
        }
    }
}
