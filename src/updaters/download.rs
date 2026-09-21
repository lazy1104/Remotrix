use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

/// Throttle window between progress emissions (~5 Hz).
const PROGRESS_THROTTLE: Duration = Duration::from_millis(200);

/// Time between transient-error retries (500 ms / 1 s / 2 s).
const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
];

/// Progress callback fired with (`downloaded`, `total`) bytes. `total`
/// may be `0` when the server omits `Content-Length` — callers should
/// treat `0` as "unknown" rather than dividing by it.
pub type ProgressFn = Arc<dyn Fn(u64, u64) + Send + Sync>;

/// Options that influence the download behaviour. Defaults to no proxy,
/// no hash verification, no progress.
pub struct DownloadOpts {
    pub proxy: Option<String>,
    pub sha256: Option<String>,
    pub on_progress: Option<ProgressFn>,
}

impl Default for DownloadOpts {
    fn default() -> Self {
        Self {
            proxy: None,
            sha256: None,
            on_progress: None,
        }
    }
}

/// Download `url` to `dest` via a sibling `{dest}.part`, returning once the
/// part file has been renamed into place. Shared behaviour:
///
/// - HTTP 200 discards any pre-existing `.part` and starts over.
/// - HTTP 206 with `Range: bytes=N-` appends to an existing `.part` of the
///   same length, restoring partial progress between attempts.
/// - The download body is hashed incrementally; a sha256 mismatch
///   short-circuits with an error and removes the `.part`.
/// - Progress is throttled to ~5 Hz and a final `EOF` sample is always
///   emitted so the UI never stalls on the last frame.
/// - Transient errors (network / timeout / 5xx) retry up to
///   `RETRY_DELAYS.len()` times; 4xx and hash mismatches do not retry.
/// - Synchronous filesystem work (`File::create`, `write_all`, `rename`,
///   `set_permissions`) runs on a blocking thread so the async runtime is
///   never starved.
///
/// # Errors
/// Returns a string error describing the failure mode (network, HTTP
/// status, I/O, or sha256 mismatch).
pub async fn download(url: &str, dest: &Path, opts: &DownloadOpts) -> Result<(), String> {
    let part_path = part_path_for(dest);

    let builder = crate::config::apply_proxy(
        reqwest::Client::builder()
            .user_agent("remotrix-updater")
            .timeout(Duration::from_secs(60)),
        opts.proxy.as_deref(),
    )
    .map_err(|e| format!("proxy: {e}"))?;
    let client = Arc::new(
        builder
            .build()
            .map_err(|e| format!("create download client: {e}"))?,
    );

    let dest_owned = dest.to_path_buf();
    let part_owned = part_path.clone();
    let expected_sha = opts.sha256.clone();
    let progress_arc: Option<Arc<ProgressState>> = opts
        .on_progress
        .as_ref()
        .map(|f| Arc::new(ProgressState::new(f.clone())));

    let mut attempt = 0usize;
    loop {
        let resume_offset: u64 = if part_owned.exists() {
            tokio::fs::metadata(&part_owned)
                .await
                .map_err(|e| format!("stat .part: {e}"))?
                .len()
        } else {
            0
        };

        let client_for_attempt = client.clone();
        let url_owned = url.to_string();
        let dest_for_attempt = dest_owned.clone();
        let part_for_attempt = part_owned.clone();
        let progress_for_attempt = progress_arc.clone();
        let expected_for_attempt = expected_sha.clone();

        let attempt_result = tokio::task::spawn_blocking(move || {
            run_attempt(
                client_for_attempt.as_ref(),
                &url_owned,
                &dest_for_attempt,
                &part_for_attempt,
                resume_offset,
                expected_for_attempt.as_deref(),
                progress_for_attempt.as_ref(),
            )
        })
        .await;

        let result = match attempt_result {
            Ok(r) => r,
            Err(e) => Err(format!("download join: {e}")),
        };

        match result {
            Ok(()) => return Ok(()),
            Err(e) if is_transient(&e) && attempt < RETRY_DELAYS.len() => {
                let delay = RETRY_DELAYS[attempt];
                tracing::warn!(
                    %url,
                    attempt = attempt + 1,
                    error = %e,
                    "transient download error, retrying"
                );
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

fn part_path_for(dest: &Path) -> PathBuf {
    let mut s = dest.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

struct ProgressState {
    cb: Arc<dyn Fn(u64, u64) + Send + Sync>,
    last_emit: Mutex<Instant>,
    last_seen_down: AtomicU64,
    last_seen_total: AtomicU64,
    has_eof: AtomicBool,
}

impl ProgressState {
    fn new(cb: Arc<dyn Fn(u64, u64) + Send + Sync>) -> Self {
        Self {
            cb,
            last_emit: Mutex::new(Instant::now() - PROGRESS_THROTTLE),
            last_seen_down: AtomicU64::new(0),
            last_seen_total: AtomicU64::new(0),
            has_eof: AtomicBool::new(false),
        }
    }

    fn emit_throttled(&self, downloaded: u64, total: u64) {
        if self.has_eof.load(Ordering::Acquire) {
            return;
        }
        let now = Instant::now();
        let last = {
            let Ok(g) = self.last_emit.lock() else {
                return;
            };
            *g
        };
        if now.duration_since(last) >= PROGRESS_THROTTLE {
            (self.cb)(downloaded, total);
            self.last_seen_down.store(downloaded, Ordering::Relaxed);
            self.last_seen_total.store(total, Ordering::Relaxed);
            if let Ok(mut g) = self.last_emit.lock() {
                *g = now;
            }
        }
    }

    fn emit_eof(&self, downloaded: u64, total: u64) {
        if self.has_eof.swap(true, Ordering::AcqRel) {
            return;
        }
        (self.cb)(downloaded, total);
        self.last_seen_down.store(downloaded, Ordering::Relaxed);
        self.last_seen_total.store(total, Ordering::Relaxed);
        if let Ok(mut g) = self.last_emit.lock() {
            *g = Instant::now();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_attempt(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    part: &Path,
    resume_offset: u64,
    expected_sha: Option<&str>,
    progress: Option<&Arc<ProgressState>>,
) -> Result<(), String> {
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("invalid download destination parent")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("create download dir: {e}"))?;

    let mut request = client.get(url);
    if resume_offset > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={resume_offset}-"));
    }

    let response = tokio::runtime::Handle::current()
        .block_on(request.send())
        .map_err(|e| format!("download request: {e}"))?;

    let status = response.status();
    let total: u64 = response.content_length().unwrap_or(0);
    let mut hasher = expected_sha.map(|_| Sha256::new());

    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        read_chunks_sync(
            response,
            part,
            resume_offset,
            hasher.as_mut(),
            progress,
            total,
        )?;
    } else if status.is_success() {
        if resume_offset > 0 {
            let _ = std::fs::remove_file(part);
        }
        read_chunks_sync(response, part, 0, hasher.as_mut(), progress, total)?;
    } else {
        return Err(format!("HTTP {status}"));
    }

    if let Some(expected) = expected_sha {
        let digest = sha256_file(part)?;
        if digest != expected {
            let _ = std::fs::remove_file(part);
            return Err(format!(
                "sha256 mismatch: expected {expected}, got {digest}"
            ));
        }
    }

    std::fs::rename(part, dest).map_err(|e| format!("rename: {e}"))?;
    set_perms(dest)?;
    if let Some(p) = progress {
        p.emit_eof(u64::MAX, total);
    }
    Ok(())
}

fn read_chunks_sync(
    mut response: reqwest::Response,
    part: &Path,
    initial_offset: u64,
    mut hasher: Option<&mut Sha256>,
    progress: Option<&Arc<ProgressState>>,
    total: u64,
) -> Result<(), String> {
    use std::io::Write;

    let mut file = if initial_offset > 0 {
        std::fs::OpenOptions::new()
            .append(true)
            .open(part)
            .map_err(|e| format!("open .part for append: {e}"))?
    } else {
        std::fs::File::create(part).map_err(|e| format!("create .part: {e}"))?
    };

    let mut downloaded: u64 = initial_offset;
    loop {
        let chunk = tokio::runtime::Handle::current()
            .block_on(response.chunk())
            .map_err(|e| format!("read body: {e}"))?;
        let Some(chunk) = chunk else { break };
        file.write_all(&chunk)
            .map_err(|e| format!("write .part: {e}"))?;
        if let Some(h) = hasher.as_deref_mut() {
            h.update(&chunk);
        }
        downloaded += chunk.len() as u64;
        if let Some(p) = progress {
            p.emit_throttled(downloaded, total);
        }
    }
    file.flush().map_err(|e| format!("flush .part: {e}"))?;
    Ok(())
}

fn is_transient(error: &str) -> bool {
    if let Some(code) = error.strip_prefix("HTTP ") {
        let status_str = code.split_whitespace().next().unwrap_or("");
        if let Ok(status) = status_str.parse::<u16>() {
            return status >= 500;
        }
        return false;
    }
    matches!(
        error,
        s if s.starts_with("download request") || s.starts_with("read body")
    )
}

/// Drive a single download for the app-update path: builds the right
/// destination, runs the shared download with throttled progress, and
/// returns an `AppUpdateOutcome` ready for the apply step. Used by the
/// update dialog when the user accepts an available app offer.
///
/// Returns `Err` if the download could not be resolved or verified; on
/// success the caller emits `AppUpdateReady` to surface a sticky
/// "click to apply" toast, and the file is left on disk as the
/// pending-update evidence.
pub async fn perform_app_update_download(
    url: String,
    dest: PathBuf,
    proxy: Option<String>,
    sha256: Option<String>,
    _version: String,
    kind: crate::app_updater::InstallKind,
    _asset_name: Option<String>,
    dest_for_outcome: PathBuf,
) -> Result<crate::app_updater::AppUpdateOutcome, String> {
    let opts = DownloadOpts {
        proxy,
        sha256,
        on_progress: None,
    };
    download(&url, &dest, &opts).await?;
    Ok(crate::app_updater::AppUpdateOutcome {
        kind,
        path: Some(dest_for_outcome),
    })
}

pub(crate) fn sha256_file(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| format!("open file for sha256: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read file for sha256: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub(crate) fn set_perms(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("chmod: {e}"))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_path_appends_suffix() {
        let p = part_path_for(Path::new("/tmp/x.deb"));
        assert_eq!(p, PathBuf::from("/tmp/x.deb.part"));
    }

    #[test]
    fn transient_recognises_5xx_and_network() {
        assert!(is_transient("HTTP 502 Bad Gateway"));
        assert!(is_transient("HTTP 503 Service Unavailable"));
        assert!(is_transient("download request: timeout"));
        assert!(is_transient("read body: connection reset"));
        assert!(!is_transient("HTTP 404 Not Found"));
        assert!(!is_transient("sha256 mismatch: ..."));
    }
}
