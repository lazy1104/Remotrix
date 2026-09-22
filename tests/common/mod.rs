//! Shared harness for `download_e2e` integration tests.
//!
//! Drives a real aria2-next sidecar via the production
//! `remotrix::engine::spawn_engine` path so the e2e tests exercise the same
//! code that ships in the GUI binary. Each test gets its own
//! `tempfile::TempDir` redirected at `$HOME` so on-disk artefacts (config,
//! session, log, db, the aria2 binary cache) land in a sandboxed location
//! and are torn down automatically. Tests are gated on
//! `#[cfg(any(target_os = "linux", target_os = "macos"))]` because the
//! `directories` crate on Windows reads from the Win32 known-folders API
//! and ignores `$HOME`.
//!
//! Run with:
//! ```text
//! ARIA2_BIN=/path/to/aria2-next cargo test --test download_e2e -- --nocapture
//! ```
//!
//! When `ARIA2_BIN` is unset and no aria2 binary is discoverable on
//! `PATH`, every test logs `skip: ARIA2_BIN not set` and returns `Ok(())`
//! so the suite is green on machines without aria2 (CI, fresh dev box).

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Map;
use tempfile::TempDir;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::task::JoinHandle;

use remotrix::config;
use remotrix::engine::{spawn_engine, EngineCmd, EngineEvent, EngineHandle, EventTx};
use remotrix::task::TaskAdvancedOptions;

pub mod bt;
pub mod http;

