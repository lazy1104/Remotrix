use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

use aria2_ws::response::TaskStatus as Aria2TaskStatus;
use aria2_ws::{Client, Event, Notification, TaskOptions};
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout, Duration};

use crate::aria2_fetcher;
use crate::task::TaskAdvancedOptions;

static MISSING_CHECK_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

const RPC_TIMEOUT: Duration = Duration::from_secs(3);

const UPDATE_DOWNLOAD_POLL_INTERVAL: Duration = Duration::from_secs(1);
const UPDATE_DOWNLOAD_MAX_WAIT: Duration = Duration::from_secs(600);
const UPDATE_DOWNLOAD_MAX_POLL_FAILURES: u32 = 10;

#[derive(Debug, Clone)]
pub enum EngineCmd {
    AddDownload {
        urls: Vec<String>,
        save_dir: PathBuf,
        split: u16,
        advanced: TaskAdvancedOptions,
        bt_metadata_only: bool,
    },
    AddExternalDownload {
        urls: Vec<String>,
        save_dir: PathBuf,
        split: u16,
        advanced: TaskAdvancedOptions,
        headers: Vec<(String, String)>,
        bt_metadata_only: bool,
    },
    Pause(String),
    Resume(String),
    Remove {
        gid: String,
        delete_files: bool,
    },
    PauseAll,
    ResumeAll,
    RemoveAll {
        delete_files: bool,
    },
    Snapshot,
    PurgeResults(Vec<String>),
    ApplyAria2Options {
        options: TaskOptions,
    },
    AddTorrent {
        path: PathBuf,
        save_dir: PathBuf,
        split: u16,
        advanced: TaskAdvancedOptions,
        select_files: Option<Vec<u64>>,
    },
    FollowTorrent {
        gid: String,
        path: PathBuf,
        save_dir: PathBuf,
        split: u16,
        advanced: TaskAdvancedOptions,
        delete_after: bool,
    },
    SelectFiles {
        gid: String,
        files: Vec<u64>,
    },
    FetchTaskDetails(String),
    FetchTaskAdvanced(String),
    ChangeTaskAdvanced {
        gid: String,
        advanced: TaskAdvancedOptions,
    },
    ReaddTask {
        gid: String,
        url: String,
        save_dir: PathBuf,
        split: u16,
        paused: bool,
        bt_metadata_only: bool,
    },
    Redownload {
        gid: String,
        url: String,
        save_dir: PathBuf,
        split: u16,
        bt_metadata_only: bool,
    },
    Shutdown,
    ForceKill,
    DownloadAria2Update {
        version: String,
        asset_name: String,
        download_url: String,
        sha256: Option<String>,
    },
    DownloadAppUpdate {
        kind: crate::app_updater::InstallKind,
        version: String,
        url: String,
        asset_name: String,
        sha256: Option<String>,
        download_dir: PathBuf,
    },
    RetryAria2Fetch,
    RestartEngine,
    ResumeGids(Vec<String>),
    CheckMissingFiles,
    ReloadSchedules,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    Added {
        gid: String,
        name: String,
        url: String,
        dir: String,
        info_hash: Option<String>,
        advanced: TaskAdvancedOptions,
        from_browser: bool,
    },
    Progress {
        gid: String,
        name: String,
        downloaded: u64,
        total: u64,
        speed: u64,
        upload_speed: u64,
        status: String,
        connections: u64,
        info_hash: Option<String>,
        is_seeding: bool,
        error_code: Option<String>,
        error_message: Option<String>,
    },
    TorrentAdded {
        gid: String,
        path: PathBuf,
    },
    Removed(String),
    TaskDetails {
        gid: String,
        details: crate::task::TaskDetails,
    },
    TaskDetailsFailed {
        gid: String,
    },
    SelectFilesFailed {
        gid: String,
    },
    TaskAdvancedLoaded {
        gid: String,
        options: Box<TaskAdvancedOptions>,
    },
    TaskAdvancedLoadFailed {
        gid: String,
    },
    TaskAdvancedApplied {
        gid: String,
        options: TaskAdvancedOptions,
    },
    TaskAdvancedApplyFailed {
        gid: String,
    },
    EngineReady,
    SyncComplete,
    EngineStopped,
    Aria2Status {
        stage: String,
        message: String,
    },
    Aria2Version {
        version: String,
    },
    Aria2UpdateApplied {
        version: String,
    },
    Aria2UpdateProgress {
        downloaded: u64,
        total: u64,
    },
    Aria2UpdateFailed {
        error: String,
    },
    Aria2FetchFailed {
        error: String,
    },
    GlobalSpeed {
        download: u64,
        upload: u64,
    },
    Aria2UpdateStaged {
        version: String,
    },
    AppUpdateDownloaded {
        kind: crate::app_updater::InstallKind,
        path: Option<PathBuf>,
    },
    AppUpdateDownloadFailed {
        error: String,
    },
    EngineDegraded {
        reason: String,
    },
    FilesMissing {
        gids: Vec<String>,
    },
}

pub type CmdTx = mpsc::UnboundedSender<EngineCmd>;
pub type CmdRx = mpsc::UnboundedReceiver<EngineCmd>;
pub type EventTx = mpsc::UnboundedSender<EngineEvent>;
pub type EventRx = mpsc::UnboundedReceiver<EngineEvent>;

#[derive(Clone)]
pub struct EngineHandle {
    pub cmd_tx: CmdTx,
}

pub fn spawn_engine() -> (EngineHandle, EventRx) {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<EngineCmd>();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<EngineEvent>();

    tokio::spawn(run_supervisor(cmd_rx, event_tx));

    (EngineHandle { cmd_tx }, event_rx)
}

struct Sidecar {
    client: Client,
    child: Option<Child>,
}

struct SidecarConfig {
    session_path: PathBuf,
    download_dir: PathBuf,
}

fn find_free_port_excluding(
    reserved: &std::collections::HashSet<u16>,
) -> Result<u16, std::io::Error> {
    loop {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        if !reserved.contains(&addr.port()) {
            return Ok(addr.port());
        }
    }
}

fn generate_secret() -> String {
    crate::config::generate_secret()
}

fn pipe_lines<R: tokio::io::AsyncRead + Unpin + Send + 'static>(reader: R, target: &'static str) {
    tokio::spawn(async move {
        let reader = tokio::io::BufReader::new(reader);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::debug!(target, "{line}");
        }
    });
}

impl Sidecar {
    async fn spawn(
        bin_path: &Path,
        config: &SidecarConfig,
        event_tx: &EventTx,
    ) -> Result<Self, String> {
        let secret = generate_secret();
        let settings = crate::config::load();
        let reserved_tcp: std::collections::HashSet<u16> =
            crate::port_guard::reserved_tcp_ports(&settings);
        let configured = settings.aria2.rpc_listen_port;
        let port = if configured > 0 && crate::port_guard::tcp_available(configured) {
            configured
        } else if configured > 0 {
            let _ = event_tx.send(EngineEvent::Aria2Status {
                stage: "warning".to_string(),
                message: format!(
                    "RPC port {configured} is occupied; falling back to an auto-assigned port"
                ),
            });
            tracing::warn!(port = configured, "rpc port occupied, falling back to auto");
            find_free_port_excluding(&reserved_tcp).map_err(|e| format!("port allocation: {e}"))?
        } else {
            // Auto-allocated port that explicitly avoids the Extension API and
            // ed2k TCP ports, so the random port can never collide with them.
            find_free_port_excluding(&reserved_tcp).map_err(|e| format!("port allocation: {e}"))?
        };

        let session_file = config.session_path.join("session.txt");
        if !session_file.exists() {
            std::fs::write(&session_file, "").map_err(|e| format!("create session file: {e}"))?;
        }
        let session_str = session_file.to_string_lossy().to_string();

        let dir_str = config.download_dir.to_string_lossy();
        tracing::info!(port, session = %session_str, dir = %dir_str, "spawning aria2-next sidecar");

        let mut cmd = Command::new(bin_path);
        #[cfg(unix)]
        cmd.arg("--stop-with-process")
            .arg(std::process::id().to_string());
        // aria2-next is a console-subsystem binary; without this flag Windows
        // allocates a new console window for it alongside our GUI.
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        for arg in settings.aria2.ed2k_startup_args() {
            cmd.arg(arg);
        }
        if let Some(log_file) = crate::logging::engine_log_path() {
            let level = crate::logging::normalize_engine_level(&settings.log.engine_level);
            cmd.arg("--log").arg(&log_file);
            cmd.arg("--log-level").arg(&level);
            tracing::info!(?log_file, level, "aria2-next log file");
        }
        let mut child = cmd
            .arg("--enable-rpc")
            .arg("--rpc-listen-all=false")
            .arg("--rpc-listen-port")
            .arg(port.to_string())
            .arg("--rpc-secret")
            .arg(&secret)
            .arg("--input-file")
            .arg(&session_str)
            .arg("--save-session")
            .arg(&session_str)
            .arg("--save-session-interval")
            .arg("5")
            .arg("--dir")
            .arg(&config.download_dir)
            .arg("--continue=true")
            .arg("--auto-file-renaming=true")
            .arg("--allow-overwrite=false")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("spawn aria2-next: {e}"))?;

