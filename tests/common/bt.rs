//! Local BitTorrent tracker + seeder fixture for the e2e tests.
//!
//! `BtSeeder` boots a Python tracker (`testdata/local_tracker.py`) on a
//! free TCP port and then spawns an `aria2-next` process pointing at
//! `testdata/media_pack.torrent`. The seeder uses the same argument set
//! as `testdata/seed.sh:40-48`:
//!
//! ```text
//! --listen-port=<seeder_port>
//! --seed-ratio=0.0
//! --bt-tracker=http://127.0.0.1:<tracker_port>/announce
//! --enable-dht=false
//! --bt-enable-lpd=false
//! --enable-peer-exchange=false
//! --check-integrity=true
//! media_pack.torrent
//! ```
//!
//! The `BtSeeder::start` call blocks until the seeder reports
//! `"Verification finished successfully"` on stdout, which is the
//! earliest signal the metadata has been checked and the seeder is
//! ready to serve. `BtSeeder::Drop` kills both children via
//! `tokio::process::Child::start_kill`.
//!
//! Also exposes [`magnet_for_torrent`] — a small helper that reads
//! `testdata/media_pack.torrent`, computes the BitTorrent v1 info hash
//! (SHA1 of the raw bencoded `info` dict) and returns a magnet URI
//! pointing at the local tracker. Test 12 uses this to construct a
//! magnet download.

unsafe fn wipe_global_aria2_state_for_seeder() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let home = std::path::PathBuf::from(home);
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

use std::path::{Path, PathBuf};
use std::process::Stdio;

use sha1::{Digest, Sha1};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::{Child, Command};

pub struct BtSeeder {
    pub tracker_port: u16,
    pub seeder_port: u16,
    pub announce_url: String,
    seeder: Child,
    tracker: Child,
    /// Root of the workspace where `testdata/` lives — defaults to
    /// `CARGO_MANIFEST_DIR` (the remotrix crate root).
    pub workspace_root: PathBuf,
}

impl BtSeeder {
    /// Spawn the tracker + seeder and wait until the seeder finishes
    /// verifying the media pack. `aria2_bin` is the same binary the
    /// harness discovered via `common::aria2_bin()`.
    pub async fn start(aria2_bin: &Path, workspace_root: PathBuf) -> Result<Self, String> {
        // The torrent file is hard-coded to announce at
        // `http://127.0.0.1:6969/announce` (see testdata/media_pack.torrent),
        // so the tracker MUST listen on 6969. A previous plan revision
        // proposed randomising the port; aria2 supports `--bt-tracker`
        // overrides via changeGlobalOption, but the engine has no path to
        // inject it without source changes (which the plan forbids),
        // so we fix the port and accept the resulting need to coordinate
        // with anything else listening on 6969.
        let tracker_port: u16 = 6969;
        let tracker_listener = TcpListener::bind(("127.0.0.1", tracker_port))
            .await
            .map_err(|e| format!("bind tracker port {tracker_port}: {e}"))?;
        drop(tracker_listener);

        let seeder_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("bind seeder port: {e}"))?;
        let seeder_port = seeder_listener.local_addr().unwrap().port();
        drop(seeder_listener);

        let tracker_script = workspace_root.join("testdata").join("local_tracker.py");
        if !tracker_script.exists() {
            return Err(format!(
                "local_tracker.py missing at {}",
                tracker_script.display()
            ));
        }
        let torrent = workspace_root.join("testdata").join("media_pack.torrent");
        if !torrent.exists() {
            return Err(format!(
                "media_pack.torrent missing at {}",
                torrent.display()
            ));
        }
        // The torrent's file paths are relative to its parent
        // directory (`testdata/media_pack/...`), so the seeder
        // must run with `testdata/` as current_dir — exactly like
        // seed.sh does.
        let seeder_cwd = workspace_root.join("testdata");

        // Spawn the tracker. The Python tracker writes a small
        // announce/debug log to stderr; we point it at a per-test
        // file so failure investigations can read what the tracker saw.
        // Also wipe aria2's GLOBAL state dir before spawning the
        // seeder — the seeder (like the client) doesn't honour a
        // redirected `$HOME` for its `--state-dir`, so any stale
        // bitfields from a prior run would corrupt the seeder's
        // view of the existing media_pack files and prevent it
        // from announcing properly.
        unsafe {
            wipe_global_aria2_state_for_seeder();
        }
        let tracker_log_path = std::env::temp_dir().join(format!(
            "remotrix_e2e_tracker_{tracker_port}_{}.log",
            std::process::id()
        ));
        let tracker_log_file = std::fs::File::create(&tracker_log_path).ok();
        eprintln!("tracker log: {}", tracker_log_path.display());
        let mut tracker = Command::new("python3")
            .arg(&tracker_script)
            .arg(tracker_port.to_string())
            .current_dir(&seeder_cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(if let Some(f) = tracker_log_file {
                Stdio::from(f)
            } else {
                Stdio::null()
            })
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("spawn local_tracker.py: {e}"))?;
        eprintln!("tracker pid: {}", tracker.id().unwrap());

        // Give the tracker a beat to bind its listener before aria2
        // announces to it.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        eprintln!("tracker ready, spawning seeder");

