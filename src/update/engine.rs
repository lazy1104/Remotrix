use std::path::PathBuf;
use std::time::Duration;

use iced::Task;

use crate::app::{
    apply_task_name, begin_task_exit, check_updates, dismiss_toast, engine_restart_cooldown_task,
    engine_restart_safety_timeout_task, finalize_close, gid_recently_removed, refresh_tray,
    send_system_notification, spawn_toast, sync_global_stat_cache, trigger_shutdown_confirm,
    Remotrix,
};
use crate::engine::{EngineCmd, EngineEvent};
use crate::i18n::Tr;
use crate::message::{ConfirmAction, EngineMsg, Message, SettingsMsg};
use crate::task::{DownloadTask, TaskAdvancedOptions, TaskStatus};
use crate::ui::components::toast::{ToastGroup, ToastKind};

pub(crate) fn handle(state: &mut Remotrix, msg: EngineMsg) -> Task<Message> {
    match msg {
        EngineMsg::Event(event) => handle_event(state, *event),
        EngineMsg::RetryAria2Fetch => {
            state.engine_ui.aria2_fetch_error = None;
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::RetryAria2Fetch)
                .is_err()
            {
                tracing::warn!("retry fetch cmd send failed");
            }
            Task::none()
        }
        EngineMsg::RestartEngine => {
            if state.restart.engine_restart_in_progress {
                return Task::none();
            }
            let has_active = state.tasks.values().any(|t| t.status == TaskStatus::Active);
            state.confirm = Some(ConfirmAction::RestartEngine { has_active });
            state.confirm_anim.open();
            Task::none()
        }
        EngineMsg::ConfirmRestartEngine => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            state.confirm_anim.begin_exit();
            state.restart.engine_restart_in_progress = true;
            state.restart.restart_resume_gids = state
                .tasks
                .values()
                .filter(|t| t.status == TaskStatus::Active)
                .map(|t| t.gid.clone())
                .collect();
            if state.handle.cmd_tx.send(EngineCmd::RestartEngine).is_err() {
                tracing::warn!("restart engine cmd send failed");
            }
            engine_restart_safety_timeout_task()
        }
        EngineMsg::EngineRestartCooldownFinished => {
            state.restart.engine_restart_in_progress = false;
            state.restart.restart_resume_gids.clear();
            Task::none()
        }
        EngineMsg::EngineRestartSafetyTimeout => {
            if state.restart.engine_restart_in_progress {
                state.restart.engine_restart_in_progress = false;
                state.restart.restart_resume_gids.clear();
            }
            Task::none()
        }
    }
}

