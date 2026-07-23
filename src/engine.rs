use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aria2_core::engine::download_command::DownloadCommand;
use aria2_core::engine::download_engine::DownloadEngine;
use aria2_core::request::request_group::DownloadOptions;
use aria2_core::request::request_group::DownloadStatus;
use aria2_core::request::request_group::GroupId;
use aria2_core::request::request_group_man::RequestGroupMan;

use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

#[derive(Debug, Clone)]
pub enum EngineCmd {
    AddDownload {
        urls: Vec<String>,
        save_dir: PathBuf,
        split: u16,
    },
    Pause(String),
    Resume(String),
    Remove(String),
    PauseAll,
    ResumeAll,
    RemoveAll,
    Snapshot,
    SetSpeedLimit {
        download: Option<u64>,
        upload: Option<u64>,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    Added {
        gid: String,
        name: String,
    },
    Progress {
        gid: String,
        downloaded: u64,
        total: u64,
        speed: u64,
        status: String,
    },
    Removed(String),
    EngineReady,
    EngineStopped,
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

async fn run_supervisor(mut cmd_rx: CmdRx, event_tx: EventTx) {
    let man: Arc<RequestGroupMan> = Arc::new(RequestGroupMan::new());

    let mut engine = DownloadEngine::new(250);
    engine.set_keep_alive(true);

    let sender = engine.command_sender();
    let _ = event_tx.send(EngineEvent::EngineReady);

    tokio::spawn(async move {
        if let Err(e) = engine.run().await {
            tracing::error!(error = ?e, "download engine exited with error");
        }
    });

    let man_clone = man.clone();
    let event_tx_clone = event_tx.clone();
    tokio::spawn(run_progress_poller(man_clone, event_tx_clone));

    let mut last_status: HashMap<String, String> = HashMap::new();

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            EngineCmd::AddDownload {
                urls,
                save_dir,
                split,
            } => {
                tracing::info!(?urls, ?save_dir, split, "engine: add download");
                if let Some(uri) = urls.first() {
                    let dir = save_dir.to_string_lossy().to_string();
                    let opts = DownloadOptions {
                        split: Some(split),
                        max_connection_per_server: Some(split.max(1)),
                        dir: Some(dir.clone()),
                        ..Default::default()
                    };

                    let gid = man.add_group(urls.clone(), opts.clone()).await.ok();
                    if let Some(gid) = gid {
                        let group = man.group_by_id(gid);
                        if let Some(g) = group {
                            let cmd =
                                DownloadCommand::new_with_group(g, uri, &opts, Some(&dir), None);
                            match cmd {
                                Ok(cmd) => {
                                    if sender.send(Box::new(cmd)).is_err() {
                                        tracing::warn!(
                                            "engine: download command channel full/closed"
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(error = ?e, "engine: build download command failed");
                                }
                            }
                        }
                        let name = basename(uri).unwrap_or_else(|| gid.to_hex_string());
                        if event_tx
                            .send(EngineEvent::Added {
                                gid: gid.to_hex_string(),
                                name,
                            })
                            .is_err()
                        {
                            tracing::warn!("engine: event channel send failed (Added)");
                        }
                    } else {
                        tracing::warn!("engine: add_group returned None");
                    }
                }
            }
            EngineCmd::Pause(gid_hex) => {
                tracing::info!(?gid_hex, "engine: pause");
                if let Some(gid) = GroupId::from_hex_string(&gid_hex) {
                    let _ = man.pause_group(gid).await;
                    last_status.insert(gid_hex.clone(), "paused".to_string());
                } else {
                    tracing::warn!(?gid_hex, "engine: pause invalid gid");
                }
            }
            EngineCmd::Resume(gid_hex) => {
                tracing::info!(?gid_hex, "engine: resume");
                if let Some(gid) = GroupId::from_hex_string(&gid_hex) {
                    let _ = man.unpause_group(gid).await;
                    last_status.insert(gid_hex.clone(), "waiting".to_string());
                } else {
                    tracing::warn!(?gid_hex, "engine: resume invalid gid");
                }
            }
            EngineCmd::Remove(gid_hex) => {
                tracing::info!(?gid_hex, "engine: remove");
                if let Some(gid) = GroupId::from_hex_string(&gid_hex) {
                    let _ = man.remove_group(gid).await;
                    let _ = event_tx.send(EngineEvent::Removed(gid_hex));
                } else {
                    tracing::warn!(?gid_hex, "engine: remove invalid gid");
                }
            }
            EngineCmd::PauseAll => {
                tracing::info!("engine: pause all");
                let snapshot = man.all_groups();
                for (gid, _) in snapshot {
                    let _ = man.pause_group(gid).await;
                    last_status.insert(gid.to_hex_string(), "paused".to_string());
                }
            }
            EngineCmd::ResumeAll => {
                tracing::info!("engine: resume all");
                let snapshot = man.all_groups();
                for (gid, _) in snapshot {
                    let _ = man.unpause_group(gid).await;
                    last_status.insert(gid.to_hex_string(), "waiting".to_string());
                }
            }
            EngineCmd::RemoveAll => {
                tracing::info!("engine: remove all");
                let snapshot = man.all_groups();
                for (gid, _) in snapshot {
                    let _ = man.remove_group(gid).await;
                    let _ = event_tx.send(EngineEvent::Removed(gid.to_hex_string()));
                }
            }
            EngineCmd::Snapshot => {
                tracing::debug!("engine: snapshot");
                let snapshot = man.all_groups();
                for (gid, group) in snapshot {
                    let gid_hex = gid.to_hex_string();
                    let rg = group.read().await;
                    let status = rg.status().await;
                    let downloaded = rg.get_completed_length();
                    let total = rg.get_total_length_atomic();
                    let speed = rg.get_download_speed_cached();
                    let status_str = match &status {
                        DownloadStatus::Waiting => "waiting",
                        DownloadStatus::Active => "active",
                        DownloadStatus::Paused => "paused",
                        DownloadStatus::Complete => "complete",
                        DownloadStatus::Error(_) => "error",
                        DownloadStatus::Removed => "removed",
                    }
                    .to_string();

                    let _ = event_tx.send(EngineEvent::Progress {
                        gid: gid_hex,
                        downloaded,
                        total,
                        speed,
                        status: status_str,
                    });
                }
            }
            EngineCmd::SetSpeedLimit { download, upload } => {
                tracing::info!(?download, ?upload, "engine: set speed limit");
                man.set_global_speed_limit(download, upload).await;
            }
            EngineCmd::Shutdown => {
                tracing::info!("engine: shutdown");
                let _ = event_tx.send(EngineEvent::EngineStopped);
                break;
            }
        }
    }

    tracing::info!("engine supervisor stopped");
}

async fn run_progress_poller(man: Arc<RequestGroupMan>, event_tx: EventTx) {
    let mut ticker = interval(Duration::from_millis(500));
    let mut last_status: HashMap<String, String> = HashMap::new();
    let mut first_seen: HashMap<String, bool> = HashMap::new();

    loop {
        ticker.tick().await;
        let snapshot = man.all_groups();
        for (gid, group) in snapshot {
            let gid_hex = gid.to_hex_string();
            let rg = group.read().await;
            let status = rg.status().await;
            let downloaded = rg.get_completed_length();
            let total = rg.get_total_length_atomic();
            let speed = rg.get_download_speed_cached();
            let status_str = match &status {
                DownloadStatus::Waiting => "waiting",
                DownloadStatus::Active => "active",
                DownloadStatus::Paused => "paused",
                DownloadStatus::Complete => "complete",
                DownloadStatus::Error(_) => "error",
                DownloadStatus::Removed => "removed",
            }
            .to_string();

            if !first_seen.contains_key(&gid_hex) {
                first_seen.insert(gid_hex.clone(), true);
                tracing::info!(?gid_hex, total, ?status_str, "task: first seen");
            }

            if let Some(prev) = last_status.get(&gid_hex) {
                if prev != &status_str {
                    tracing::info!(
                        ?gid_hex, from = prev, to = ?status_str, downloaded, total,
                        "task: status changed"
                    );
                    if status_str == "error" || status_str == "removed" {
                        tracing::warn!(?gid_hex, ?status_str, "task: error/removed");
                    }
                }
            }

            let _ = event_tx.send(EngineEvent::Progress {
                gid: gid_hex.clone(),
                downloaded,
                total,
                speed,
                status: status_str.clone(),
            });

            last_status.insert(gid_hex, status_str);
        }
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
