use std::path::PathBuf;

use iced::Task;

use crate::app::{
    apply_path, open_add_dialog, open_path_in_manager, pick_path, picker_mut, set_page, Remotrix,
};
use crate::engine::EngineCmd;
use crate::message::{AddField, AddMsg, AddTab, Message, PathPickerId, WindowMsg};
use crate::task::TaskAdvancedOptions;
use crate::ui::components::path_picker::PathPickerAction;
use crate::ui::components::torrent_upload::TorrentUploadAction;

pub(crate) fn handle(state: &mut Remotrix, msg: AddMsg) -> Task<Message> {
    match msg {
        AddMsg::OpenAddDialog => {
            open_add_dialog(state);
            Task::none()
        }
        AddMsg::CancelAdd => {
            state.add_dialog.save_picker.close_history();
            state.add_dialog_anim.begin_exit();
            Task::none()
        }
        AddMsg::SelectAddTab(tab) => {
            state.add_dialog.active_tab = tab;
            Task::none()
        }
        AddMsg::TorrentUpload(event) => {
            if let Some(TorrentUploadAction::Browse) = state.add_dialog.handle_torrent_event(event)
            {
                return pick_path(PathPickerId::Torrent);
            }
            Task::none()
        }
        AddMsg::MetalinkUpload(event) => {
            if state.add_dialog.handle_torrent_event(event).is_some() {
                return pick_path(PathPickerId::Metalink);
            }
            Task::none()
        }
        AddMsg::TorrentTreeExpand(path) => {
            state.add_dialog.toggle_torrent_expand(&path);
            Task::none()
        }
        AddMsg::TorrentTreeToggle(path) => {
            state.add_dialog.toggle_torrent_node(&path);
            Task::none()
        }
        AddMsg::TorrentFilesSelectAll => {
            state.add_dialog.set_all_torrent_files(true);
            Task::none()
        }
        AddMsg::TorrentFilesSelectNone => {
            state.add_dialog.set_all_torrent_files(false);
            Task::none()
        }
        AddMsg::TorrentFilesScroll(off) => {
            state.add_dialog.torrent_scroll_offset = off;
            Task::none()
        }
        AddMsg::TorrentFilesTogglePanel => {
            state.add_dialog.toggle_torrent_panel();
            Task::none()
        }
        AddMsg::FileHovered => {
            state.drop_hover = true;
            if state.add_dialog.is_visible() {
                if state.add_dialog.active_tab == AddTab::Torrent {
                    state.add_dialog.torrent_upload.set_dragging(true);
                } else if state.add_dialog.active_tab == AddTab::Metalink {
                    state.add_dialog.metalink_upload.set_dragging(true);
                }
            }
            Task::none()
        }
        AddMsg::FilesHoveredLeft => {
            state.drop_hover = false;
            if state.add_dialog.is_visible() {
                state.add_dialog.torrent_upload.set_dragging(false);
                state.add_dialog.metalink_upload.set_dragging(false);
            }
            Task::none()
        }
        AddMsg::FileDropped(path) => {
            state.drop_hover = false;
            if state.add_dialog.is_visible() {
                state.add_dialog.torrent_upload.set_dragging(false);
            }
            if state.window.show_close_dialog
                || state.about_dialog_visible
                || state.confirm.is_some()
                || state.update_dialog.is_some()
            {
                return Task::none();
            }
            let prefs = state.settings.clipboard_types;
            let path_str = path.to_string_lossy().to_string();
            Task::perform(
                async move { crate::clipboard_watch::parse_clipboard(&path_str, prefs) },
                |payload| Message::Window(WindowMsg::DroppedFileParsed(payload)),
            )
        }
        AddMsg::UrlEditor(action) => {
            state.add_dialog.url_editor.perform(action);
            Task::none()
        }
        AddMsg::PathPicker(id, event) => {
            let action = picker_mut(state, id).update(event);
            match action {
                Some(PathPickerAction::Copy(s)) => {
                    return iced::clipboard::write::<Message>(s);
                }
                Some(PathPickerAction::Browse) => {
                    return pick_path(id);
                }
                Some(PathPickerAction::Select(p)) => {
                    apply_path(state, id, p);
                }
                Some(PathPickerAction::Open(p)) => {
                    return open_path_in_manager(p);
                }
                None => {}
            }
            Task::none()
        }
        AddMsg::PathPicked(id, maybe_path) => {
            tracing::debug!(?id, picked = maybe_path.is_some(), "ui: path picked");
            if let Some(p) = maybe_path {
                apply_path(state, id, p);
            }
            Task::none()
        }
        AddMsg::SplitChanged(value) => {
            if let Ok(n) = value.parse::<u16>() {
                state.add_dialog.split = n.max(1);
            }
            Task::none()
        }
        AddMsg::ToggleAdvanced(value) => {
            state.add_dialog.advanced_open = value;
            Task::none()
        }
        AddMsg::AddFieldChanged(field, value) => {
            let add = &mut state.add_dialog;
            match field {
                AddField::Out => add.out = value,
                AddField::UserAgent => add.user_agent = value,
                AddField::HttpUser => add.http_user = value,
                AddField::HttpPasswd => add.http_passwd = value,
                AddField::Referer => add.referer = value,
                AddField::Cookie => add.cookie = value,
                AddField::ProxyServer => add.proxy_server = value,
                AddField::ProxyUsername => add.proxy_username = value,
                AddField::ProxyPassword => add.proxy_password = value,
            }
            Task::none()
        }
        AddMsg::AddDownload => {
            if state.add_dialog_anim.is_dismissing() {
                return Task::none();
            }
            if state.add_dialog.can_submit() {
                let nav = state.settings.nav_to_tasks_after_add;

                let advanced = TaskAdvancedOptions {
                    out: if state.add_dialog.url_count() == 1 {
                        state.add_dialog.out.clone()
                    } else {
                        String::new()
                    },
                    user_agent: state.add_dialog.user_agent.clone(),
                    http_user: state.add_dialog.http_user.clone(),
                    http_passwd: state.add_dialog.http_passwd.clone(),
                    referer: state.add_dialog.referer.clone(),
                    cookie: state.add_dialog.cookie.clone(),
                    proxy_server: state.add_dialog.proxy_server.clone(),
                    proxy_username: state.add_dialog.proxy_username.clone(),
                    proxy_password: state.add_dialog.proxy_password.clone(),
                };

                let tpath_str = state.add_dialog.torrent_upload.path().to_string();
                if !tpath_str.is_empty() && state.add_dialog.active_tab == AddTab::Torrent {
                    let tpath = PathBuf::from(&tpath_str);
                    let save_dir = PathBuf::from(state.add_dialog.save_picker.value());
                    let mut torrent_advanced = advanced.clone();
                    torrent_advanced.out.clear();
                    let total_files = state.add_dialog.torrent_files.len();
                    let selected = state.add_dialog.selected_file_indices();
                    let select_files = if total_files == 0 || selected.len() == total_files {
                        None
                    } else {
                        Some(selected)
                    };
                    if state
                        .handle
                        .cmd_tx
                        .send(EngineCmd::AddTorrent {
                            path: tpath,
                            save_dir,
                            split: state.add_dialog.split,
                            advanced: torrent_advanced,
                            select_files,
                        })
                        .is_err()
                    {
                        tracing::warn!("ui: add torrent cmd send failed");
                    }
                    tracing::info!("ui: torrent submitted");
                    state.add_dialog_anim.begin_exit();
                    if nav {
                        set_page(state, crate::message::Page::Tasks);
                    }
                    return Task::none();
                }

                let mpath_str = state.add_dialog.metalink_upload.path().to_string();
                if !mpath_str.is_empty() && state.add_dialog.active_tab == AddTab::Metalink {
                    let mpath = PathBuf::from(&mpath_str);
                    let save_dir = PathBuf::from(state.add_dialog.save_picker.value());
                    let mut metalink_advanced = advanced.clone();
                    metalink_advanced.out.clear();
                    if state
                        .handle
                        .cmd_tx
                        .send(EngineCmd::AddMetalink {
                            path: mpath,
                            save_dir,
                            split: state.add_dialog.split,
                            advanced: metalink_advanced,
                        })
                        .is_err()
                    {
                        tracing::warn!("ui: add metalink cmd send failed");
                    }
                    tracing::info!("ui: metalink submitted");
                    state.add_dialog_anim.begin_exit();
                    if nav {
                        set_page(state, crate::message::Page::Tasks);
                    }
                    return Task::none();
                }

                let urls: Vec<String> = state
                    .add_dialog
                    .url_editor
                    .text()
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                if !urls.is_empty() {
                    let save_dir = PathBuf::from(state.add_dialog.save_picker.value());
                    let bt_metadata_only = !state.settings.aria2.bt_auto_download;
                    if state
                        .handle
                        .cmd_tx
                        .send(EngineCmd::AddDownload {
                            urls: urls.clone(),
                            save_dir,
                            split: state.add_dialog.split,
                            advanced,
                            bt_metadata_only,
                        })
                        .is_err()
                    {
                        tracing::warn!("ui: add download cmd send failed");
                    }
                    tracing::info!(count = urls.len(), "ui: add download submitted");
                    state.add_dialog_anim.begin_exit();
                    if nav {
                        set_page(state, crate::message::Page::Tasks);
                    }
                } else {
                    tracing::debug!("ui: add download skipped (no urls after filter)");
                }
            }
            Task::none()
        }
    }
}