        if let Some(out) = child.stdout.take() {
            pipe_lines(out, "aria2-stdout");
        }
        if let Some(err) = child.stderr.take() {
            pipe_lines(err, "aria2-stderr");
        }

        let ws_url = format!("ws://127.0.0.1:{port}/jsonrpc");
        let mut last_err = String::new();
        for attempt in 1..=10 {
            tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
            match Client::connect(&ws_url, Some(&secret)).await {
                Ok(client) => {
                    tracing::info!(port, "aria2-ws connected");
                    let _ = client.get_version().await.map(|v| {
                        tracing::info!(enabled_features = ?v.enabled_features, version = ?v.version, "aria2-next version");
                    });
                    if let Some(pid) = child.id() {
                        let pid_path = config.session_path.join("aria2.pid");
                        if let Err(e) = std::fs::write(&pid_path, pid.to_string()) {
                            tracing::warn!(?e, "write aria2 pid file failed");
                        }
                    }
                    return Ok(Sidecar {
                        client,
                        child: Some(child),
                    });
                }
                Err(e) => {
                    last_err = format!("connect attempt {attempt}: {e}");
                    tracing::warn!("{last_err}");
                }
            }
        }
        Err(format!(
            "failed to connect aria2-ws after 10 attempts: {last_err}"
        ))
    }
}

fn status_to_string(status: &aria2_ws::response::TaskStatus) -> &'static str {
    match status {
        Aria2TaskStatus::Active => "active",
        Aria2TaskStatus::Waiting => "waiting",
        Aria2TaskStatus::Paused => "paused",
        Aria2TaskStatus::Complete => "complete",
        Aria2TaskStatus::Error => "error",
        Aria2TaskStatus::Removed => "removed",
    }
}

fn basename(uri: &str) -> Option<String> {
    let trimmed = uri.split('?').next().unwrap_or(uri);
    let trimmed = trimmed.trim_end_matches('/');
    let seg = trimmed.rsplit(['/', '\\']).next()?;
    if seg.is_empty() {
        None
    } else {
        Some(seg.to_string())
    }
}

pub(crate) fn is_torrent_url(url: &str) -> bool {
    basename(url)
        .map(|n| n.to_lowercase().ends_with(".torrent"))
        .unwrap_or(false)
}

pub(crate) fn is_magnet_url(url: &str) -> bool {
    url.trim_start().to_ascii_lowercase().starts_with("magnet:")
}

fn apply_bt_url_options(opts: &mut TaskOptions, url: &str, bt_metadata_only: bool) {
    if is_torrent_url(url) {
        opts.extra_options
            .insert("follow-torrent".to_string(), "false".into());
    }
    if bt_metadata_only && is_magnet_url(url) {
        opts.extra_options
            .insert("bt-metadata-only".to_string(), "true".into());
        opts.extra_options
            .insert("bt-save-metadata".to_string(), "true".into());
    }
}