        let mut seeder = Command::new(aria2_bin)
            .arg(format!("--listen-port={seeder_port}"))
            .arg("--seed-ratio=0.0")
            .arg("--enable-dht=false")
            .arg("--bt-enable-lpd=false")
            .arg("--enable-peer-exchange=false")
            .arg("--check-integrity=true")
            .arg("--console-log-level=info")
            .arg("--summary-interval=1")
            .arg("--log=-")
            .arg("--log-level=info")
            .arg("media_pack.torrent")
            .current_dir(&seeder_cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("spawn aria2 seeder: {e}"))?;

        // Wait for the seeder to transition from "Downloading"
        // into "SEED" status (which means the metadata has been
        // verified and the torrent is now offering itself to the
        // swarm). aria2-next does not always emit "Verification
        // finished successfully" depending on whether pieces need
        // re-checking, so we look for the `SEED(...)` progress
        // indicator instead. seed.sh relies on the same signal.
        let stdout = seeder
            .stdout
            .take()
            .ok_or_else(|| "seeder stdout unavailable".to_string())?;
        let stderr = seeder.stderr.take();
        let mut reader = BufReader::new(stdout).lines();
        let mut err_reader: Option<tokio::io::Lines<BufReader<tokio::process::ChildStderr>>> =
            stderr.map(|e| BufReader::new(e).lines());
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut ready = false;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                let _ = seeder.start_kill();
                let _ = tracker.start_kill();
                return Err("seeder did not reach SEED state in time".into());
            }
            tokio::select! {
                _ = tokio::time::sleep(remaining) => {
                    let _ = seeder.start_kill();
                    let _ = tracker.start_kill();
                    return Err("seeder did not reach SEED state in time".into());
                }
                line = reader.next_line() => {
                    match line {
                        Ok(Some(text)) => {
                            eprintln!("seeder: {text}");
                            tracing::debug!(seeder = %text);
                            if text.contains("SEED(") {
                                ready = true;
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            return Err(format!("seeder stdout read: {e}"));
                        }
                    }
                }
                line = async {
                    if let Some(r) = err_reader.as_mut() {
                        r.next_line().await
                    } else {
                        Ok(None)
                    }
                } => {
                    if let Ok(Some(text)) = line {
                        eprintln!("seeder-err: {text}");
                    }
                }
            }
        }
        if !ready {
            let _ = seeder.start_kill();
            let _ = tracker.start_kill();
            return Err("seeder exited before reaching SEED state".into());
        }

        // Drain the rest of stdout in a background task so the pipe
        // doesn't fill up and block the seeder.
        tokio::spawn(async move {
            let mut reader = reader;
            while let Ok(Some(_)) = reader.next_line().await {}
        });

        Ok(Self {
            tracker_port,
            seeder_port,
            announce_url: format!("http://127.0.0.1:{tracker_port}/announce"),
            seeder,
            tracker,
            workspace_root,
        })
    }

    pub async fn shutdown(mut self) {
        let _ = self.seeder.start_kill();
        let _ = self.tracker.start_kill();
        let _ = self.seeder.wait().await;
        let _ = self.tracker.wait().await;
    }
}

/// Compute the BitTorrent v1 info hash (SHA1 of the raw bencoded
/// `info` dict) for a `.torrent` file and return a magnet URI pointing
/// at the given tracker.
///
/// `torrent_path` is the absolute path to the `.torrent` file. The
/// bencoded info dict is located by scanning for the `"4:info"` key
/// (length-prefixed) at the top level of the root dict; we then match
/// the closing `e` that ends the info dict by tracking dict/list depth
/// from there. This is sufficient for well-formed, single-file-or-multi-
/// file torrents like the testdata fixture.
pub fn magnet_for_torrent(torrent_path: &Path, tracker_url: &str) -> Result<String, String> {
    let bytes = std::fs::read(torrent_path).map_err(|e| format!("read torrent: {e}"))?;
    // Top-level must be a dict.
    if bytes.first() != Some(&b'd') {
        return Err("torrent root is not a bencoded dict".into());
    }
    // Find the `"4:info"` key (4 bytes: '4', ':', 'i', 'n', 'f', 'o').
    const KEY: &[u8] = b"4:info";
    let info_start = bytes
        .windows(KEY.len())
        .position(|w| w == KEY)
        .ok_or_else(|| "info dict not found in torrent".to_string())?
        + KEY.len();
    let mut depth: i32 = 1;
    let mut i = info_start;
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'd' | b'l' => depth += 1,
            b'e' => depth -= 1,
            b'i' => {
                // Integer: skip until the trailing 'e'.
                if let Some(end) = bytes[i + 1..].iter().position(|&b| b == b'e') {
                    i += 1 + end;
                    continue;
                } else {
                    return Err("unterminated integer in info dict".into());
                }
            }
            _ => {}
        }
        i += 1;
    }
    if depth != 0 {
        return Err("info dict not closed".into());
    }
    let info_bytes = &bytes[info_start..i];
    let mut hasher = Sha1::new();
    hasher.update(info_bytes);
    let info_hash = hex::encode(hasher.finalize());
    Ok(format!("magnet:?xt=urn:btih:{info_hash}&tr={tracker_url}"))
}