fn handle_event(state: &mut Remotrix, event: EngineEvent) -> Task<Message> {
    match event {
        EngineEvent::EngineReady => {
            tracing::info!("engine ready");
            state.engine_ui.aria2_fetch_error = None;
            state.tracking.synced_gids.clear();
            state.tracking.sync_done = false;
            if let Some(id) = state.engine_ui.downloading_toast_id.take() {
                dismiss_toast(state, id);
            }
            if let Some(id) = state.engine_ui.startup_error_toast_id.take() {
                dismiss_toast(state, id);
            }
            state.engine_ui.startup_starting_toast_shown = false;
            state.restart.engine_restart_pending = false;
            state.engine_ui.aria2_status =
                Some(("ready".to_string(), state.fluent.get(Tr::Aria2Ready)));
            if !state.restart.restart_resume_gids.is_empty() {
                let gids: Vec<String> = state.restart.restart_resume_gids.iter().cloned().collect();
                if state
                    .handle
                    .cmd_tx
                    .send(EngineCmd::ResumeGids(gids))
                    .is_err()
                {
                    tracing::warn!("resume gids cmd send failed");
                }
            }
            spawn_toast(
                state,
                ToastGroup::Engine,
                ToastKind::Success,
                state.fluent.get(Tr::EngineStarted),
                Some(Duration::from_secs(3)),
                false,
            );
            if state.restart.engine_restart_in_progress {
                return engine_restart_cooldown_task();
            }
            Task::none()
        }
        EngineEvent::EngineStopped => {
            tracing::info!("engine stopped");
            state.global_speed = None;
            state.tracking.paused_gids.clear();
            if state.window.closing {
                return finalize_close(state);
            }
            Task::none()
        }
        EngineEvent::SyncComplete => {
            tracing::info!("engine sync complete");
            if state.tracking.sync_done {
                return Task::none();
            }
            state.tracking.sync_done = true;
            for (gid, t) in state.tasks.iter() {
                if t.status == TaskStatus::Completed
                    && !t.url.is_empty()
                    && crate::engine::is_torrent_url(&t.url)
                {
                    state.tracking.torrent_followed.insert(gid.clone());
                }
            }
            let purge: Vec<String> = state
                .tasks
                .iter()
                .filter(|(gid, t)| {
                    !state.tracking.synced_gids.contains(*gid)
                        && matches!(
                            t.status,
                            TaskStatus::Waiting | TaskStatus::Active | TaskStatus::Paused
                        )
                        && t.url.is_empty()
                        && t.info_hash.is_none()
                })
                .map(|(gid, _)| gid.clone())
                .collect();
            for gid in &purge {
                begin_task_exit(state, gid, true);
                tracing::info!(?gid, "ui: purged non-terminal ghost task");
            }
            let split = state.settings.split;
            let bt_metadata_only = !state.settings.aria2.bt_auto_download;
            let ghost: Vec<(String, String, PathBuf, bool, bool)> = state
                .tasks
                .iter()
                .filter(|(gid, t)| {
                    !state.tracking.synced_gids.contains(*gid)
                        && (!t.url.is_empty() || t.info_hash.is_some())
                        && matches!(
                            t.status,
                            TaskStatus::Waiting | TaskStatus::Active | TaskStatus::Paused
                        )
                })
                .map(|(gid, t)| {
                    let url = if !t.url.is_empty() {
                        t.url.clone()
                    } else {
                        let hash = t.info_hash.clone().unwrap_or_default();
                        format!("magnet:?xt=urn:btih:{hash}")
                    };
                    (
                        gid.clone(),
                        url,
                        t.save_dir.clone(),
                        t.status == TaskStatus::Paused,
                        bt_metadata_only,
                    )
                })
                .collect();
            for (gid, url, save_dir, paused, bt_metadata_only) in ghost {
                if paused {
                    state.tracking.paused_gids.insert(gid.clone());
                }
                if state
                    .handle
                    .cmd_tx
                    .send(EngineCmd::ReaddTask {
                        gid,
                        url,
                        save_dir,
                        split,
                        paused,
                        bt_metadata_only,
                    })
                    .is_err()
                {
                    tracing::warn!("ui: re-add ghost task cmd send failed");
                }
            }
            if state.settings.remove_task_if_files_missing
                && state
                    .handle
                    .cmd_tx
                    .send(EngineCmd::CheckMissingFiles)
                    .is_err()
            {
                tracing::warn!("check missing files cmd send failed");
            }
            Task::none()
        }
        EngineEvent::FilesMissing { gids } => {
            let removed: Vec<String> = gids
                .iter()
                .filter(|g| state.tasks.contains_key(*g))
                .cloned()
                .collect();
            for gid in &removed {
                tracing::info!(?gid, "ui: removed task with missing files");
                begin_task_exit(state, gid, true);
            }
            if !removed.is_empty() {
                if state
                    .handle
                    .cmd_tx
                    .send(EngineCmd::PurgeResults(removed))
                    .is_err()
                {
                    tracing::warn!("ui: purge missing-files results cmd send failed");
                }
                spawn_toast(
                    state,
                    ToastGroup::Task,
                    ToastKind::Normal,
                    state.fluent.get(Tr::FilesMissingRemoved),
                    Some(Duration::from_secs(3)),
                    false,
                );
                return Task::none();
            }
            Task::none()
        }
        EngineEvent::Added {
            gid,
            name,
            url,
            dir,
            info_hash,
            advanced,
            from_browser,
        } => {
            tracing::info!(?gid, ?name, from_browser, "ui: task added");
            state.tracking.synced_gids.insert(gid.clone());
            let task_name = name.clone();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let mut probe = Task::none();
            if let Some(existing) = state.tasks.get_mut(&gid) {
                probe = apply_task_name(&state.db, &gid, existing, name);
                existing.url = url;
                existing.save_dir = PathBuf::from(dir);
                if info_hash.is_some() {
                    existing.info_hash = info_hash;
                }
                if !advanced.is_empty() {
                    existing.advanced = Some(advanced);
                }
                state.tracking.dirty.insert(gid.clone());
            } else if !gid_recently_removed(state, &gid) {
                let saved_advanced = if advanced.is_empty() {
                    None
                } else {
                    Some(advanced)
                };
                let task = DownloadTask {
                    gid: gid.clone(),
                    name,
                    url,
                    save_dir: PathBuf::from(dir),
                    downloaded: 0,
                    total: 0,
                    speed: 0,
                    upload_speed: 0,
                    status: TaskStatus::Waiting,
                    connections: 0,
                    added_at: now,
                    info_hash,
                    metadata_probe_size: None,
                    is_seeding: false,
                    advanced: saved_advanced.clone(),
                };
                state.tasks.insert(gid.clone(), task);
                state.task_order.insert(0, gid.clone());
                state.card_anim.insert(
                    gid.clone(),
                    crate::ui::animation::Animated::transition(
                        0.0,
                        crate::ui::animation::ease_out_cubic(crate::ui::animation::CARD_ENTER_MS),
                    )
                    .to(1.0),
                );
                if let Some(ref db) = state.db {
                    db.upsert_meta(
                        &gid,
                        &state.tasks[&gid].name,
                        &state.tasks[&gid].url,
                        &state.tasks[&gid].save_dir.to_string_lossy(),
                        "waiting",
                        now,
                        &state.tasks[&gid].info_hash.clone().unwrap_or_default(),
                        saved_advanced.as_ref(),
                    );
                }
            }
            if from_browser && state.tasks.contains_key(&gid) {
                let mut args = std::collections::HashMap::new();
                args.insert(
                    std::borrow::Cow::from("name"),
                    std::borrow::Cow::from(task_name).into(),
                );
                spawn_toast(
                    state,
                    ToastGroup::Task,
                    ToastKind::Normal,
                    state.fluent.get_args(Tr::BrowserAdded, &args),
                    Some(Duration::from_secs(3)),
                    false,
                );
                if state.settings.notifications.download_added {
                    let title = state.fluent.get(Tr::DownloadAddedTitle);
                    let body = state.fluent.get_args(Tr::DownloadAdded, &args);
                    send_system_notification(
                        state,
                        title,
                        body,
                        vec![],
                        crate::notify::NotifyAction::ActivateWindow,
                    );
                }
            }
            state.tracking.dirty.insert(gid);
            sync_global_stat_cache(state);
            refresh_tray(state);
            probe
        }
        EngineEvent::TorrentAdded { gid, path } => {
            state.tracking.torrent_files.insert(gid, path);
            Task::none()
        }
        EngineEvent::Progress {
            gid,
            name,
            downloaded,
            total,
            speed,
            upload_speed,
            status,
            connections,
            info_hash,
            is_seeding,
        } => {
            refresh_tray(state);
            state.tracking.synced_gids.insert(gid.clone());
            let was_download_complete = state.tracking.completion_toasted.contains(&gid);
            let is_download_complete = crate::task::is_download_complete(&status, is_seeding);
            let was_error = state
                .tasks
                .get(&gid)
                .map(|t| t.status == TaskStatus::Error)
                .unwrap_or(false);
            let mut side_tasks: Vec<Task<Message>> = Vec::new();
            if status == "complete"
                && state.settings.delete_torrent_after_complete
                && state.tracking.torrent_files.contains_key(&gid)
            {
                if let Some(path) = state.tracking.torrent_files.remove(&gid) {
                    side_tasks.push(Task::perform(
                        async move {
                            let _ = std::fs::remove_file(&path);
                        },
                        |_| Message::Noop,
                    ));
                }
            }
            if !state.tasks.contains_key(&gid)
                && !gid_recently_removed(state, &gid)
                && !matches!(status.as_str(), "complete" | "error" | "removed")
            {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let task_status = TaskStatus::from_engine(&status);
                if task_status == TaskStatus::Active {
                    state.tracking.active_count += 1;
                }
                state.tasks.insert(
                    gid.clone(),
                    DownloadTask {
                        gid: gid.clone(),
                        name: String::new(),
                        url: String::new(),
                        save_dir: PathBuf::new(),
                        downloaded: 0,
                        total: 0,
                        speed: 0,
                        upload_speed: 0,
                        status: task_status,
                        connections: 0,
                        added_at: now,
                        info_hash: info_hash.clone(),
                        metadata_probe_size: None,
                        is_seeding,
                        advanced: None,
                    },
                );
                state.task_order.insert(0, gid.clone());
                state.card_anim.insert(
                    gid.clone(),
                    crate::ui::animation::Animated::transition(
                        0.0,
                        crate::ui::animation::ease_out_cubic(crate::ui::animation::CARD_ENTER_MS),
                    )
                    .to(1.0),
                );
                if let Some(ref db) = state.db {
                    db.upsert_meta(
                        &gid,
                        &name,
                        "",
                        "",
                        &status,
                        now,
                        info_hash.as_deref().unwrap_or_default(),
                        None,
                    );
                }
            }
            if let Some(t) = state.tasks.get_mut(&gid) {
                let was_active = t.status == TaskStatus::Active;
                side_tasks.push(apply_task_name(&state.db, &gid, t, name));
                if info_hash.is_some() {
                    t.info_hash = info_hash;
                }
                if total == 0 && t.total > 0 {
                    t.status = TaskStatus::from_engine(&status);
                    t.speed = speed;
                    t.upload_speed = upload_speed;
                    t.connections = connections;
                    t.is_seeding = is_seeding;
                } else {
                    t.downloaded = downloaded;
                    t.total = total;
                    t.speed = speed;
                    t.upload_speed = upload_speed;
                    t.status = TaskStatus::from_engine(&status);
                    t.connections = connections;
                    t.is_seeding = is_seeding;
                }
                if state.tracking.paused_gids.contains(&gid) {
                    t.status = TaskStatus::Paused;
                }
                if t.status == TaskStatus::Paused {
                    t.speed = 0;
                    t.upload_speed = 0;
                    t.is_seeding = false;
                }
                if was_active != (t.status == TaskStatus::Active) {
                    if t.status == TaskStatus::Active {
                        state.tracking.active_count += 1;
                    } else {
                        state.tracking.active_count = state.tracking.active_count.saturating_sub(1);
                    }
                }
                state.tracking.dirty.insert(gid.clone());
                let pct = t.progress_pct();
                state
                    .progress_anim
                    .entry(gid.clone())
                    .or_insert_with(|| {
                        crate::ui::animation::Animated::transition(
                            pct,
                            crate::ui::animation::ease_out_quad(crate::ui::animation::PROGRESS_MS),
                        )
                    })
                    .set_target(pct);
            }
            if status == "complete" && state.tracking.sync_done {
                if let Some(t) = state.tasks.get(&gid) {
                    if !t.url.is_empty()
                        && crate::engine::is_torrent_url(&t.url)
                        && state.settings.aria2.bt_auto_download
                    {
                        if state.tracking.torrent_followed.insert(gid.clone()) {
                            let path = t.save_dir.join(&t.name);
                            let save_dir = t.save_dir.clone();
                            let _ = state.handle.cmd_tx.send(EngineCmd::FollowTorrent {
                                gid: gid.clone(),
                                path,
                                save_dir,
                                split: state.settings.split,
                                advanced: TaskAdvancedOptions::default(),
                                delete_after: state.settings.delete_torrent_after_complete,
                            });
                            tracing::info!(?gid, "ui: auto-adding downloaded torrent as new task");
                        } else {
                            state.tracking.torrent_followed.remove(&gid);
                        }
                    }
                }
            }
            if !is_download_complete {
                state.tracking.completion_toasted.remove(&gid);
            } else if !state.tracking.sync_done {
                state.tracking.completion_toasted.insert(gid.clone());
            } else if !was_download_complete {
                if let Some(t) = state.tasks.get(&gid) {
                    let name = t.name.clone();
                    let open_path = t.save_dir.join(&t.name);
                    state.tracking.completion_toasted.insert(gid.clone());
                    let mut args = std::collections::HashMap::new();
                    args.insert(
                        std::borrow::Cow::from("name"),
                        std::borrow::Cow::from(name).into(),
                    );
                    spawn_toast(
                        state,
                        ToastGroup::Task,
                        ToastKind::Success,
                        state.fluent.get_args(Tr::DownloadComplete, &args),
                        Some(Duration::from_secs(4)),
                        false,
                    );
                    if state.settings.notifications.download_complete {
                        let title = state.fluent.get(Tr::DownloadCompleteTitle);
                        let body = state.fluent.get_args(Tr::DownloadComplete, &args);
                        let open_path_clone = open_path.clone();
                        send_system_notification(
                            state,
                            title,
                            body,
                            vec![
                                (
                                    state.fluent.get(Tr::Open),
                                    crate::notify::NotifyAction::OpenFile(open_path_clone.clone()),
                                ),
                                (
                                    state.fluent.get(Tr::Locate),
                                    crate::notify::NotifyAction::RevealDir(open_path_clone),
                                ),
                            ],
                            crate::notify::NotifyAction::OpenFile(open_path),
                        );
                    }
                    if state.shutdown.after_complete
                        && !state.tasks.values().any(|t| t.is_download_active())
                        && !matches!(state.confirm, Some(ConfirmAction::Shutdown { .. }))
                    {
                        trigger_shutdown_confirm(state);
                        return Task::batch(side_tasks);
                    }
                    return Task::batch(side_tasks);
                }
            }
            if was_error && status != "error" {
                state.tracking.error_notified.remove(&gid);
            }
            if status == "error" && !was_error {
                if let Some(t) = state.tasks.get(&gid) {
                    let name = t.name.clone();
                    if state.tracking.error_notified.insert(gid.clone()) {
                        let mut args = std::collections::HashMap::new();
                        args.insert(
                            std::borrow::Cow::from("name"),
                            std::borrow::Cow::from(name).into(),
                        );
                        if state.tracking.sync_done && state.settings.notifications.download_error {
                            let title = state.fluent.get(Tr::DownloadErrorTitle);
                            let body = state.fluent.get_args(Tr::DownloadError, &args);
                            send_system_notification(
                                state,
                                title,
                                body,
                                vec![],
                                crate::notify::NotifyAction::ActivateWindow,
                            );
                        }
                        return Task::batch(side_tasks);
                    }
                }
            }
            Task::batch(side_tasks)
        }
        EngineEvent::Removed(gid) => {
            tracing::info!(?gid, "ui: task removed");
            begin_task_exit(state, &gid, true);
            sync_global_stat_cache(state);
            refresh_tray(state);
            Task::none()
        }
        EngineEvent::TaskDetails { gid, details } => {
            tracing::debug!(?gid, "task details received");
            if state.details.gid.as_deref() == Some(&gid) {
                let first_load = state.details.loading;
                state.details.details = Some(details);
                state.details.loading = false;
                state.details.fetch_failed = false;
                let save_dir = state.tasks.get(&gid).map(|t| t.save_dir.clone());
                let tree = crate::app::details_files_tree(
                    state.details.details.as_ref(),
                    save_dir.as_deref(),
                );
                state.details.files_tree = tree;
                if first_load || state.details.files_tree.is_empty() {
                    state.details.files_expanded.clear();
                    crate::ui::components::file_tree::collect_dir_paths(
                        &state.details.files_tree,
                        &mut state.details.files_expanded,
                    );
                }
            }
            Task::none()
        }
        EngineEvent::TaskDetailsFailed { gid } => {
            tracing::debug!(?gid, "task details failed");
            if state.details.gid.as_deref() == Some(&gid) {
                state.details.loading = false;
                state.details.fetch_failed = true;
            }
            Task::none()
        }
        EngineEvent::SelectFilesFailed { gid } => {
            tracing::warn!(?gid, "change file selection failed");
            spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Warning,
                state.fluent.get(crate::i18n::Tr::SelectFilesFailed),
                Some(Duration::from_secs(4)),
                false,
            );
            Task::none()
        }
        EngineEvent::TaskAdvancedLoaded { gid, options } => {
            tracing::debug!(?gid, "task advanced options received");
            if state.details.gid.as_deref() == Some(&gid) {
                state.details.advanced_loaded = true;
                state.details.advanced_saving = false;
                if !state.details.advanced_dirty {
                    state.details.apply_advanced(&options);
                    state.details.advanced_dirty = false;
                }
            }
            Task::none()
        }
        EngineEvent::TaskAdvancedLoadFailed { gid } => {
            tracing::debug!(?gid, "task advanced options fetch failed");
            if state.details.gid.as_deref() == Some(&gid) {
                state.details.advanced_loaded = false;
                state.details.advanced_saving = false;
            }
            Task::none()
        }
        EngineEvent::TaskAdvancedApplied { gid, options } => {
            tracing::info!(?gid, "task advanced options applied");
            if state.details.gid.as_deref() == Some(&gid) {
                state.details.advanced_dirty = false;
                state.details.advanced_saving = false;
            }
            if let Some(t) = state.tasks.get_mut(&gid) {
                t.advanced = if options.is_empty() {
                    None
                } else {
                    Some(options.clone())
                };
                if let Some(ref db) = state.db {
                    db.upsert_meta(
                        &gid,
                        &t.name,
                        &t.url,
                        &t.save_dir.to_string_lossy(),
                        crate::task::TaskStatus::to_str(t.status),
                        t.added_at,
                        t.info_hash.clone().unwrap_or_default().as_str(),
                        Some(&options),
                    );
                }
            }
            spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Success,
                state.fluent.get(crate::i18n::Tr::AdvancedApplied),
                Some(Duration::from_secs(3)),
                false,
            );
            Task::none()
        }
        EngineEvent::TaskAdvancedApplyFailed { gid } => {
            tracing::warn!(?gid, "task advanced options apply failed");
            if state.details.gid.as_deref() == Some(&gid) {
                state.details.advanced_saving = false;
            }
            spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Warning,
                state.fluent.get(crate::i18n::Tr::AdvancedApplyFailed),
                Some(Duration::from_secs(4)),
                false,
            );
            Task::none()
        }
        EngineEvent::Aria2Version { version } => {
            tracing::info!(?version, "aria2 version received");
            state.engine_ui.aria2_version = Some(version.clone());
            check_updates(state, true, false)
        }
        EngineEvent::Aria2UpdateApplied { version } => {
            state.engine_ui.update_check_in_flight = false;
            state.engine_ui.aria2_version = Some(version.clone());
            state.engine_ui.aria2_downloading = false;
            state.engine_ui.aria2_downloading_version = None;
            state.engine_ui.aria2_download_progress = None;
            spawn_toast(
                state,
                ToastGroup::Engine,
                ToastKind::Success,
                format!(
                    "{} v{version}",
                    state.fluent.get(crate::i18n::Tr::UpdatedTo)
                ),
                Some(Duration::from_secs(4)),
                false,
            );
            Task::none()
        }
        EngineEvent::Aria2UpdateProgress { downloaded, total } => {
            state.engine_ui.aria2_downloading = true;
            state.engine_ui.aria2_download_progress = Some((downloaded, total));
            Task::none()
        }
        EngineEvent::Aria2UpdateFailed { error } => {
            state.engine_ui.update_check_in_flight = false;
            state.engine_ui.aria2_downloading = false;
            state.engine_ui.aria2_downloading_version = None;
            state.engine_ui.aria2_download_progress = None;
            spawn_toast(
                state,
                ToastGroup::Engine,
                ToastKind::Error,
                format!("{}: {error}", state.fluent.get(Tr::UpdateFailed)),
                Some(Duration::from_secs(6)),
                true,
            );
            Task::none()
        }
        EngineEvent::Aria2UpdateStaged { version } => {
            state.engine_ui.update_check_in_flight = false;
            state.engine_ui.aria2_downloading = false;
            state.engine_ui.aria2_downloading_version = None;
            state.engine_ui.aria2_download_progress = None;
            spawn_toast(
                state,
                ToastGroup::Engine,
                ToastKind::Normal,
                format!(
                    "aria2-next v{version} - {}",
                    state.fluent.get(crate::i18n::Tr::UpdateEngineRestart)
                ),
                Some(Duration::from_secs(5)),
                false,
            );
            Task::none()
        }
        EngineEvent::AppUpdateDownloaded { kind, path } => {
            Task::done(Message::Settings(SettingsMsg::UpdateDownloadStarted(Ok(
                crate::app_updater::AppUpdateOutcome { kind, path },
            ))))
        }
        EngineEvent::AppUpdateDownloadFailed { error } => Task::done(Message::Settings(
            SettingsMsg::UpdateDownloadStarted(Err(error)),
        )),
        EngineEvent::Aria2FetchFailed { error } => {
            let msg = format!("{}: {error}", state.fluent.get(Tr::EngineStartFailed));
            state.engine_ui.aria2_fetch_error = Some(error);
            state.engine_ui.startup_starting_toast_shown = false;
            if state.restart.engine_restart_in_progress {
                state.restart.engine_restart_in_progress = false;
                state.restart.restart_resume_gids.clear();
            }
            if let Some(id) = state.engine_ui.downloading_toast_id.take() {
                dismiss_toast(state, id);
            }
            if let Some(id) = state.engine_ui.startup_error_toast_id.take() {
                dismiss_toast(state, id);
            }
            let id = spawn_toast(state, ToastGroup::Engine, ToastKind::Error, msg, None, true);
            state.engine_ui.startup_error_toast_id = Some(id);
            if state.settings.notifications.engine_degraded && !state.engine_ui.degraded_notified {
                state.engine_ui.degraded_notified = true;
                send_system_notification(
                    state,
                    state.fluent.get(Tr::EngineDegradedTitle),
                    state.fluent.get(Tr::EngineDegradedBody),
                    vec![],
                    crate::notify::NotifyAction::ActivateWindow,
                );
            }
            Task::none()
        }
        EngineEvent::EngineDegraded { reason } => {
            state.engine_ui.aria2_fetch_error = Some(reason);
            if state.restart.engine_restart_in_progress {
                state.restart.engine_restart_in_progress = false;
                state.restart.restart_resume_gids.clear();
            }
            if state.settings.notifications.engine_degraded && !state.engine_ui.degraded_notified {
                state.engine_ui.degraded_notified = true;
                send_system_notification(
                    state,
                    state.fluent.get(Tr::EngineDegradedTitle),
                    state.fluent.get(Tr::EngineDegradedBody),
                    vec![],
                    crate::notify::NotifyAction::ActivateWindow,
                );
            }
            Task::none()
        }
        EngineEvent::GlobalSpeed { download, upload } => {
            state.global_speed = Some((download, upload));
            if let Ok(mut cache) = state.stat_cache.lock() {
                cache.download_speed = download;
                cache.upload_speed = upload;
            }
            sync_global_stat_cache(state);
            refresh_tray(state);
            Task::none()
        }
        EngineEvent::Aria2Status { stage, message } => {
            if stage == "ready" {
                state.engine_ui.aria2_fetch_error = None;
                state.engine_ui.degraded_notified = false;
            }
            if stage == "ready" || stage == "starting" {
                if let Some(id) = state.engine_ui.downloading_toast_id.take() {
                    dismiss_toast(state, id);
                }
            }
            let mut toast_task = false;
            if stage == "downloading" && state.engine_ui.downloading_toast_id.is_none() {
                let id = spawn_toast(
                    state,
                    ToastGroup::Engine,
                    ToastKind::Normal,
                    state.fluent.get(Tr::DownloadingAria2),
                    None,
                    false,
                );
                state.engine_ui.downloading_toast_id = Some(id);
                toast_task = true;
            }
            if stage == "starting" && !state.engine_ui.startup_starting_toast_shown {
                state.engine_ui.startup_starting_toast_shown = true;
                spawn_toast(
                    state,
                    ToastGroup::Engine,
                    ToastKind::Normal,
                    state.fluent.get(Tr::EngineStarting),
                    Some(Duration::from_secs(3)),
                    false,
                );
                toast_task = true;
            }
            state.engine_ui.aria2_status = Some((stage, message));
            if toast_task {
                return Task::none();
            }
            Task::none()
        }
    }
}