fn collect_file_paths(s: &aria2_ws::response::Status) -> Vec<String> {
    let mut paths = Vec::new();
    for f in &s.files {
        if !f.path.is_empty() {
            paths.push(f.path.clone());
        } else if let Some(uri) = f.uris.first() {
            if let Some(name) = basename(&uri.uri) {
                paths.push(
                    std::path::Path::new(&s.dir)
                        .join(name)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    paths
}

fn name_from_status(s: &aria2_ws::response::Status) -> String {
    if let Some(file) = s.files.first() {
        let path = std::path::Path::new(&file.path);
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if !name.is_empty() {
                return name.to_string();
            }
        }
        if let Some(u) = file.uris.first() {
            if let Some(b) = basename(&u.uri) {
                return b;
            }
        }
    }
    s.gid.clone()
}

async fn emit_added(
    event_tx: &EventTx,
    s: &aria2_ws::response::Status,
    advanced: TaskAdvancedOptions,
) {
    let url = s
        .files
        .first()
        .and_then(|f| f.uris.first())
        .map(|u| u.uri.clone())
        .unwrap_or_default();
    let _ = event_tx.send(EngineEvent::Added {
        gid: s.gid.clone(),
        name: name_from_status(s),
        url,
        dir: s.dir.clone(),
        info_hash: s.info_hash.clone(),
        advanced,
        from_browser: false,
    });
}

async fn emit_progress(event_tx: &EventTx, s: &aria2_ws::response::Status) {
    let _ = event_tx.send(EngineEvent::Progress {
        gid: s.gid.clone(),
        name: name_from_status(s),
        downloaded: s.completed_length,
        total: s.total_length,
        speed: s.download_speed,
        upload_speed: s.upload_speed,
        status: status_to_string(&s.status).to_string(),
        connections: s.connections,
        info_hash: s.info_hash.clone(),
        is_seeding: s.seeder.unwrap_or(false),
        error_code: s.error_code.clone(),
        error_message: s.error_message.clone(),
    });
}

async fn fetch_all_tasks(client: &Client) -> Vec<aria2_ws::response::Status> {
    match fetch_all_tasks_strict(client).await {
        Ok((all, _)) => all,
        Err(e) => {
            tracing::warn!("fetch_all_tasks failed: {e}");
            Vec::new()
        }
    }
}

async fn fetch_all_tasks_strict(
    client: &Client,
) -> Result<(Vec<aria2_ws::response::Status>, bool), String> {
    let active = client
        .tell_active()
        .await
        .map_err(|e| format!("tell_active: {e}"))?;
    let waiting = client
        .tell_waiting(-1, 1000)
        .await
        .map_err(|e| format!("tell_waiting: {e}"))?;
    let stopped = client
        .tell_stopped(-1, 1000)
        .await
        .map_err(|e| format!("tell_stopped: {e}"))?;
    let truncated = waiting.len() >= 1000 || stopped.len() >= 1000;
    let mut all = active;
    all.extend(waiting);
    all.extend(stopped);
    Ok((all, truncated))
}

async fn check_missing_files(client: &Client) -> Vec<String> {
    let probe = client.tell_stopped(0, 1).await.unwrap_or_default();
    if probe.is_empty() {
        return vec![];
    }
    let stopped = client.tell_stopped(-1, 1000).await.unwrap_or_default();
    let mut missing = Vec::new();
    for s in stopped {
        if s.status != Aria2TaskStatus::Complete {
            continue;
        }
        let paths = collect_file_paths(&s);
        if paths.is_empty() {
            continue;
        }
        if paths.iter().all(|p| !path_exists(p)) {
            missing.push(s.gid.clone());
        }
    }
    missing
}

fn path_exists(p: &str) -> bool {
    match std::fs::metadata(p) {
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
        Ok(_) => true,
    }
}

fn url_host(uri: &str) -> Option<String> {
    reqwest::Url::parse(uri)
        .ok()?
        .host_str()
        .map(|h| h.to_ascii_lowercase())
}

async fn sync_existing_tasks(client: &Client, event_tx: &EventTx) -> bool {
    let (all, truncated) = match fetch_all_tasks_strict(client).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("startup sync failed, skipping reconciliation: {e}");
            return false;
        }
    };
    for s in &all {
        emit_added(event_tx, s, TaskAdvancedOptions::default()).await;
        emit_progress(event_tx, s).await;
    }
    if truncated {
        tracing::warn!("startup sync hit result cap, skipping reconciliation");
        return false;
    }
    if all.is_empty() {
        tracing::info!("no existing tasks found during sync");
    } else {
        tracing::info!("synced {} existing tasks", all.len());
    }
    true
}

async fn remove_task_from_aria2(client: &Client, gid: &str) {
    if client.force_remove(gid).await.is_err() {
        let _ = client.remove(gid).await;
    }
    let mut gone = false;
    for _ in 0..25 {
        match client.tell_status(gid).await {
            Err(_) => {
                gone = true;
                break;
            }
            Ok(s) => {
                if matches!(
                    s.status,
                    Aria2TaskStatus::Removed | Aria2TaskStatus::Complete | Aria2TaskStatus::Error
                ) {
                    gone = true;
                    break;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let _ = client.remove_download_result(gid).await;
    if !gone {
        tracing::warn!(?gid, "remove: task still present after force");
    }
}

async fn delete_task_files(paths: &[String]) {
    for p in paths {
        if p.is_empty() {
            continue;
        }
        tracing::debug!(path = %p, "deleting file");
        if let Err(e) = tokio::fs::remove_file(std::path::Path::new(p)).await {
            tracing::debug!(path = %p, error = %e, "delete file skipped");
        }
        let _ = tokio::fs::remove_file(std::path::Path::new(&format!("{p}.aria2"))).await;
    }
}

fn select_file_csv(files: &[u64]) -> Option<String> {
    if files.is_empty() {
        return None;
    }
    Some(
        files
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn parse_all_proxy(url: &str) -> (String, String, String) {
    let url = url.trim();
    if url.is_empty() {
        return (String::new(), String::new(), String::new());
    }
    let (scheme, rest) = match url.split_once("://") {
        Some((s, r)) => (Some(s), r),
        None => (None, url),
    };
    let (server, user, pass) = if let Some((userinfo, host)) = rest.rsplit_once('@') {
        if host.is_empty() {
            (url.to_string(), String::new(), String::new())
        } else {
            let (user, pass) = match userinfo.rsplit_once(':') {
                Some((u, p)) => (u.to_string(), p.to_string()),
                None => (userinfo.to_string(), String::new()),
            };
            (host.to_string(), user, pass)
        }
    } else {
        (rest.to_string(), String::new(), String::new())
    };
    let server = match scheme {
        Some(s) => format!("{s}://{server}"),
        None => server,
    };
    (server, user, pass)
}

fn base_task_options(dir: &Path, split: u16) -> TaskOptions {
    TaskOptions {
        dir: Some(dir.to_string_lossy().to_string()),
        split: Some(split as i32),
        max_connection_per_server: Some((split as i32).max(1)),
        ..Default::default()
    }
}

fn has_header_control_chars(name: &str, value: &str) -> bool {
    [name, value]
        .iter()
        .any(|s| s.chars().any(|c| matches!(c, '\r' | '\n' | '\0')))
}

#[allow(clippy::too_many_arguments)]
async fn add_download_internal(
    client: &Client,
    urls: &[String],
    save_dir: &Path,
    split: u16,
    advanced: &TaskAdvancedOptions,
    headers: &[(String, String)],
    bt_metadata_only: bool,
    from_browser: bool,
    event_tx: &EventTx,
) -> Result<(), String> {
    if urls.is_empty() {
        return Err("no URLs provided".into());
    }
    let mut options = base_task_options(save_dir, split);
    advanced.apply(&mut options);
    if !headers.is_empty() {
        options.header = Some(
            headers
                .iter()
                .filter(|(n, v)| !n.trim().is_empty() && !has_header_control_chars(n, v))
                .map(|(n, v)| format!("{n}: {v}"))
                .collect(),
        );
    }
    let dir = save_dir.to_string_lossy().to_string();
    let mut added = 0;
    for url in urls {
        let mut opts = options.clone();
        apply_bt_url_options(&mut opts, url, bt_metadata_only);
        match client
            .add_uri(vec![url.clone()], Some(opts), None, None)
            .await
        {
            Ok(gid) => {
                let name = basename(url).unwrap_or_else(|| gid.clone());
                let _ = event_tx.send(EngineEvent::Added {
                    gid,
                    name,
                    url: url.clone(),
                    dir: dir.clone(),
                    info_hash: None,
                    advanced: advanced.clone(),
                    from_browser,
                });
                added += 1;
            }
            Err(e) => {
                tracing::error!(url = %url, error = %e, "add_uri failed");
            }
        }
    }
    if added == 0 {
        return Err("all add_uri calls failed".into());
    }
    Ok(())
}

async fn add_torrent_and_emit(
    client: &Client,
    path: &Path,
    save_dir: &Path,
    split: u16,
    advanced: &TaskAdvancedOptions,
    select_files: Option<&[u64]>,
    event_tx: &EventTx,
) -> Result<String, String> {
    tracing::info!(?path, ?save_dir, split, "add torrent");
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("read torrent: {e}"))?;
    let mut options = base_task_options(save_dir, split);
    advanced.apply(&mut options);
    if let Some(csv) = select_file_csv(select_files.unwrap_or(&[])) {
        options
            .extra_options
            .insert("select-file".to_string(), serde_json::Value::String(csv));
    }
    let gid = client
        .add_torrent(bytes, None, Some(options), None, None)
        .await
        .map_err(|e| format!("add_torrent: {e}"))?;
    match client.tell_status(&gid).await {
        Ok(status) => {
            emit_added(event_tx, &status, advanced.clone()).await;
            emit_progress(event_tx, &status).await;
        }
        Err(e) => {
            tracing::warn!(?gid, error = ?e, "tell_status after add_torrent failed");
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&gid)
                .to_string();
            let dir = save_dir.to_string_lossy().to_string();
            let _ = event_tx.send(EngineEvent::Added {
                gid: gid.clone(),
                name,
                url: String::new(),
                dir,
                info_hash: None,
                advanced: advanced.clone(),
                from_browser: false,
            });
        }
    }
    Ok(gid)
}

async fn resume_staggered(client: &Client, gids: Option<HashSet<String>>, event_tx: &EventTx) {
    let tasks = fetch_all_tasks(client).await;
    let mut groups: Vec<(Option<String>, Vec<aria2_ws::response::Status>)> = Vec::new();
    for s in tasks {
        if s.status != Aria2TaskStatus::Paused {
            continue;
        }
        if let Some(gids) = &gids {
            if !gids.contains(&s.gid) {
                continue;
            }
        }
        let url = s
            .files
            .first()
            .and_then(|f| f.uris.first())
            .map(|u| u.uri.clone())
            .unwrap_or_default();
        let host = url_host(&url);
        match groups.iter_mut().find(|(h, _)| *h == host) {
            Some((_, v)) => v.push(s),
            None => groups.push((host, vec![s])),
        }
    }
    if groups.is_empty() {
        return;
    }
    const RESUME_GROUP_INTERVAL: Duration = Duration::from_millis(500);
    tracing::info!(groups = groups.len(), "resume staggered groups");
    for (host, group) in groups {
        tracing::info!(?host, count = group.len(), "resuming group");
        let client = client.clone();
        let tx = event_tx.clone();
        tokio::spawn(async move {
            for (i, s) in group.iter().enumerate() {
                if i > 0 {
                    tokio::time::sleep(RESUME_GROUP_INTERVAL).await;
                }
                let _ = client.unpause(&s.gid).await;
                if let Ok(st) = client.tell_status(&s.gid).await {
                    emit_progress(&tx, &st).await;
                }
            }
        });
    }
}

async fn handle_client_cmd(
    client: &Client,
    cmd: EngineCmd,
    event_tx: &EventTx,
) -> Result<(), String> {
    match cmd {
        EngineCmd::AddDownload {
            urls,
            save_dir,
            split,
            advanced,
            bt_metadata_only,
        } => {
            tracing::info!(?urls, ?save_dir, split, "add download");
            add_download_internal(
                client,
                &urls,
                &save_dir,
                split,
                &advanced,
                &[],
                bt_metadata_only,
                false,
                event_tx,
            )
            .await?;
        }
        EngineCmd::AddExternalDownload {
            urls,
            save_dir,
            split,
            advanced,
            headers,
            bt_metadata_only,
        } => {
            tracing::info!(
                ?urls,
                ?save_dir,
                split,
                headers = headers.len(),
                "add external download"
            );
            add_download_internal(
                client,
                &urls,
                &save_dir,
                split,
                &advanced,
                &headers,
                bt_metadata_only,
                true,
                event_tx,
            )
            .await?;
        }
        EngineCmd::Pause(gid) => {
            tracing::info!(?gid, "pause");
            let _ = client.pause(&gid).await;
            if let Ok(status) = client.tell_status(&gid).await {
                emit_progress(event_tx, &status).await;
            }
        }
        EngineCmd::Resume(gid) => {
            tracing::info!(?gid, "resume");
            let _ = client.unpause(&gid).await;
            if let Ok(status) = client.tell_status(&gid).await {
                emit_progress(event_tx, &status).await;
            }
        }
        EngineCmd::Remove { gid, delete_files } => {
            tracing::info!(?gid, delete_files, "remove");
            let status = client.tell_status(&gid).await.ok();
            let paths = status.as_ref().map(collect_file_paths).unwrap_or_default();
            let mut related: Vec<String> = Vec::new();
            if let Some(s) = &status {
                if let Some(followed) = &s.followed_by {
                    related.extend(followed.iter().cloned());
                }
                if let Some(parent) = &s.belongs_to {
                    related.push(parent.clone());
                }
            }
            for other in related {
                if other != gid {
                    remove_task_from_aria2(client, &other).await;
                    let _ = event_tx.send(EngineEvent::Removed(other));
                }
            }
            remove_task_from_aria2(client, &gid).await;
            let _ = client.save_session().await;
            if delete_files {
                delete_task_files(&paths).await;
            }
            let _ = event_tx.send(EngineEvent::Removed(gid));
        }
        EngineCmd::PauseAll => {
            tracing::info!("pause all");
            let _ = client.pause_all().await;
            for s in fetch_all_tasks(client).await {
                emit_progress(event_tx, &s).await;
            }
        }
        EngineCmd::ResumeAll => {
            tracing::info!("resume all (staggered by host)");
            resume_staggered(client, None, event_tx).await;
        }
        EngineCmd::ResumeGids(gids) => {
            if gids.is_empty() {
                return Ok(());
            }
            tracing::info!(count = gids.len(), "resume gids (staggered by host)");
            let gids: HashSet<String> = gids.into_iter().collect();
            resume_staggered(client, Some(gids), event_tx).await;
        }
        EngineCmd::RemoveAll { delete_files } => {
            tracing::info!(delete_files, "remove all");
            let tasks = fetch_all_tasks(client).await;
            for s in &tasks {
                remove_task_from_aria2(client, &s.gid).await;
                let _ = event_tx.send(EngineEvent::Removed(s.gid.clone()));
            }
            let _ = client.save_session().await;
            if delete_files {
                for s in &tasks {
                    delete_task_files(&collect_file_paths(s)).await;
                }
            }
        }
        EngineCmd::Snapshot => {
            tracing::debug!("snapshot");
            for s in fetch_all_tasks(client).await {
                emit_progress(event_tx, &s).await;
            }
        }
        EngineCmd::PurgeResults(gids) => {
            tracing::info!(count = gids.len(), "purge download results");
            for gid in gids {
                let _ = client.remove_download_result(&gid).await;
            }
        }
        EngineCmd::CheckMissingFiles => {
            trigger_missing_files_check(client.clone(), event_tx.clone());
        }
        EngineCmd::AddTorrent {
            path,
            save_dir,
            split,
            advanced,
            select_files,
        } => {
            let gid = match add_torrent_and_emit(
                client,
                &path,
                &save_dir,
                split,
                &advanced,
                select_files.as_deref(),
                event_tx,
            )
            .await
            {
                Ok(gid) => gid,
                Err(e) => return Err(e),
            };
            let _ = event_tx.send(EngineEvent::TorrentAdded { gid, path });
        }
        EngineCmd::FollowTorrent {
            gid,
            path,
            save_dir,
            split,
            advanced,
            delete_after,
        } => {
            tracing::info!(
                ?gid,
                ?path,
                ?save_dir,
                split,
                delete_after,
                "follow torrent"
            );
            match add_torrent_and_emit(client, &path, &save_dir, split, &advanced, None, event_tx)
                .await
            {
                Ok(new_gid) => {
                    tracing::info!(?gid, new_gid, "torrent follow created content task");
                    if delete_after {
                        let _ = tokio::fs::remove_file(&path).await;
                        remove_task_from_aria2(client, &gid).await;
                        let _ = client.save_session().await;
                        let _ = event_tx.send(EngineEvent::Removed(gid));
                    }
                }
                Err(e) => {
                    tracing::warn!(?gid, error = ?e, "follow torrent failed, keeping source task");
                }
            }
        }
        EngineCmd::FetchTaskDetails(gid) => {
            tracing::debug!(?gid, "fetch task details");
            match client.tell_status(&gid).await {
                Ok(s) => {
                    let files = s
                        .files
                        .iter()
                        .map(|f| crate::task::TaskFile {
                            index: f.index,
                            path: f.path.clone(),
                            length: f.length,
                            completed_length: f.completed_length,
                            selected: f.selected,
                        })
                        .collect();
                    let mode = s.bittorrent.as_ref().and_then(|b| {
                        b.mode.clone().map(|m| match m {
                            aria2_ws::response::BitTorrentFileMode::Single => "single".to_string(),
                            aria2_ws::response::BitTorrentFileMode::Multi => "multi".to_string(),
                        })
                    });
                    let details = crate::task::TaskDetails {
                        bitfield: s.bitfield,
                        num_pieces: s.num_pieces,
                        piece_length: s.piece_length,
                        files,
                        creation_date: s
                            .bittorrent
                            .as_ref()
                            .and_then(|b| b.creation_date.map(|d| d.timestamp())),
                        comment: s.bittorrent.as_ref().and_then(|b| b.comment.clone()),
                        mode,
                    };
                    let _ = event_tx.send(EngineEvent::TaskDetails { gid, details });
                }
                Err(e) => {
                    tracing::warn!(?gid, error = ?e, "tell_status failed");
                    let _ = event_tx.send(EngineEvent::TaskDetailsFailed { gid });
                }
            }
        }
        EngineCmd::FetchTaskAdvanced(gid) => {
            tracing::debug!(?gid, "fetch task advanced options");
            match client
                .call_and_wait::<serde_json::Value>(
                    "getOption",
                    vec![serde_json::Value::String(gid.clone())],
                )
                .await
            {
                Ok(map) => {
                    let get = |key: &str| -> String {
                        match map.get(key) {
                            Some(serde_json::Value::String(s)) => s.clone(),
                            _ => String::new(),
                        }
                    };
                    let (proxy_server, proxy_username, proxy_password) =
                        parse_all_proxy(&get("all-proxy"));
                    let options = TaskAdvancedOptions {
                        out: String::new(),
                        user_agent: get("user-agent"),
                        http_user: get("http-user"),
                        http_passwd: get("http-passwd"),
                        referer: get("referer"),
                        cookie: get("cookie"),
                        proxy_server,
                        proxy_username,
                        proxy_password,
                    };
                    let _ = event_tx.send(EngineEvent::TaskAdvancedLoaded {
                        gid,
                        options: Box::new(options),
                    });
                }
                Err(e) => {
                    tracing::warn!(?gid, error = ?e, "getOption failed");
                    let _ = event_tx.send(EngineEvent::TaskAdvancedLoadFailed { gid });
                }
            }
        }
        EngineCmd::ChangeTaskAdvanced { gid, advanced } => {
            tracing::info!(?gid, "change task advanced options");
            let mut options = TaskOptions::default();
            advanced.apply_change(&mut options);
            let params = match serde_json::to_value(options) {
                Ok(v) => vec![serde_json::Value::String(gid.clone()), v],
                Err(e) => {
                    tracing::warn!(?gid, error = ?e, "serialize advanced options failed");
                    let _ = event_tx.send(EngineEvent::TaskAdvancedApplyFailed { gid });
                    return Ok(());
                }
            };
            match client
                .call_and_wait::<serde_json::Value>("changeOption", params)
                .await
            {
                Ok(_) => {
                    let _ = client.save_session().await;
                    let _ = event_tx.send(EngineEvent::TaskAdvancedApplied {
                        gid,
                        options: advanced,
                    });
                }
                Err(e) => {
                    tracing::warn!(?gid, error = ?e, "changeOption advanced failed");
                    let _ = event_tx.send(EngineEvent::TaskAdvancedApplyFailed { gid });
                }
            }
        }
        EngineCmd::ReaddTask {
            gid,
            url,
            save_dir,
            split,
            paused,
            bt_metadata_only,
        } => {
            tracing::info!(?gid, ?url, ?save_dir, split, paused, "re-add ghost task");
            let mut options = base_task_options(&save_dir, split);
            options.gid = Some(gid.clone());
            options.r#continue = Some(true);
            options.auto_file_renaming = Some(true);
            apply_bt_url_options(&mut options, &url, bt_metadata_only);
            match client
                .add_uri(vec![url.clone()], Some(options), None, None)
                .await
            {
                Ok(_) => {
                    if paused {
                        let _ = client.pause(&gid).await;
                    }
                    let name = basename(&url).unwrap_or_else(|| gid.clone());
                    let dir = save_dir.to_string_lossy().to_string();
                    let _ = event_tx.send(EngineEvent::Added {
                        gid: gid.clone(),
                        name,
                        url,
                        dir,
                        info_hash: None,
                        advanced: TaskAdvancedOptions::default(),
                        from_browser: false,
                    });
                    if let Ok(status) = client.tell_status(&gid).await {
                        emit_progress(event_tx, &status).await;
                    }
                }
                Err(e) => {
                    tracing::warn!(?gid, error = ?e, "re-add ghost task failed");
                    let _ = event_tx.send(EngineEvent::TaskDetailsFailed { gid });
                }
            }
        }
        EngineCmd::Redownload {
            gid,
            url,
            save_dir,
            split,
            bt_metadata_only,
        } => {
            tracing::info!(?gid, ?url, ?save_dir, split, "re-download task");
            let _ = client.remove_download_result(&gid).await;
            let mut options = base_task_options(&save_dir, split);
            options.gid = Some(gid.clone());
            options.r#continue = Some(false);
            options.auto_file_renaming = Some(true);
            apply_bt_url_options(&mut options, &url, bt_metadata_only);
            match client
                .add_uri(vec![url.clone()], Some(options), None, None)
                .await
            {
                Ok(_) => {
                    if let Ok(status) = client.tell_status(&gid).await {
                        emit_added(event_tx, &status, TaskAdvancedOptions::default()).await;
                        emit_progress(event_tx, &status).await;
                    }
                }
                Err(e) => {
                    tracing::warn!(?gid, error = ?e, "re-download failed");
                }
            }
        }
        EngineCmd::SelectFiles { gid, files } => {
            let Some(csv) = select_file_csv(&files) else {
                return Ok(());
            };
            tracing::info!(?gid, ?files, "change file selection");
            let options = TaskOptions {
                extra_options: {
                    let mut map = serde_json::Map::new();
                    map.insert("select-file".to_string(), serde_json::Value::String(csv));
                    map
                },
                ..Default::default()
            };
            let params = match serde_json::to_value(options) {
                Ok(v) => vec![serde_json::Value::String(gid.clone()), v],
                Err(e) => {
                    tracing::warn!(?gid, error = ?e, "serialize select-file options failed");
                    let _ = event_tx.send(EngineEvent::SelectFilesFailed { gid });
                    return Ok(());
                }
            };
            match client
                .call_and_wait::<serde_json::Value>("changeOption", params)
                .await
            {
                Ok(_) => {
                    let _ = client.save_session().await;
                }
                Err(e) => {
                    tracing::warn!(?gid, error = ?e, "changeOption select-file failed");
                    let _ = event_tx.send(EngineEvent::SelectFilesFailed { gid });
                }
            }
        }
        EngineCmd::ApplyAria2Options { options } => {
            tracing::info!("apply aria2 options");
            if let Err(e) = apply_global_options(client, options).await {
                tracing::warn!("apply_global_options: {e}");
            }
        }
        EngineCmd::Shutdown
        | EngineCmd::ForceKill
        | EngineCmd::DownloadAria2Update { .. }
        | EngineCmd::DownloadAppUpdate { .. }
        | EngineCmd::ReloadSchedules
        | EngineCmd::RetryAria2Fetch
        | EngineCmd::RestartEngine => {
            tracing::debug!("cmd handled by supervisor, not dispatched here");
        }
    }
    Ok(())
}

async fn apply_global_options(client: &Client, options: TaskOptions) -> Result<(), String> {
    let params = serde_json::to_value(options).map_err(|e| format!("serialize options: {e}"))?;
    match timeout(
        RPC_TIMEOUT,
        client.call_and_wait::<String>("changeGlobalOption", vec![params]),
    )
    .await
    {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("change_global_option: {e}")),
        Err(_) => Err("change_global_option timed out".into()),
    }
}

pub(crate) fn trigger_missing_files_check(client: Client, event_tx: EventTx) {
    if MISSING_CHECK_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    tokio::spawn(async move {
        let gids = timeout(Duration::from_secs(30), check_missing_files(&client))
            .await
            .unwrap_or_default();
        MISSING_CHECK_IN_FLIGHT.store(false, Ordering::Release);
        if !gids.is_empty() {
            let _ = event_tx.send(EngineEvent::FilesMissing { gids });
        }
    });
}

async fn apply_speed_limits(client: &Client, settings: &crate::config::Settings, inside: bool) {
    let (dl, ul) = if inside {
        (
            (settings.download_limit_kb * 1024).to_string(),
            (settings.upload_limit_kb * 1024).to_string(),
        )
    } else {
        ("0".to_string(), "0".to_string())
    };
    let mut extra = serde_json::Map::new();
    extra.insert(
        "max-overall-download-limit".into(),
        serde_json::Value::String(dl),
    );
    extra.insert(
        "max-overall-upload-limit".into(),
        serde_json::Value::String(ul),
    );
    let options = TaskOptions {
        extra_options: extra,
        ..Default::default()
    };
    if let Err(e) = apply_global_options(client, options).await {
        tracing::warn!("apply scheduled speed limits failed: {e}");
    } else {
        tracing::info!(inside, "scheduled speed limit window transition");
    }
}

fn run_scheduler(client: Client, event_tx: EventTx) -> JoinHandle<()> {
    tokio::spawn(async move {
        let settings = crate::config::load();
        let schedule = settings.speed_limit_schedule.clone();

        let now = chrono::Local::now();
        let mut inside = schedule.active_at(&now);

        let missing_enabled = settings.remove_task_if_files_missing;
        const MISSING_CHECK_INTERVAL: Duration = Duration::from_secs(600);
        let mut last_missing_check = tokio::time::Instant::now();

        let mut ticker = interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            let now = chrono::Local::now();

            let cur = schedule.active_at(&now);
            if cur && !inside {
                inside = true;
                apply_speed_limits(&client, &settings, true).await;
            } else if !cur && inside {
                inside = false;
                apply_speed_limits(&client, &settings, false).await;
            }

            if missing_enabled && last_missing_check.elapsed() >= MISSING_CHECK_INTERVAL {
                last_missing_check = tokio::time::Instant::now();
                trigger_missing_files_check(client.clone(), event_tx.clone());
            }
        }
    })
}

fn installed_version() -> String {
    aria2_fetcher::installed_version().unwrap_or_default()
}

/// Download `url` to `dest` through the live aria2 RPC client. The file is
/// written directly to `dest` (same filesystem as the caller expects), then
/// verified against an optional sha256. On completion or any failure the
/// download result and the session are cleaned up.
async fn download_via_engine(
    client: &Client,
    url: &str,
    dest: &Path,
    sha256: Option<&str>,
) -> Result<(), String> {
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("invalid download destination parent")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("create download dir: {e}"))?;

    let filename = dest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let _ = std::fs::remove_file(dest);
    let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));

    let mut extra = serde_json::Map::new();
    extra.insert(
        "allow-overwrite".to_string(),
        serde_json::Value::String("false".to_string()),
    );
    let opts = TaskOptions {
        dir: Some(parent.to_string_lossy().into_owned()),
        out: Some(filename.to_string()),
        split: Some(8),
        max_connection_per_server: Some(8),
        r#continue: Some(true),
        auto_file_renaming: Some(false),
        extra_options: extra,
        ..Default::default()
    };
    let gid = timeout(
        RPC_TIMEOUT,
        client.add_uri(vec![url.to_string()], Some(opts), None, None),
    )
    .await
    .map_err(|_| "update add_uri timed out".to_string())?
    .map_err(|e| format!("update add_uri: {e}"))?;

    let mut last_progress = tokio::time::Instant::now();
    let mut last_completed: u64 = 0;
    let mut consecutive_failures: u32 = 0;
    loop {
        let status = match timeout(RPC_TIMEOUT, client.tell_status(&gid)).await {
            Ok(Ok(s)) => {
                consecutive_failures = 0;
                s
            }
            Ok(Err(e)) => {
                tracing::warn!(?gid, error = ?e, "update tell_status failed");
                consecutive_failures += 1;
                if consecutive_failures >= UPDATE_DOWNLOAD_MAX_POLL_FAILURES {
                    let _ = client.force_remove(&gid).await;
                    let _ = client.remove_download_result(&gid).await;
                    let _ = client.save_session().await;
                    let _ = std::fs::remove_file(dest);
                    let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
                    return Err("update download connection lost".to_string());
                }
                tokio::time::sleep(UPDATE_DOWNLOAD_POLL_INTERVAL).await;
                continue;
            }
            Err(_) => {
                tracing::warn!(?gid, "update tell_status timed out");
                consecutive_failures += 1;
                if consecutive_failures >= UPDATE_DOWNLOAD_MAX_POLL_FAILURES {
                    let _ = client.force_remove(&gid).await;
                    let _ = client.remove_download_result(&gid).await;
                    let _ = client.save_session().await;
                    let _ = std::fs::remove_file(dest);
                    let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
                    return Err("update download connection lost".to_string());
                }
                tokio::time::sleep(UPDATE_DOWNLOAD_POLL_INTERVAL).await;
                continue;
            }
        };
        match status.status {
            Aria2TaskStatus::Complete => break,
            Aria2TaskStatus::Removed => {
                let _ = client.remove_download_result(&gid).await;
                let _ = client.save_session().await;
                let _ = std::fs::remove_file(dest);
                let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
                return Err("update download removed".to_string());
            }
            Aria2TaskStatus::Error => {
                let msg = status
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "unknown error".to_string());
                let _ = client.remove_download_result(&gid).await;
                let _ = client.save_session().await;
                let _ = std::fs::remove_file(dest);
                let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
                return Err(format!("update download failed: {msg}"));
            }
            Aria2TaskStatus::Paused | Aria2TaskStatus::Waiting => {
                consecutive_failures = 0;
                last_progress = tokio::time::Instant::now();
                tokio::time::sleep(UPDATE_DOWNLOAD_POLL_INTERVAL).await;
            }
            _ => {
                let making_progress =
                    status.completed_length > last_completed || status.download_speed > 0;
                if making_progress {
                    last_progress = tokio::time::Instant::now();
                    last_completed = status.completed_length;
                }
                if !making_progress && last_progress.elapsed() >= UPDATE_DOWNLOAD_MAX_WAIT {
                    let _ = client.force_remove(&gid).await;
                    let _ = client.remove_download_result(&gid).await;
                    let _ = client.save_session().await;
                    let _ = std::fs::remove_file(dest);
                    let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
                    return Err("update download stalled".to_string());
                }
                tokio::time::sleep(UPDATE_DOWNLOAD_POLL_INTERVAL).await;
            }
        }
    }

    if let Some(expected) = sha256 {
        let dest_clone = dest.to_path_buf();
        let digest = tokio::task::spawn_blocking(move || aria2_fetcher::sha256_file(&dest_clone))
            .await
            .map_err(|e| format!("sha256 task: {e}"))?
            .map_err(|e| format!("sha256: {e}"))?;
        if digest != expected {
            let _ = std::fs::remove_file(dest);
            let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
            let _ = client.remove_download_result(&gid).await;
            let _ = client.save_session().await;
            return Err(format!(
                "sha256 mismatch: expected {expected}, got {digest}"
            ));
        }
    }

    aria2_fetcher::set_perms(dest)?;
    let _ = std::fs::remove_file(format!("{}.aria2", dest.display()));
    let _ = client.remove_download_result(&gid).await;
    let _ = client.save_session().await;
    Ok(())
}