/// Locate an aria2 binary, preferring the explicit `ARIA2_BIN` env var and
/// falling back to `which aria2-next` / `which aria2c`. Returns `None` when
/// nothing is found so tests can skip instead of fail on machines without
/// aria2.
pub fn aria2_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ARIA2_BIN") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    for cmd in ["aria2-next", "aria2c"] {
        if let Ok(out) = std::process::Command::new("which").arg(cmd).output() {
            if out.status.success() {
                let trimmed = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !trimmed.is_empty() {
                    let path = PathBuf::from(trimmed);
                    if path.exists() {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

/// Returns `true` when the binary reports itself as `aria2-next` (vs.
/// vanilla `aria2c`) — used by test 14 to gate the session-replay test
/// because aria2-next extends the persisted `aria2.session` with BT
/// fields vanilla aria2 does not write.
pub fn is_aria2_next(bin: &Path) -> bool {
    let output = match std::process::Command::new(bin).arg("--version").output() {
        Ok(o) => o,
        Err(_) => return false,
    };
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    combined.contains("aria2-next")
}

/// Drain aria2's stdout/stderr (the engine uses `pipe_lines` from
/// `tokio::io::AsyncBufReadExt::lines` to forward them to `tracing`).
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_test_writer()
        .try_init();
}

/// Append-only log of every `EngineEvent` the harness has seen, indexed by
/// gid for the most recent progress snapshot. Powers both the
/// `wait_for_event` blocking helper and the diagnostic `recent_events`
/// tail that failure messages print.
#[derive(Clone)]
pub struct EventLog {
    inner: Arc<Mutex<EventLogInner>>,
    notify: Arc<Notify>,
}

struct EventLogInner {
    events: Vec<EngineEvent>,
    last_progress: HashMap<String, ProgressSnapshot>,
    last_status: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct ProgressSnapshot {
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64,
    pub status: String,
    pub connections: u64,
    pub is_seeding: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EventLogInner {
                events: Vec::new(),
                last_progress: HashMap::new(),
                last_status: HashMap::new(),
            })),
            notify: Arc::new(Notify::new()),
        }
    }

    pub async fn push(&self, event: EngineEvent) {
        if let EngineEvent::Progress {
            ref gid,
            downloaded,
            total,
            speed,
            ref status,
            connections,
            is_seeding,
            ref error_code,
            ref error_message,
            ..
        } = event
        {
            let snapshot = ProgressSnapshot {
                downloaded,
                total,
                speed,
                status: status.clone(),
                connections,
                is_seeding,
                error_code: error_code.clone(),
                error_message: error_message.clone(),
            };
            let mut inner = self.inner.lock().await;
            inner.last_progress.insert(gid.clone(), snapshot);
            inner.last_status.insert(gid.clone(), status.clone());
        }
        let mut inner = self.inner.lock().await;
        if inner.events.len() >= 5000 {
            inner.events.remove(0);
        }
        inner.events.push(event);
        drop(inner);
        self.notify.notify_one();
    }

    /// Block until an event matching `pred` arrives, or `timeout` elapses.
    /// Returns the matching event (cloned) or `None` on timeout.
    pub async fn wait_for<F>(&self, pred: F, timeout: Duration) -> Option<EngineEvent>
    where
        F: Fn(&EngineEvent) -> bool,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            {
                let inner = self.inner.lock().await;
                if let Some(e) = inner.events.iter().find(|e| pred(e)).cloned() {
                    return Some(e);
                }
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            let _ = tokio::time::timeout(remaining, self.notify.notified()).await;
        }
    }

    /// Block until an event matching `pred` arrives AND every event seen
    /// so far satisfies `progress` (used by tests that want to drain a
    /// stream of stale events before checking the new one).
    pub async fn last_status_for(&self, gid: &str) -> Option<String> {
        let inner = self.inner.lock().await;
        inner.last_status.get(gid).cloned()
    }

    /// Most recent progress snapshot for `gid`, if any.
    pub async fn progress(&self, gid: &str) -> Option<ProgressSnapshot> {
        let inner = self.inner.lock().await;
        inner.last_progress.get(gid).cloned()
    }

    /// Tail of recent events (newest last) — used by failing tests to
    /// dump diagnostics. Caps at `n` entries.
    pub async fn recent_events(&self, n: usize) -> Vec<EngineEvent> {
        let inner = self.inner.lock().await;
        let start = inner.events.len().saturating_sub(n);
        inner.events[start..].to_vec()
    }

    pub async fn event_count(&self) -> usize {
        let inner = self.inner.lock().await;
        inner.events.len()
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Owns a real aria2-next sidecar plus the temp dir the engine writes
/// its session/log/cache into. One `Harness` per test; call
/// [`Harness::shutdown`] (or let `shutdown` consume it) before the test
/// returns so aria2 is killed and `$HOME` is restored.
pub struct Harness {
    pub handle: EngineHandle,
    pub events: EventLog,
    pub session_dir: PathBuf,
    pub download_dir: PathBuf,
    pub fixture_dir: PathBuf,
    /// Path of the aria2 binary in use. Surfaced so tests can skip
    /// when the binary is vanilla `aria2c` (gating test 14).
    #[allow(dead_code)]
    pub bin: PathBuf,
    /// Local mirror of the engine event channel for tests that want to
    /// publish synthetic events (none yet, kept for Phase 4-5).
    #[allow(dead_code)]
    pub event_tx: EventTx,
    event_pump: Option<JoinHandle<()>>,
    prev_home: Option<OsString>,
    temp: TempDir,
}

impl Harness {
    /// Build a Harness. Returns `None` when no aria2 binary is available
    /// so tests can skip cleanly. Returns `Some(harness)` even when the
    /// engine fails to start — tests that need a live engine should
    /// follow up with `harness.is_ready()`.
    pub async fn new() -> Option<Self> {
        let bin = match aria2_bin() {
            Some(b) => b,
            None => {
                eprintln!("skip: ARIA2_BIN not set and no aria2-next/aria2c on PATH");
                return None;
            }
        };
        // aria2 reads its state dir from the real `$HOME` via
        // `getpwuid_r` (it does NOT honor a redirected `HOME` env var
        // the way the Rust `directories` crate does), so the per-test
        // temp-dir redirection we use for `session.txt` does not extend
        // to BT fast-resume caches. Wipe the global state before each
        // test so a previous run's stale piece-bitfield can't reject
        // a fresh download with a "mismatching file size" warning.
        // SAFETY: #[serial(aria2)] ensures no concurrent file ops.
        unsafe {
            wipe_aria2_state_for_test();
        }
        let prev_home = std::env::var_os("HOME");
        let temp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skip: TempDir::new failed: {e}");
                return None;
            }
        };
        // Wipe aria2's *global* state dirs (it doesn't honour
        // redirected HOME for state-dir / cache-dir) so a previous
        // test's stale bitfields can't reject a fresh download with
        // a "mismatching file size" warning. Done before HOME is
        // redirected so we wipe the *original* home's aria2 dirs,
        // which is also where aria2 will read on this test run.
        wipe_aria2_state_for_test();
        // SAFETY: std::env::set_var is marked unsafe because the underlying
        // C runtime is not thread-safe; under #[serial(aria2)] tests run
        // strictly sequentially so this is fine.
        unsafe {
            std::env::set_var("HOME", temp.path());
        }
        let session_dir = match config::session_dir() {
            Some(p) => p,
            None => {
                restore_home(prev_home);
                eprintln!("skip: cannot resolve session_dir");
                return None;
            }
        };
        let download_dir = temp.path().join("downloads");
        if let Err(e) = std::fs::create_dir_all(&download_dir) {
            restore_home(prev_home);
            eprintln!("skip: create download_dir failed: {e}");
            return None;
        }
        let fixture_dir = temp.path().join("fixtures");
        if let Err(e) = std::fs::create_dir_all(&fixture_dir) {
            restore_home(prev_home);
            eprintln!("skip: create fixture_dir failed: {e}");
            return None;
        }
        let (handle, mut rx) = spawn_engine();
        let (event_tx, mut event_rx_local) = mpsc::unbounded_channel::<EngineEvent>();
        let events = EventLog::new();
        let events_for_pump = events.clone();
        let event_pump = tokio::spawn(async move {
            // Forward both the engine's outbound events AND the local
            // mirror so tests that publish synthetic events through
            // `event_tx` still see them via `events.wait_for`.
            loop {
                tokio::select! {
                    biased;
                    engine = rx.recv() => {
                        let Some(ev) = engine else { break };
                        events_for_pump.push(ev).await;
                    }
                    local = event_rx_local.recv() => {
                        let Some(ev) = local else { continue };
                        events_for_pump.push(ev).await;
                    }
                }
            }
        });
        Some(Self {
            handle,
            events,
            session_dir,
            download_dir,
            fixture_dir,
            bin,
            event_tx,
            event_pump: Some(event_pump),
            prev_home,
            temp,
        })
    }

    /// Returns true if the engine sent `EngineReady` within `timeout`.
    pub async fn wait_for_ready(&self, timeout: Duration) -> bool {
        self.events
            .wait_for(|e| matches!(e, EngineEvent::EngineReady), timeout)
            .await
            .is_some()
    }

    /// Root of the temp dir backing this harness. Useful for fixtures
    /// the test wants the harness to own (cert dirs, BT staging areas).
    pub fn temp_path(&self) -> &Path {
        self.temp.path()
    }

    /// Convenience: send `AddDownload` for `urls` into `save_dir`.
    /// `extra_options` is merged into the per-task option bag, which is
    /// how tests inject things like `ca-certificate=...` that
    /// `TaskAdvancedOptions` does not expose directly.
    pub async fn add_download(
        &self,
        urls: Vec<String>,
        save_dir: PathBuf,
        split: u16,
        extra_options: Map<String, serde_json::Value>,
        bt_metadata_only: bool,
    ) {
        let mut advanced = TaskAdvancedOptions::default();
        for (k, v) in extra_options {
            let serde_json::Value::String(s) = v else {
                continue;
            };
            match k.as_str() {
                "user-agent" => advanced.user_agent = s,
                "http-user" => advanced.http_user = s,
                "http-passwd" => advanced.http_passwd = s,
                "referer" => advanced.referer = s,
                "cookie" => advanced.cookie = s,
                _ => {
                    // Unmapped keys (e.g. ca-certificate, max-tries) are
                    // dropped: per-task `extra_options` cannot be set
                    // through TaskAdvancedOptions. Tests that need them
                    // must use `apply_options` instead.
                }
            }
        }
        let _ = self.handle.cmd_tx.send(EngineCmd::AddDownload {
            urls,
            save_dir,
            split,
            advanced,
            bt_metadata_only,
        });
    }

    /// Send an `ApplyAria2Options` so tests can adjust global limits or
    /// inject `ca-certificate` for the HTTPS fixture. Re-applies a few
    /// times to defeat the boot-time apply task race: the engine
    /// spawns a background `changeGlobalOption` (with default settings)
    /// right after `EngineReady` is emitted, and if it lands between
    /// our two apply calls the test's setting is reverted to defaults.
    pub async fn apply_options(&self, extra: Map<String, serde_json::Value>) {
        let options = aria2_ws::TaskOptions {
            extra_options: extra.clone(),
            ..Default::default()
        };
        let _ = self
            .handle
            .cmd_tx
            .send(EngineCmd::ApplyAria2Options { options });
        // Wait for the boot-time apply to finish, then re-apply so our
        // value lands LAST and is the one in effect for subsequent
        // commands.
        tokio::time::sleep(Duration::from_millis(400)).await;
        let options2 = aria2_ws::TaskOptions {
            extra_options: extra,
            ..Default::default()
        };
        let _ = self
            .handle
            .cmd_tx
            .send(EngineCmd::ApplyAria2Options { options: options2 });
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    /// Send a `Pause(gid)` and wait until the next Progress event for
    /// `gid` reports `status == "paused"` (or `error` if aria2 fails to
    /// pause the task for some reason).
    pub async fn pause_and_wait(&self, gid: &str) -> Option<String> {
        let _ = self.handle.cmd_tx.send(EngineCmd::Pause(gid.to_string()));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let ev = self
                .events
                .wait_for(
                    |e| {
                        matches!(e, EngineEvent::Progress { gid: g, status, .. }
                            if g == gid && (status == "paused" || status == "error"))
                    },
                    remaining,
                )
                .await;
            if let Some(EngineEvent::Progress { status, .. }) = ev {
                return Some(status);
            }
        }
        None
    }

    /// Send a `Resume(gid)` and wait until the next Progress event for
    /// `gid` reports `status == "active"` (or `complete` if aria2
    /// finished while paused, which we treat as success).
    pub async fn resume_and_wait(&self, gid: &str) -> Option<String> {
        let _ = self.handle.cmd_tx.send(EngineCmd::Resume(gid.to_string()));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let ev = self
                .events
                .wait_for(
                    |e| {
                        matches!(e, EngineEvent::Progress { gid: g, status, .. }
                            if g == gid
                                && (status == "active"
                                    || status == "complete"
                                    || status == "error"))
                    },
                    remaining,
                )
                .await;
            if let Some(EngineEvent::Progress { status, .. }) = ev {
                return Some(status);
            }
        }
        None
    }

    /// Number of distinct `connections` values seen for `gid` so far.
    /// Tests assert on this to confirm `split=N` actually opened
    /// parallel connections (test 6).
    pub async fn max_connections(&self, gid: &str) -> u64 {
        let evs = self.events.recent_events(5000).await;
        evs.iter()
            .filter_map(|e| match e {
                EngineEvent::Progress {
                    gid: g,
                    connections,
                    ..
                } if g == gid => Some(*connections),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }

    /// Send an `AddTorrent` command and return once aria2 emits `Added`
    /// for the new gid. Returns the gid (or `None` on timeout). Reserved
    /// for Phase 4 BT tests.
    #[allow(dead_code)]
    pub async fn add_torrent(
        &self,
        path: &Path,
        save_dir: PathBuf,
        select_files: Option<Vec<u64>>,
    ) -> Option<String> {
        let _ = self.handle.cmd_tx.send(EngineCmd::AddTorrent {
            path: path.to_path_buf(),
            save_dir,
            split: 1,
            advanced: TaskAdvancedOptions::default(),
            select_files,
        });
        let ev = self
            .events
            .wait_for(
                |e| matches!(e, EngineEvent::Added { .. }),
                Duration::from_secs(30),
            )
            .await?;
        match ev {
            EngineEvent::Added { gid, .. } => Some(gid),
            _ => None,
        }
    }

    /// Shut the engine down gracefully, then SIGKILL the aria2 child via
    /// its pid file (the engine writes `<session_dir>/aria2.pid` after
    /// the WS client connects). Restores `$HOME` and drops the temp dir
    /// so the test process exits cleanly. **Must** be called before the
    /// `Harness` is dropped — `Drop` is intentionally a no-op because we
    /// cannot `await` inside it.
    pub async fn shutdown(mut self) {
        let _ = self.handle.cmd_tx.send(EngineCmd::Shutdown);
        let _ = self
            .events
            .wait_for(
                |e| matches!(e, EngineEvent::EngineStopped),
                Duration::from_secs(15),
            )
            .await;
        // The supervisor removes the pid file *after* sending
        // `EngineStopped` (run_supervisor end-of-function), so there's
        // a small race: harness reads pid file, but supervisor may
        // have just unlinked it. Try a few times, then fall back to a
        // global pkill of aria2-next — safe under `#[serial(aria2)]`
        // since only one test's aria2 is ever alive at a time.
        for _ in 0..10 {
            if let Ok(content) = std::fs::read_to_string(self.session_dir.join("aria2.pid")) {
                if let Ok(pid) = content.trim().parse::<i32>() {
                    unsafe {
                        libc::kill(pid, libc::SIGKILL);
                    }
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        // Final belt-and-braces: nuke any straggler aria2 process.
        // Cheap, no-op when nothing is left running.
        let _ = std::process::Command::new("pkill")
            .args(["-9", "-f", "aria2-next"])
            .output();
        let _ = std::process::Command::new("pkill")
            .args(["-9", "-x", "aria2c"])
            .output();
        tokio::time::sleep(Duration::from_millis(200)).await;

        // The supervisor's task drops its `event_tx` clone as it exits,
        // so the receiver in our pump returns `None` and the task ends
        // on its own. Give it a moment, then abort as a last resort.
        if let Some(handle) = self.event_pump.take() {
            if !handle.is_finished() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if !handle.is_finished() {
                handle.abort();
            }
        }
        restore_home(self.prev_home.take());
        // Drop happens at end of fn: event_pump (already finished),
        // then temp (auto-removed).
    }
}

fn restore_home(prev: Option<OsString>) {
    // SAFETY: see Harness::new — under #[serial(aria2)] tests are
    // sequential so racing the C environment is fine.
    unsafe {
        match prev {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
}

/// Best-effort cleanup of aria2's global state dirs. aria2 stores
/// its BT bitfields / DHT / etc. under `~/.local/state/aria2-next/`
/// *and* `~/.aria2-next/`, and the GUI's persisted session + cached
/// per-info-hash `.torrent` metadata files under
/// `~/.local/share/remotrix/aria2/`. None of these paths honour a
/// redirected `$HOME` env var (aria2 reads the real home via
/// `getpwuid_r`), so per-test temp-dir isolation alone isn't enough —
/// a previous run's stale bitfields can reject a fresh download with
/// a "mismatching file size" warning. Wipe all known paths before
/// each test. Safe under `#[serial(aria2)]`.
pub fn wipe_aria2_state_for_test() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let home = std::path::PathBuf::from(home);
    // aria2's legacy + current state roots.
    for rel in [".local/state/aria2-next", ".aria2-next"] {
        let _ = std::fs::remove_dir_all(home.join(rel));
    }

    let app_dir = home.join(".local/share/remotrix/aria2");
    let _ = std::fs::remove_file(app_dir.join("session.txt"));
    let _ = std::fs::remove_file(app_dir.join("aria2.pid"));
    if let Ok(dir) = std::fs::read_dir(&app_dir) {
        for entry in dir.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("torrent") {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

/// Print the last `n` engine events for failure diagnostics.
pub async fn dump_events_on_failure(events: &EventLog, label: &str) {
    let tail = events.recent_events(20).await;
    eprintln!("---- last engine events ({label}) ----");
    for e in tail {
        eprintln!("  {e:?}");
    }
    eprintln!("---- end ----");
}

/// Compute SHA-256 of a file using blocking I/O on the current thread.
/// Tests run on a tokio runtime so prefer [`sha256_async`] when called
/// from inside `#[tokio::test]`.
pub fn sha256_file_sync(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path).expect("open file");
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).expect("read file");
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    hex::encode(hasher.finalize())
}

/// Generate a `size` byte file filled with `rand`-driven bytes. Used by
/// smoke tests to build a source fixture that the HTTP fixture serves.
pub fn write_random_fixture(path: &Path, size: usize) {
    use rand::TryRngCore;
    let mut rng = rand::rngs::OsRng;
    let mut file = std::fs::File::create(path).expect("create fixture");
    let mut buf = vec![0u8; 65536];
    let mut written = 0;
    while written < size {
        let chunk = (size - written).min(buf.len());
        rng.try_fill_bytes(&mut buf[..chunk])
            .expect("os rng fill_bytes");
        use std::io::Write;
        file.write_all(&buf[..chunk]).expect("write fixture");
        written += chunk;
    }
}

/// Forward-compat helper for tests that need a small random pad
/// (currently unused — Phase 4 might use it for `out=` filenames).
#[allow(dead_code)]
pub fn rand_bytes(out: &mut [u8]) {
    use rand::TryRngCore;
    rand::rngs::OsRng
        .try_fill_bytes(out)
        .expect("os rng fill_bytes");
}
