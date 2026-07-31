use std::path::{Path, PathBuf};
use std::process::Stdio;

use aria2_ws::response::TaskStatus as Aria2TaskStatus;
use aria2_ws::{Client, Event, Notification, TaskOptions};
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};

use crate::aria2_fetcher;

#[derive(Debug, Clone, Default)]
pub struct TaskAdvancedOptions {
    pub out: String,
    pub user_agent: String,
    pub http_user: String,
    pub http_passwd: String,
    pub referer: String,
    pub cookie: String,
}

impl TaskAdvancedOptions {
    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
            && self.user_agent.is_empty()
            && self.http_user.is_empty()
            && self.http_passwd.is_empty()
            && self.referer.is_empty()
            && self.cookie.is_empty()
    }

    pub fn apply(&self, opts: &mut TaskOptions) {
        if !self.out.is_empty() {
            opts.out = Some(self.out.clone());
        }
        let mut extra = vec![
            ("user-agent", &self.user_agent),
            ("http-user", &self.http_user),
            ("http-passwd", &self.http_passwd),
            ("referer", &self.referer),
            ("cookie", &self.cookie),
        ];
        for (key, value) in extra.drain(..) {
            if !value.is_empty() {
                opts.extra_options
                    .insert(key.to_string(), serde_json::Value::String(value.clone()));
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum EngineCmd {
    AddDownload {
        urls: Vec<String>,
        save_dir: PathBuf,
        split: u16,
        advanced: TaskAdvancedOptions,
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
    ApplyAria2Options {
        options: TaskOptions,
    },
    AddTorrent {
        path: PathBuf,
        save_dir: PathBuf,
        split: u16,
        advanced: TaskAdvancedOptions,
    },
    FetchTaskDetails(String),
    ReaddTask {
        gid: String,
        url: String,
        save_dir: PathBuf,
        split: u16,
        paused: bool,
    },
    Shutdown,
    CheckAria2Update,
    RetryAria2Fetch,
    RestartEngine,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    Added {
        gid: String,
        name: String,
        url: String,
        dir: String,
    },
    Progress {
        gid: String,
        downloaded: u64,
        total: u64,
        speed: u64,
        upload_speed: u64,
        status: String,
        connections: u64,
    },
    Removed(String),
    TaskDetails {
        gid: String,
        details: crate::task::TaskDetails,
    },
    TaskDetailsFailed {
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
    Aria2CheckResult {
        current: String,
        latest: Option<String>,
    },
    Aria2UpdateApplied {
        version: String,
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
    EngineDegraded {
        reason: String,
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
    port: u16,
    secret: String,
}

struct SidecarConfig {
    session_path: PathBuf,
    download_dir: PathBuf,
}

fn find_free_port() -> Result<u16, std::io::Error> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    Ok(addr.port())
}

fn generate_secret() -> String {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}{:x}", nanos, std::process::id())
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
    async fn spawn(bin_path: &Path, config: &SidecarConfig) -> Result<Self, String> {
        let port = find_free_port().map_err(|e| format!("port allocation: {e}"))?;
        let secret = generate_secret();

        let session_file = config.session_path.join("session.txt");
        if !session_file.exists() {
            std::fs::write(&session_file, "").map_err(|e| format!("create session file: {e}"))?;
        }
        let session_str = session_file.to_string_lossy().to_string();

        let dir_str = config.download_dir.to_string_lossy();
        tracing::info!(port, session = %session_str, dir = %dir_str, "spawning aria2-next sidecar");

        let mut child = Command::new(bin_path)
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
                    return Ok(Sidecar {
                        client,
                        child: Some(child),
                        port,
                        secret,
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

async fn emit_progress(event_tx: &EventTx, s: &aria2_ws::response::Status) {
    let _ = event_tx.send(EngineEvent::Progress {
        gid: s.gid.clone(),
        downloaded: s.completed_length,
        total: s.total_length,
        speed: s.download_speed,
        upload_speed: s.upload_speed,
        status: status_to_string(&s.status).to_string(),
        connections: s.connections,
    });
}

async fn fetch_all_tasks(client: &Client) -> Vec<aria2_ws::response::Status> {
    let mut all = Vec::new();
    if let Ok(list) = client.tell_active().await {
        all.extend(list);
    }
    if let Ok(list) = client.tell_waiting(-1, 1000).await {
        all.extend(list);
    }
    if let Ok(list) = client.tell_stopped(-1, 1000).await {
        all.extend(list);
    }
    all
}

async fn sync_existing_tasks(client: &Client, event_tx: &EventTx) {
    let all = fetch_all_tasks(client).await;
    for s in &all {
        let name = name_from_status(s);
        let url = s
            .files
            .first()
            .and_then(|f| f.uris.first())
            .map(|u| u.uri.clone())
            .unwrap_or_default();
        let dir = s.dir.clone();
        let _ = event_tx.send(EngineEvent::Added {
            gid: s.gid.clone(),
            name,
            url,
            dir,
        });
        emit_progress(event_tx, s).await;
    }
    if all.is_empty() {
        tracing::info!("no existing tasks found during sync");
    } else {
        tracing::info!("synced {} existing tasks", all.len());
    }
}

async fn remove_task_from_aria2(client: &Client, gid: &str) {
    if client.remove(gid).await.is_err() {
        let _ = client.force_remove(gid).await;
    }
    let _ = client.remove_download_result(gid).await;
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
        } => {
            tracing::info!(?urls, ?save_dir, split, "add download");
            let uri = match urls.first() {
                Some(u) => u.clone(),
                None => return Err("no URLs provided".into()),
            };
            let mut options = TaskOptions {
                dir: Some(save_dir.to_string_lossy().to_string()),
                split: Some(split as i32),
                max_connection_per_server: Some((split as i32).max(1)),
                ..Default::default()
            };
            advanced.apply(&mut options);
            let gid = client
                .add_uri(urls, Some(options), None, None)
                .await
                .map_err(|e| format!("add_uri: {e}"))?;
            let name = basename(&uri).unwrap_or_else(|| gid.clone());
            let dir = save_dir.to_string_lossy().to_string();
            let _ = event_tx.send(EngineEvent::Added {
                gid,
                name,
                url: uri,
                dir,
            });
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
            let paths = client
                .tell_status(&gid)
                .await
                .ok()
                .map(|s| collect_file_paths(&s))
                .unwrap_or_default();
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
            tracing::info!("resume all");
            let _ = client.unpause_all().await;
            for s in fetch_all_tasks(client).await {
                emit_progress(event_tx, &s).await;
            }
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
        EngineCmd::AddTorrent {
            path,
            save_dir,
            split,
            advanced,
        } => {
            tracing::info!(?path, ?save_dir, split, "add torrent");
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|e| format!("read torrent: {e}"))?;
            let mut options = TaskOptions {
                dir: Some(save_dir.to_string_lossy().to_string()),
                split: Some(split as i32),
                max_connection_per_server: Some((split as i32).max(1)),
                ..Default::default()
            };
            advanced.apply(&mut options);
            let gid = client
                .add_torrent(bytes, None, Some(options), None, None)
                .await
                .map_err(|e| format!("add_torrent: {e}"))?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&gid)
                .to_string();
            let dir = save_dir.to_string_lossy().to_string();
            let _ = event_tx.send(EngineEvent::Added {
                gid,
                name,
                url: String::new(),
                dir,
            });
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
                    let details = crate::task::TaskDetails {
                        bitfield: s.bitfield,
                        num_pieces: s.num_pieces,
                        piece_length: s.piece_length,
                        files,
                        upload_speed: s.upload_speed,
                        num_seeders: s.num_seeders,
                        info_hash: s.info_hash,
                        error_code: s.error_code,
                        error_message: s.error_message,
                    };
                    let _ = event_tx.send(EngineEvent::TaskDetails { gid, details });
                }
                Err(e) => {
                    tracing::warn!(?gid, error = ?e, "tell_status failed");
                    let _ = event_tx.send(EngineEvent::TaskDetailsFailed { gid });
                }
            }
        }
        EngineCmd::ReaddTask {
            gid,
            url,
            save_dir,
            split,
            paused,
        } => {
            tracing::info!(?gid, ?url, ?save_dir, split, paused, "re-add ghost task");
            let options = TaskOptions {
                gid: Some(gid.clone()),
                dir: Some(save_dir.to_string_lossy().to_string()),
                split: Some(split as i32),
                max_connection_per_server: Some((split as i32).max(1)),
                r#continue: Some(true),
                auto_file_renaming: Some(true),
                ..Default::default()
            };
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
        EngineCmd::ApplyAria2Options { options } => {
            tracing::info!("apply aria2 options");
            if let Err(e) = client.change_global_option(options.clone()).await {
                tracing::warn!("change_global_option: {e}");
            }
        }
        _ => {}
    }
    Ok(())
}

fn installed_version() -> String {
    aria2_fetcher::installed_version().unwrap_or_default()
}

async fn handle_check_update(event_tx: &EventTx) {
    tracing::info!("check aria2 update");
    let current = installed_version();
    let slug = crate::updater::platform_slug();
    match crate::updater::fetch_latest_release("AnInsomniacy/aria2-next", slug).await {
        Ok(release) => {
            if release.version == current {
                let _ = event_tx.send(EngineEvent::Aria2CheckResult {
                    current,
                    latest: None,
                });
            } else {
                let settings = crate::config::load();
                if settings.update.is_skipped("aria2-next", &release.version) {
                    let _ = event_tx.send(EngineEvent::Aria2CheckResult {
                        current,
                        latest: None,
                    });
                } else {
                    match crate::aria2_fetcher::stage_update_download(&release, event_tx).await {
                        Ok(version) => {
                            let _ = event_tx.send(EngineEvent::Aria2UpdateStaged { version });
                        }
                        Err(e) => {
                            let _ = event_tx.send(EngineEvent::Aria2UpdateFailed { error: e });
                        }
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("update check failed: {e}");
            let _ = event_tx.send(EngineEvent::Aria2CheckResult {
                current,
                latest: None,
            });
        }
    }
}

async fn boot(
    config: &SidecarConfig,
    restart_tx: &mpsc::UnboundedSender<()>,
    event_tx: &EventTx,
) -> Result<(Sidecar, Option<String>), String> {
    let (bin_path, applied) = crate::aria2_fetcher::ensure_aria2_next(event_tx).await?;
    let _ = event_tx.send(EngineEvent::Aria2Status {
        stage: "starting".to_string(),
        message: "Starting aria2-next engine...".to_string(),
    });
    let mut sidecar = Sidecar::spawn(&bin_path, config).await?;

    if let Some(mut child) = sidecar.child.take() {
        let tx = restart_tx.clone();
        tokio::spawn(async move {
            let _ = child.wait().await;
            tracing::warn!("aria2-next process exited");
            let _ = tx.send(());
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
        sync_existing_tasks(&sync_client, &sync_event_tx).await;
        let _ = sync_event_tx.send(EngineEvent::SyncComplete);
    });

    let boot_client = sidecar.client.clone();
    tokio::spawn(async move {
        let opts = crate::config::load().to_aria2_task_options();
        if let Err(e) = boot_client.change_global_option(opts).await {
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
                        if let Ok(status) = notif_client.tell_status(&gid).await {
                            emit_progress(&notif_event_tx, &status).await;
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
        loop {
            ticker.tick().await;
            let active = match poll_client.tell_active().await {
                Ok(list) => list,
                Err(e) => {
                    tracing::debug!("tell_active: {e}");
                    continue;
                }
            };
            for s in &active {
                emit_progress(&poll_event_tx, s).await;
            }
            if let Ok(stat) = poll_client.get_global_stat().await {
                let _ = poll_event_tx.send(EngineEvent::GlobalSpeed {
                    download: stat.download_speed,
                    upload: stat.upload_speed,
                });
            } else {
                tracing::debug!("get_global_stat failed");
            }
        }
    }));

    handles
}

async fn run_supervisor(mut cmd_rx: CmdRx, event_tx: EventTx) {
    let session_path = crate::config::session_dir().unwrap_or_else(|| PathBuf::from("."));
    let download_dir = crate::config::load().download_dir;
    let config = SidecarConfig {
        session_path,
        download_dir,
    };

    let (restart_tx, mut restart_rx) = mpsc::unbounded_channel::<()>();

    let mut sidecar: Option<Sidecar> = None;
    let mut poll_handles: Vec<JoinHandle<()>> = Vec::new();
    let mut retry_count = 0;
    const MAX_RETRIES: u32 = 3;

    match boot(&config, &restart_tx, &event_tx).await {
        Ok((s, applied)) => {
            poll_handles = on_sidecar_ready(&s, &event_tx);
            sidecar = Some(s);
            retry_count = 0;
            if let Some(v) = applied {
                let _ = event_tx.send(EngineEvent::Aria2UpdateApplied { version: v });
            }
        }
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
                            let _ = s.client.save_session().await;
                            let _ = s.client.shutdown().await;
                        }
                        let _ = event_tx.send(EngineEvent::EngineStopped);
                        break;
                    }
                    EngineCmd::CheckAria2Update => {
                        handle_check_update(&event_tx).await;
                    }
                    EngineCmd::RetryAria2Fetch => {
                        for h in poll_handles.drain(..) { h.abort(); }
                        sidecar = None;
                        match boot(&config, &restart_tx, &event_tx).await {
                            Ok((s, applied)) => {
                                poll_handles = on_sidecar_ready(&s, &event_tx);
                                sidecar = Some(s);
                                retry_count = 0;
                                if let Some(v) = applied {
                                    let _ = event_tx.send(EngineEvent::Aria2UpdateApplied { version: v });
                                }
                            }
                            Err(e) => {
                                let _ = event_tx.send(EngineEvent::Aria2FetchFailed { error: e });
                            }
                        }
                    }
                    EngineCmd::RestartEngine => {
                        if let Some(ref s) = sidecar {
                            for h in poll_handles.drain(..) { h.abort(); }
                            let _ = s.client.save_session().await;
                            let _ = s.client.shutdown().await;
                            sidecar = None;
                        } else {
                            for h in poll_handles.drain(..) { h.abort(); }
                            match boot(&config, &restart_tx, &event_tx).await {
                                Ok((s, applied)) => {
                                    poll_handles = on_sidecar_ready(&s, &event_tx);
                                    sidecar = Some(s);
                                    retry_count = 0;
                                    if let Some(v) = applied {
                                        let _ = event_tx.send(EngineEvent::Aria2UpdateApplied { version: v });
                                    }
                                }
                                Err(e) => {
                                    let _ = event_tx.send(EngineEvent::Aria2FetchFailed { error: e });
                                }
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
            _ = restart_rx.recv() => {
                tracing::warn!("aria2-next exited, restarting...");
                for h in poll_handles.drain(..) { h.abort(); }
                sidecar = None;
                retry_count += 1;
                if retry_count > MAX_RETRIES {
                    tracing::error!("sidecar restart failed after {MAX_RETRIES} attempts");
                    let _ = event_tx.send(EngineEvent::Aria2FetchFailed {
                        error: "aria2-next crashed repeatedly".to_string(),
                    });
                    continue;
                }
                match boot(&config, &restart_tx, &event_tx).await {
                    Ok((s, applied)) => {
                        poll_handles = on_sidecar_ready(&s, &event_tx);
                        sidecar = Some(s);
                        retry_count = 0;
                        if let Some(v) = applied {
                            let _ = event_tx.send(EngineEvent::Aria2UpdateApplied { version: v });
                        }
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

    for h in poll_handles {
        h.abort();
    }
    tracing::info!("engine supervisor stopped");
}