/// Reduce an externally-supplied value to a single safe path component,
/// rejecting anything that is empty, `.`, `..`, or contains path separators.
fn sanitize_component(input: &str, what: &str) -> Result<String, String> {
    let base = std::path::Path::new(input)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if base.is_empty() || base == "." || base == ".." {
        return Err(format!("invalid {what}: {input:?}"));
    }
    Ok(base.to_string())
}

/// Resolve the platform slug for an aria2-next update asset, stripping any
/// path components from the externally-supplied `asset_name` so it cannot
/// escape the aria2 data directory. Falls back to the platform slug.
fn resolve_aria2_slug(version: &str, asset_name: &str) -> Result<String, String> {
    let slug = asset_name
        .strip_prefix(&format!("aria2-next-{version}-"))
        .unwrap_or(crate::updater::platform_slug());
    sanitize_component(slug, "aria2 update asset name")
}

/// Resolve the update asset's sha256, short-circuiting on an already-known
/// value and otherwise fetching it from the release's checksums.
async fn resolve_sha256(
    repo: &str,
    version: &str,
    asset_name: &str,
    proxy: Option<String>,
    existing: Option<String>,
) -> Option<String> {
    if let Some(e) = existing {
        Some(e)
    } else {
        crate::updater::fetch_asset_checksum(repo, version, asset_name, proxy).await
    }
}

