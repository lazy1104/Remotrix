use std::time::Duration;

use iced::Task;

use crate::app::{
    clear_all_local, clear_completed_local, copy_to_clipboard, details_files_tree,
    open_path_in_manager, refresh_tray, schedule_details_select_flush, spawn_toast, Remotrix,
};
use crate::engine::EngineCmd;
use crate::i18n::Tr;
use crate::message::{AddField, AddTab, ConfirmAction, Message, TaskMsg};
use crate::task::TaskStatus;
use crate::ui::components::toast::{ToastGroup, ToastKind};

pub(crate) fn handle(state: &mut Remotrix, msg: TaskMsg) -> Task<Message> {
    match msg {
        TaskMsg::CopyPath(s) => copy_to_clipboard(state, s),
        TaskMsg::OpenFolder(p) => open_path_in_manager(p),
        TaskMsg::PauseTask(gid) => {
            state.tracking.paused_gids.insert(gid.clone());
            if state.handle.cmd_tx.send(EngineCmd::Pause(gid)).is_err() {
                tracing::warn!("ui: pause cmd send failed");
            }
            refresh_tray(state);
            Task::none()
        }
        TaskMsg::ResumeTask(gid) => {
            state.tracking.paused_gids.remove(&gid);
            if state.handle.cmd_tx.send(EngineCmd::Resume(gid)).is_err() {
                tracing::warn!("ui: resume cmd send failed");
            }
            refresh_tray(state);
            Task::none()
        }
        TaskMsg::RedownloadTask(gid) => {
            state.tracking.paused_gids.remove(&gid);
            state.tracking.torrent_followed.remove(&gid);
            let bt_metadata_only = !state.settings.aria2.bt_auto_download;
            let (url, save_dir, split) = match state.tasks.get(&gid) {
                Some(t) => {
                    let url = if !t.url.is_empty() {
                        t.url.clone()
                    } else {
                        let hash = t.info_hash.clone().unwrap_or_default();
                        if hash.is_empty() {
                            tracing::warn!(?gid, "ui: redownload skipped (no url or info hash)");
                            return Task::none();
                        }
                        format!("magnet:?xt=urn:btih:{hash}")
                    };
                    (url, t.save_dir.clone(), state.settings.split)
                }
                None => return Task::none(),
            };
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::Redownload {
                    gid: gid.clone(),
                    url,
                    save_dir,
                    split,
                    bt_metadata_only,
                })
                .is_err()
            {
                tracing::warn!("ui: redownload cmd send failed");
            }
            if let Some(t) = state.tasks.get_mut(&gid) {
                t.downloaded = 0;
                t.total = 0;
                t.speed = 0;
                t.upload_speed = 0;
                t.connections = 0;
            }
            Task::none()
        }
        TaskMsg::RemoveTask(gid) => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            state.tracking.paused_gids.remove(&gid);
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::Remove {
                    gid,
                    delete_files: false,
                })
                .is_err()
            {
                tracing::warn!("ui: remove cmd send failed");
            }
            state.confirm_anim.begin_exit();
            let _ = spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::TaskRemoved),
                Some(Duration::from_secs(3)),
                false,
            );
            Task::none()
        }
        TaskMsg::DeleteTask(gid) => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            state.tracking.paused_gids.remove(&gid);
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::Remove {
                    gid,
                    delete_files: true,
                })
                .is_err()
            {
                tracing::warn!("ui: delete cmd send failed");
            }
            state.confirm_anim.begin_exit();
            let _ = spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::TaskDeleted),
                Some(Duration::from_secs(3)),
                false,
            );
            Task::none()
        }
        TaskMsg::StartAll => {
            state.tracking.paused_gids.clear();
            if state.handle.cmd_tx.send(EngineCmd::ResumeAll).is_err() {
                tracing::warn!("ui: resume all cmd send failed");
            }
            refresh_tray(state);
            Task::none()
        }
        TaskMsg::PauseAll => {
            state.tracking.paused_gids.extend(
                state
                    .tasks
                    .values()
                    .filter(|t| t.status == TaskStatus::Active)
                    .map(|t| t.gid.clone()),
            );
            if state.handle.cmd_tx.send(EngineCmd::PauseAll).is_err() {
                tracing::warn!("ui: pause all cmd send failed");
            }
            refresh_tray(state);
            Task::none()
        }
        TaskMsg::DeleteAll => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::RemoveAll { delete_files: true })
                .is_err()
            {
                tracing::warn!("ui: remove all cmd send failed");
            }
            clear_all_local(state);
            state.confirm_anim.begin_exit();
            let _ = spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::TasksDeleted),
                Some(Duration::from_secs(3)),
                false,
            );
            Task::none()
        }
        TaskMsg::RemoveAllRecords => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::RemoveAll {
                    delete_files: false,
                })
                .is_err()
            {
                tracing::warn!("ui: remove all records cmd send failed");
            }
            clear_all_local(state);
            state.confirm_anim.begin_exit();
            let _ = spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::TasksRemoved),
                Some(Duration::from_secs(3)),
                false,
            );
            Task::none()
        }
        TaskMsg::ClearCompleted => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            let completed: Vec<String> = state
                .tasks
                .iter()
                .filter(|(_, t)| matches!(t.status, TaskStatus::Completed | TaskStatus::Removed))
                .map(|(gid, _)| gid.clone())
                .collect();
            clear_completed_local(state, &completed);
            state.confirm_anim.begin_exit();
            Task::none()
        }
        TaskMsg::Refresh => {
            if state.handle.cmd_tx.send(EngineCmd::Snapshot).is_err() {
                tracing::warn!("ui: snapshot cmd send failed");
            }
            Task::none()
        }
        TaskMsg::OpenTaskDetails(gid) => {
            state.details.select_gen = 0;
            state.details.pending_select = None;
            state.details.open(gid.clone());
            state.details_anim.open();
            if let Some(a) = state.tasks.get(&gid).and_then(|t| t.advanced.as_ref()) {
                state.details.apply_advanced(a);
                state.details.advanced_loaded = true;
                state.details.advanced_dirty = false;
                state.details.advanced_saving = false;
            }
            if state
                .tasks
                .get(&gid)
                .map(|t| t.is_completed())
                .unwrap_or(false)
            {
                state.details.loading = false;
            } else if state
                .handle
                .cmd_tx
                .send(EngineCmd::FetchTaskDetails(gid.clone()))
                .is_err()
            {
                tracing::warn!("fetch task details cmd send failed");
            } else if state
                .handle
                .cmd_tx
                .send(EngineCmd::FetchTaskAdvanced(gid))
                .is_err()
            {
                tracing::warn!("fetch task advanced cmd send failed");
            }
            Task::none()
        }
        TaskMsg::CloseTaskDetails => {
            state.details.select_gen += 1;
            if let Some((gid, files)) = state.details.pending_select.take() {
                let _ = state
                    .handle
                    .cmd_tx
                    .send(EngineCmd::SelectFiles { gid, files });
            }
            state.details_anim.begin_exit();
            Task::none()
        }
        TaskMsg::RefreshTaskDetails => {
            if state.details.is_visible() && !state.details.fetch_failed {
                if let Some(ref gid) = state.details.gid {
                    let queryable = state
                        .tasks
                        .get(gid)
                        .map(|t| !t.is_completed())
                        .unwrap_or(false);
                    if queryable
                        && state
                            .handle
                            .cmd_tx
                            .send(EngineCmd::FetchTaskDetails(gid.clone()))
                            .is_err()
                    {
                        tracing::warn!("refresh task details cmd send failed");
                    }
                    if queryable
                        && !state.details.advanced_dirty
                        && state
                            .handle
                            .cmd_tx
                            .send(EngineCmd::FetchTaskAdvanced(gid.clone()))
                            .is_err()
                    {
                        tracing::warn!("refresh task advanced cmd send failed");
                    }
                }
            }
            Task::none()
        }
        TaskMsg::DetailsAdvancedFieldChanged(field, value) => {
            let d = &mut state.details;
            match field {
                AddField::UserAgent => d.user_agent = value,
                AddField::HttpUser => d.http_user = value,
                AddField::HttpPasswd => d.http_passwd = value,
                AddField::Referer => d.referer = value,
                AddField::Cookie => d.cookie = value,
                AddField::ProxyServer => d.proxy_server = value,
                AddField::ProxyUsername => d.proxy_username = value,
                AddField::ProxyPassword => d.proxy_password = value,
                AddField::Out => {}
            }
            state.details.advanced_dirty = true;
            Task::none()
        }
        TaskMsg::DetailsAdvancedSave => {
            if let Some(ref gid) = state.details.gid {
                let advanced = state.details.to_advanced();
                state.details.advanced_saving = true;
                if state
                    .handle
                    .cmd_tx
                    .send(EngineCmd::ChangeTaskAdvanced {
                        gid: gid.clone(),
                        advanced,
                    })
                    .is_err()
                {
                    tracing::warn!("change task advanced cmd send failed");
                    state.details.advanced_saving = false;
                }
            }
            Task::none()
        }
        TaskMsg::MetadataProbeResult {
            gid,
            incoming,
            size,
            name,
        } => {
            if let Some(t) = state.tasks.get_mut(&gid) {
                if size.is_some() && size != t.metadata_probe_size {
                    t.metadata_probe_size = size;
                }
                let placeholder =
                    t.name.is_empty() || t.name.starts_with("[METADATA]") || t.name == "magnet:";
                if placeholder {
                    if let Some(real) = name {
                        t.name = real;
                        if let Some(ref db) = state.db {
                            db.update_name(&gid, &t.name, t.metadata_only);
                        }
                    } else if !t.name.starts_with("[METADATA]") {
                        t.name = incoming;
                        if let Some(ref db) = state.db {
                            db.update_name(&gid, &t.name, t.metadata_only);
                        }
                    }
                }
            }
            Task::none()
        }
        TaskMsg::DetailsTreeExpand(path) => {
            if state.details.files_expanded.contains(&path) {
                state.details.files_expanded.remove(&path);
            } else {
                state.details.files_expanded.insert(path);
            }
            Task::none()
        }
        TaskMsg::DetailsTreeToggle(path) => {
            let Some(details) = state.details.details.clone() else {
                return Task::none();
            };
            let Some(gid) = state.details.gid.clone() else {
                return Task::none();
            };
            let save_dir = state.tasks.get(&gid).map(|t| t.save_dir.clone());
            let tree = details_files_tree(Some(&details), save_dir.as_deref());
            let Some(node) = crate::ui::components::file_tree::find_node(&tree, &path) else {
                return Task::none();
            };
            let indices = crate::ui::components::file_tree::descendant_indices(node);
            let mut pairs: Vec<(u64, bool)> = details
                .files
                .iter()
                .map(|f| (f.index, f.selected))
                .collect();
            crate::ui::components::file_tree::flip_with_guard(&mut pairs, &indices);
            if let Some(ref mut details) = state.details.details {
                for (idx, selected) in pairs {
                    if let Some(file) = details.files.iter_mut().find(|f| f.index == idx) {
                        file.selected = selected;
                    }
                }
            }
            schedule_details_select_flush(state)
        }
        TaskMsg::DetailsFilesSelectAll => {
            if let Some(ref mut details) = state.details.details {
                for file in &mut details.files {
                    file.selected = true;
                }
            }
            schedule_details_select_flush(state)
        }
        TaskMsg::DetailsFilesSelectNone => {
            if let Some(ref mut details) = state.details.details {
                for file in &mut details.files {
                    file.selected = false;
                }
                if let Some(first) = details.files.iter_mut().min_by_key(|f| f.index) {
                    first.selected = true;
                }
            }
            schedule_details_select_flush(state)
        }
        TaskMsg::DetailsFilesScroll(off) => {
            state.details.files_scroll_offset = off;
            Task::none()
        }
        TaskMsg::DetailsFilesFlush(gen) => {
            if gen != state.details.select_gen {
                return Task::none();
            }
            if let Some((gid, files)) = state.details.pending_select.take() {
                let _ = state.handle.cmd_tx.send(EngineCmd::SelectFiles {
                    gid: gid.clone(),
                    files,
                });
                let _ = state.handle.cmd_tx.send(EngineCmd::FetchTaskDetails(gid));
            }
            Task::none()
        }
        TaskMsg::OpenTaskFile(gid) => {
            let Some(t) = state.tasks.get(&gid).cloned() else {
                return Task::none();
            };
            let metadata_preview = t.metadata_only || t.name.starts_with("[METADATA]");
            let path = if metadata_preview {
                match t.info_hash.as_deref() {
                    Some(hash) => t.save_dir.join(format!("{hash}.torrent")),
                    None => t.save_dir.join(&t.name),
                }
            } else {
                t.save_dir.join(&t.name)
            };
            if path.exists() && (metadata_preview || crate::engine::is_torrent_url(&t.name)) {
                let default_dir = if t.save_dir.as_os_str().is_empty() {
                    state.settings.download_dir.clone()
                } else {
                    t.save_dir.clone()
                };
                state.add_dialog.save_picker.close_history();
                state.add_dialog.open(default_dir, state.settings.split);
                state.add_dialog_anim.open();
                state
                    .add_dialog
                    .set_torrent_path(path.to_string_lossy().to_string());
                state.add_dialog.active_tab = AddTab::Torrent;
                return Task::none();
            }
            if path.exists() {
                return Task::perform(
                    async move {
                        let _ = open::that(&path);
                    },
                    |_| Message::Noop,
                );
            }
            let is_bt = t.info_hash.is_some()
                || crate::engine::is_torrent_url(&t.url)
                || crate::engine::is_magnet_url(&t.url);
            if is_bt {
                let hash = t.info_hash.as_deref().filter(|h| !h.is_empty());
                let has_url =
                    crate::engine::is_magnet_url(&t.url) || crate::engine::is_torrent_url(&t.url);
                if hash.is_some() || has_url {
                    let default_dir = if t.save_dir.as_os_str().is_empty() {
                        state.settings.download_dir.clone()
                    } else {
                        t.save_dir.clone()
                    };
                    state.add_dialog.save_picker.close_history();
                    state.add_dialog.open(default_dir, state.settings.split);
                    if let Some(h) = hash {
                        state
                            .add_dialog
                            .set_urls(vec![format!("magnet:?xt=urn:btih:{h}")]);
                    } else {
                        state.add_dialog.set_urls(vec![t.url.clone()]);
                    }
                    state.add_dialog.active_tab = AddTab::Url;
                    state.add_dialog_anim.open();
                    return Task::none();
                }
            }
            if t.status == TaskStatus::Completed {
                if state.settings.remove_task_if_files_missing {
                    state.tracking.paused_gids.remove(&gid);
                    let _ = state.handle.cmd_tx.send(EngineCmd::Remove {
                        gid: gid.clone(),
                        delete_files: false,
                    });
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
                state.confirm = Some(ConfirmAction::RemoveMissingFileTask(gid));
                state.confirm_anim.open();
                return Task::none();
            }
            spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Warning,
                state.fluent.get(Tr::FileMissing),
                Some(Duration::from_secs(4)),
                false,
            );
            Task::none()
        }
        TaskMsg::OpenTaskFolder(gid) => {
            let dir = state
                .tasks
                .get(&gid)
                .map(|t| t.save_dir.clone())
                .unwrap_or_default();
            if !dir.as_os_str().is_empty() {
                return Task::perform(
                    async move {
                        let _ = open::that(&dir);
                    },
                    |_| Message::Noop,
                );
            }
            Task::none()
        }
        TaskMsg::CopyTaskLink(gid) => {
            let Some(t) = state.tasks.get(&gid) else {
                return Task::none();
            };
            let content = if !t.url.is_empty() {
                t.url.clone()
            } else if let Some(hash) = t.info_hash.as_deref() {
                if hash.is_empty() {
                    return Task::none();
                }
                format!("magnet:?xt=urn:btih:{hash}")
            } else {
                return Task::none();
            };
            copy_to_clipboard(state, content)
        }
    }
}