async fn handle_download_aria2_update(cmd: EngineCmd, event_tx: &EventTx) {
    let EngineCmd::DownloadAria2Update {
        version,
        asset_name,
        download_url,
        sha256,
    } = cmd
    else {
        return;
    };
    tracing::info!(?version, "download aria2 update via engine");
    let proxy = crate::config::load().aria2.all_proxy_value();
    let version = match sanitize_component(&version, "aria2 update version") {
        Ok(v) => v,
        Err(e) => {
            let _ = event_tx.send(EngineEvent::Aria2UpdateFailed { error: e });
            return;
        }
    };
    let slug = match resolve_aria2_slug(&version, &asset_name) {
        Ok(s) => s,
        Err(e) => {
            let _ = event_tx.send(EngineEvent::Aria2UpdateFailed { error: e });
            return;
        }
    };
    let sha256 = resolve_sha256(
        "AnInsomniacy/aria2-next",
        &version,
        &asset_name,
        proxy.clone(),
        sha256,
    )
    .await;
    let dir = match crate::config::aria2_bin_dir() {
        Some(d) => d,
        None => {
            let _ = event_tx.send(EngineEvent::Aria2UpdateFailed {
                error: "cannot determine data directory".to_string(),
            });
            return;
        }
    };
    let dest = dir.join(format!("aria2-next-{version}-{slug}"));
    let tx = event_tx.clone();
    let prog: crate::aria2_fetcher::ProgressFn = Box::new(move |downloaded, total| {
        let _ = tx.send(EngineEvent::Aria2UpdateProgress { downloaded, total });
    });
    match crate::aria2_fetcher::download_verified(
        &download_url,
        &dest,
        sha256.as_deref(),
        proxy.as_deref(),
        Some(&prog),
    )
    .await
    {
        Ok(()) => {
            match crate::aria2_fetcher::stage_pending(&dir, &version, &slug, sha256.as_deref()) {
                Ok(()) => {
                    let _ = event_tx.send(EngineEvent::Aria2UpdateStaged { version });
                }
                Err(e) => {
                    let _ = event_tx.send(EngineEvent::Aria2UpdateFailed { error: e });
                }
            }
        }
        Err(e) => {
            let _ = event_tx.send(EngineEvent::Aria2UpdateFailed { error: e });
        }
    }
}

async fn handle_download_app_update_via_engine(
    client: &Client,
    cmd: EngineCmd,
    event_tx: &EventTx,
) {
    let EngineCmd::DownloadAppUpdate {
        kind,
        version,
        url,
        asset_name,
        sha256,
        download_dir,
    } = cmd
    else {
        return;
    };
    tracing::info!(?version, "download app update via engine");
    let proxy = crate::config::load().aria2.all_proxy_value();
    let sha256 = resolve_sha256(
        crate::updater::APP_REPO,
        &version,
        &asset_name,
        proxy.clone(),
        sha256,
    )
    .await;
    let dest = match crate::app_updater::app_update_dest(kind, &asset_name, Some(&download_dir)) {
        Ok(d) => d,
        Err(e) => {
            let _ = event_tx.send(EngineEvent::AppUpdateDownloadFailed { error: e });
            return;
        }
    };
    match download_via_engine(client, &url, &dest, sha256.as_deref()).await {
        Ok(()) => match crate::app_updater::apply_after_download(kind, &dest) {
            Ok(outcome) => {
                let _ = event_tx.send(EngineEvent::AppUpdateDownloaded {
                    kind: outcome.kind,
                    path: outcome.path,
                });
            }
            Err(e) => {
                let _ = event_tx.send(EngineEvent::AppUpdateDownloadFailed { error: e });
            }
        },
        Err(e) => {
            let _ = event_tx.send(EngineEvent::AppUpdateDownloadFailed { error: e });
        }
    }
}

#[cfg(not(unix))]
async fn cleanup_stale_aria2(_bin_path: &Path, _pid_path: &Path) {}

#[cfg(not(unix))]
fn kill_sidecar_by_pid(_pid_path: &Path) {}

#[cfg(unix)]
fn kill_sidecar_by_pid(pid_path: &Path) {
    let Ok(content) = std::fs::read_to_string(pid_path) else {
        return;
    };
    let Ok(pid) = content.trim().parse::<i32>() else {
        return;
    };
    tracing::warn!(%pid, "SIGKILL aria2-next by pid file");
    unsafe { libc::kill(pid, libc::SIGKILL) };
}

#[cfg(unix)]
async fn cleanup_stale_aria2(bin_path: &Path, pid_path: &Path) {
    let Ok(content) = std::fs::read_to_string(pid_path) else {
        return;
    };
    let Ok(pid) = content.trim().parse::<i32>() else {
        let _ = std::fs::remove_file(pid_path);
        return;
    };
    let alive = std::path::Path::new(&format!("/proc/{pid}")).exists();
    let is_ours = std::fs::read_link(format!("/proc/{pid}/exe"))
        .map(|p| p == bin_path)
        .unwrap_or(false);
    if alive && is_ours {
        tracing::warn!(%pid, "stale aria2-next from previous run detected, SIGTERM");
        unsafe { libc::kill(pid, libc::SIGTERM) };
        let mut waited = 0;
        while std::path::Path::new(&format!("/proc/{pid}")).exists() && waited < 50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            waited += 1;
        }
        let still_ours = std::path::Path::new(&format!("/proc/{pid}")).exists()
            && std::fs::read_link(format!("/proc/{pid}/exe"))
                .map(|p| p == bin_path)
                .unwrap_or(false);
        if still_ours {
            tracing::warn!(%pid, "stale aria2-next still alive, SIGKILL");
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    }
    let _ = std::fs::remove_file(pid_path);
}

async fn boot(
    config: &SidecarConfig,
    restart_tx: &mpsc::UnboundedSender<u64>,
    generation: u64,
    event_tx: &EventTx,
) -> Result<(Sidecar, Option<String>), String> {
    let (bin_path, applied) = {
        let proxy = crate::config::load().aria2.all_proxy_value();
        crate::aria2_fetcher::ensure_aria2_next(event_tx, proxy).await?
    };
    let pid_path = config.session_path.join("aria2.pid");
    cleanup_stale_aria2(&bin_path, &pid_path).await;
    let _ = event_tx.send(EngineEvent::Aria2Status {
        stage: "starting".to_string(),
        message: "Starting aria2-next engine...".to_string(),
    });
    let mut sidecar = Sidecar::spawn(&bin_path, config, event_tx).await?;

    if let Some(mut child) = sidecar.child.take() {
        let tx = restart_tx.clone();
        let gen = generation;
        tokio::spawn(async move {
            let _ = child.wait().await;
            tracing::warn!("aria2-next process exited");
            let _ = tx.send(gen);
        });
    }

    Ok((sidecar, applied))
}

fn on_sidecar_ready(sidecar: &Sidecar, event_tx: &EventTx) -> Vec<JoinHandle<()>> {
    let _ = event_tx.send(EngineEvent::EngineReady);
    let _ = event_tx.send(EngineEvent::Aria2Version {
        version: installed_version(),
    });

    let sync_client = sidecar.client.clone();
    let sync_event_tx = event_tx.clone();
    tokio::spawn(async move {
        if sync_existing_tasks(&sync_client, &sync_event_tx).await {
            let _ = sync_event_tx.send(EngineEvent::SyncComplete);
        }
    });

    let boot_client = sidecar.client.clone();
    tokio::spawn(async move {
        let opts = crate::config::load().effective_task_options();
        if let Err(e) = apply_global_options(&boot_client, opts).await {
            tracing::warn!("boot apply global options: {e}");
        }
    });

    let mut handles = Vec::new();

    let notif_client = sidecar.client.clone();
    let notif_event_tx = event_tx.clone();
    handles.push(tokio::spawn(async move {
        let mut rx = notif_client.subscribe_notifications();
        loop {
            match rx.recv().await {
                Ok(Notification::Aria2 { gid, event }) => match event {
                    Event::Start
                    | Event::Pause
                    | Event::Complete
                    | Event::Error
                    | Event::Stop
                    | Event::BtComplete => {
                        match timeout(RPC_TIMEOUT, notif_client.tell_status(&gid)).await {
                            Ok(Ok(status)) => emit_progress(&notif_event_tx, &status).await,
                            Ok(Err(e)) => tracing::warn!(?gid, error = ?e, "tell_status after notification failed"),
                            Err(_) => tracing::warn!(?gid, "tell_status after notification timed out"),
                        }
                    }
                },
                Ok(Notification::WebSocketConnected) => {
                    tracing::info!("aria2-ws reconnected");
                }
                Ok(Notification::WebsocketClosed) => {
                    tracing::warn!("aria2-ws connection closed");
                }
                Err(_) => {
                    break;
                }
            }
        }
    }));

    let poll_client = sidecar.client.clone();
    let poll_event_tx = event_tx.clone();
    handles.push(tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(1000));
        let mut slow = interval(Duration::from_secs(10));
        let mut stopped_seen: HashSet<String> = HashSet::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut terminal: HashSet<String> = HashSet::new();
        let mut orphan_grace: HashMap<String, u32> = HashMap::new();
        let mut last_logged_pct: HashMap<String, u32> = HashMap::new();
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let active = match timeout(RPC_TIMEOUT, poll_client.tell_active()).await {
                        Ok(Ok(list)) => list,
                        Ok(Err(e)) => {
                            tracing::warn!("tell_active: {e}");
                            continue;
                        }
                        Err(_) => {
                            tracing::warn!("tell_active timed out; skipping tick");
                            continue;
                        }
                    };
                    for s in &active {
                        stopped_seen.remove(&s.gid);
                        let total = s.total_length;
                        let pct = if total > 0 {
                            ((s.completed_length as u128 * 100) / total as u128) as u32
                        } else {
                            0
                        };
                        match last_logged_pct.get(&s.gid) {
                            None => {
                                tracing::info!(?s.gid, pct, "download started");
                                last_logged_pct.insert(s.gid.clone(), pct);
                            }
                            Some(&last) if pct >= last.saturating_add(5) => {
                                tracing::info!(
                                    ?s.gid, pct, speed = s.download_speed,
                                    "download progress"
                                );
                                last_logged_pct.insert(s.gid.clone(), pct);
                            }
                            _ => {}
                        }
                        emit_progress(&poll_event_tx, s).await;
                    }
                    match timeout(RPC_TIMEOUT, poll_client.get_global_stat()).await {
                        Ok(Ok(stat)) => {
                            let _ = poll_event_tx.send(EngineEvent::GlobalSpeed {
                                download: stat.download_speed,
                                upload: stat.upload_speed,
                            });
                        }
                        Ok(Err(e)) => tracing::warn!("get_global_stat: {e}"),
                        Err(_) => tracing::debug!("get_global_stat timed out"),
                    }
                }
                _ = slow.tick() => {
                    let (active_res, waiting_res, stopped_res) = tokio::join!(
                        poll_client.tell_active(),
                        poll_client.tell_waiting(-1, 1000),
                        poll_client.tell_stopped(-1, 1000),
                    );
                    let (active, waiting, stopped) = match (active_res, waiting_res, stopped_res) {
                        (Ok(a), Ok(w), Ok(s)) => (a, w, s),
                        _ => {
                            tracing::debug!("slow scan skipped (rpc failure)");
                            continue;
                        }
                    };
                    let mut all = active;
                    all.extend(waiting);
                    all.extend(stopped);
                    let mut current: HashSet<&str> = HashSet::with_capacity(all.len());
                    for s in &all {
                        current.insert(s.gid.as_str());
                        if seen.insert(s.gid.clone()) {
                            emit_added(&poll_event_tx, s, TaskAdvancedOptions::default()).await;
                        }
                        let is_terminal = matches!(
                            s.status,
                            Aria2TaskStatus::Complete
                                | Aria2TaskStatus::Error
                                | Aria2TaskStatus::Removed
                        );
                        if is_terminal {
                            terminal.insert(s.gid.clone());
                            if stopped_seen.insert(s.gid.clone()) {
                                match s.status {
                                    Aria2TaskStatus::Complete => {
                                        tracing::info!(?s.gid, "download finished")
                                    }
                                    _ => tracing::warn!(?s.gid, "download failed"),
                                }
                                emit_progress(&poll_event_tx, s).await;
                            }
                        } else {
                            stopped_seen.remove(&s.gid);
                            if s.status != Aria2TaskStatus::Active {
                                emit_progress(&poll_event_tx, s).await;
                            }
                        }
                    }
                    for g in &seen {
                        if current.contains(g.as_str()) {
                            orphan_grace.remove(g.as_str());
                        }
                    }
                    let orphans: Vec<String> = seen
                        .iter()
                        .filter(|g| !current.contains(g.as_str()) && !terminal.contains(*g))
                        .cloned()
                        .collect();
                    for gid in orphans {
                        let count = orphan_grace.entry(gid.clone()).or_insert(0);
                        *count += 1;
                        if *count < 2 {
                            continue;
                        }
                        tracing::info!(?gid, "orphan task detected, removing");
                        let _ = poll_event_tx.send(EngineEvent::Removed(gid.clone()));
                        orphan_grace.remove(&gid);
                        seen.remove(&gid);
                        terminal.remove(&gid);
                        stopped_seen.remove(&gid);
                    }
                    seen.retain(|g| current.contains(g.as_str()) || orphan_grace.contains_key(g));
                    terminal.retain(|g| current.contains(g.as_str()));
                    stopped_seen.retain(|g| current.contains(g.as_str()));
                    last_logged_pct.retain(|g, _| current.contains(g.as_str()));
                }
            }
        }
    }));

    handles
}

fn start_scheduler(sidecar: &Sidecar, event_tx: &EventTx) -> JoinHandle<()> {
    run_scheduler(sidecar.client.clone(), event_tx.clone())
}

async fn graceful_stop(client: &Client, event_tx: &EventTx, warn_msg: &str) {
    if let Err(e) = client.pause_all().await {
        tracing::warn!("pause_all failed: {e}");
    }
    let mut paused = false;
    for _ in 0..10 {
        match client.tell_active().await {
            Ok(list) if list.is_empty() => {
                paused = true;
                break;
            }
            _ => {}
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    if !paused {
        tracing::warn!("{warn_msg}");
    }
    for status in fetch_all_tasks(client).await {
        emit_progress(event_tx, &status).await;
    }
    let _ = client.save_session().await;
    let _ = client.shutdown().await;
}

#[allow(clippy::too_many_arguments)]
async fn install_sidecar(
    config: &SidecarConfig,
    restart_tx: &mpsc::UnboundedSender<u64>,
    event_tx: &EventTx,
    generation: u64,
    sidecar: &mut Option<Sidecar>,
    poll_handles: &mut Vec<JoinHandle<()>>,
    scheduler_handle: &mut Option<JoinHandle<()>>,
    retry_count: &mut u32,
) -> Result<(), String> {
    for h in poll_handles.drain(..) {
        h.abort();
    }
    if let Some(h) = scheduler_handle.take() {
        h.abort();
    }
    *sidecar = None;
    match boot(config, restart_tx, generation, event_tx).await {
        Ok((s, applied)) => {
            *poll_handles = on_sidecar_ready(&s, event_tx);
            *scheduler_handle = Some(start_scheduler(&s, event_tx));
            *sidecar = Some(s);
            *retry_count = 0;
            if let Some(v) = applied {
                let _ = event_tx.send(EngineEvent::Aria2UpdateApplied { version: v });
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

async fn run_supervisor(mut cmd_rx: CmdRx, event_tx: EventTx) {
    let session_path = crate::config::session_dir().unwrap_or_else(|| PathBuf::from("."));
    let download_dir = crate::config::load().download_dir;
    let config = SidecarConfig {
        session_path,
        download_dir,
    };

    let (restart_tx, mut restart_rx) = mpsc::unbounded_channel::<u64>();

    let mut sidecar: Option<Sidecar> = None;
    let mut poll_handles: Vec<JoinHandle<()>> = Vec::new();
    let mut scheduler_handle: Option<JoinHandle<()>> = None;
    let mut retry_count = 0;
    let mut generation: u64 = 0;
    const MAX_RETRIES: u32 = 3;

    match install_sidecar(
        &config,
        &restart_tx,
        &event_tx,
        generation,
        &mut sidecar,
        &mut poll_handles,
        &mut scheduler_handle,
        &mut retry_count,
    )
    .await
    {
        Ok(()) => {}
        Err(e) => {
            tracing::error!("initial sidecar startup failed: {e}");
            let _ = event_tx.send(EngineEvent::Aria2FetchFailed { error: e });
        }
    }

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else {
                    tracing::info!("cmd channel closed");
                    break;
                };
                match &cmd {
                    EngineCmd::Shutdown => {
                        if let Some(ref s) = sidecar {
                            graceful_stop(&s.client, &event_tx, "tasks did not fully pause before session save").await;
                        }
                        let _ = event_tx.send(EngineEvent::EngineStopped);
                        if let Some(h) = scheduler_handle.take() {
                            h.abort();
                        }
                        break;
                    }
                    EngineCmd::ForceKill => {
                        tracing::warn!("force-killing sidecar");
                        for h in poll_handles.drain(..) {
                            h.abort();
                        }
                        if let Some(h) = scheduler_handle.take() {
                            h.abort();
                        }
                        if let Some(ref s) = sidecar {
                            if let Ok(Ok(())) =
                                tokio::time::timeout(Duration::from_secs(1), s.client.force_shutdown()).await
                            {
                                tracing::info!("sidecar force-shutdown accepted");
                            }
                        }
                        kill_sidecar_by_pid(&config.session_path.join("aria2.pid"));
                        let _ = event_tx.send(EngineEvent::EngineStopped);
                        break;
                    }
                    EngineCmd::DownloadAria2Update { .. } => {
                        let tx = event_tx.clone();
                        let cmd = cmd.clone();
                        if sidecar.is_some() {
                            tokio::spawn(async move {
                                handle_download_aria2_update(cmd, &tx).await;
                            });
                        } else {
                            let _ = event_tx.send(EngineEvent::Aria2UpdateFailed {
                                error: "aria2-next not available".to_string(),
                            });
                        }
                    }
                    EngineCmd::DownloadAppUpdate { .. } => {
                        let tx = event_tx.clone();
                        let cmd = cmd.clone();
                        if let Some(ref s) = sidecar {
                            let client = s.client.clone();
                            tokio::spawn(async move {
                                handle_download_app_update_via_engine(&client, cmd, &tx).await;
                            });
                        } else {
                            let _ = event_tx.send(EngineEvent::AppUpdateDownloadFailed {
                                error: "aria2-next not available".to_string(),
                            });
                        }
                    }
                    EngineCmd::ReloadSchedules => {
                        if let Some(ref s) = sidecar {
                            if let Some(h) = scheduler_handle.take() {
                                h.abort();
                            }
                            scheduler_handle = Some(start_scheduler(s, &event_tx));
                            tracing::info!("scheduler reloaded from config");
                        }
                    }
                    EngineCmd::RetryAria2Fetch => {
                        generation += 1;
                        match install_sidecar(
                            &config,
                            &restart_tx,
                            &event_tx,
                            generation,
                            &mut sidecar,
                            &mut poll_handles,
                            &mut scheduler_handle,
                            &mut retry_count,
                        )
                        .await
                        {
                            Ok(()) => {}
                            Err(e) => {
                                let _ = event_tx.send(EngineEvent::Aria2FetchFailed { error: e });
                            }
                        }
                    }
                    EngineCmd::RestartEngine => {
                        if let Some(ref s) = sidecar {
                            graceful_stop(&s.client, &event_tx, "tasks did not fully pause before engine restart").await;
                            sidecar = None;
                        }
                        generation += 1;
                        match install_sidecar(
                            &config,
                            &restart_tx,
                            &event_tx,
                            generation,
                            &mut sidecar,
                            &mut poll_handles,
                            &mut scheduler_handle,
                            &mut retry_count,
                        )
                        .await
                        {
                            Ok(()) => {}
                            Err(e) => {
                                let _ = event_tx.send(EngineEvent::EngineDegraded { reason: e });
                            }
                        }
                    }
                    _ => {
                        if let Some(ref s) = sidecar {
                            if let Err(e) = handle_client_cmd(&s.client, cmd, &event_tx).await {
                                tracing::error!("cmd error: {e}");
                            }
                        } else {
                            let _ = event_tx.send(EngineEvent::EngineDegraded {
                                reason: "aria2-next not available".to_string(),
                            });
                        }
                    }
                }
            }
            gen = restart_rx.recv() => {
                let Some(gen) = gen else {
                    tracing::info!("restart channel closed");
                    continue;
                };
                if gen != generation {
                    tracing::debug!(?gen, current = generation, "ignoring stale sidecar exit notification");
                    continue;
                }
                tracing::warn!("aria2-next exited, restarting...");
                retry_count += 1;
                if retry_count > MAX_RETRIES {
                    tracing::error!("sidecar restart failed after {MAX_RETRIES} attempts");
                    for h in poll_handles.drain(..) { h.abort(); }
                    if let Some(h) = scheduler_handle.take() { h.abort(); }
                    sidecar = None;
                    let _ = event_tx.send(EngineEvent::Aria2FetchFailed {
                        error: "aria2-next crashed repeatedly".to_string(),
                    });
                    continue;
                }
                match install_sidecar(
                    &config,
                    &restart_tx,
                    &event_tx,
                    generation,
                    &mut sidecar,
                    &mut poll_handles,
                    &mut scheduler_handle,
                    &mut retry_count,
                )
                .await
                {
                    Ok(()) => {
                        tracing::info!("aria2-next restarted successfully");
                    }
                    Err(e) => {
                        tracing::warn!("restart attempt failed: {e}");
                        let _ = event_tx.send(EngineEvent::Aria2FetchFailed { error: e });
                    }
                }
            }
        }
    }

    if let Some(h) = scheduler_handle.take() {
        h.abort();
    }
    for h in poll_handles {
        h.abort();
    }
    let _ = std::fs::remove_file(config.session_path.join("aria2.pid"));
    tracing::info!("engine supervisor stopped");
}
