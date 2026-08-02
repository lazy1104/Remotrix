use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::alignment::{Horizontal, Vertical};
use iced::futures::SinkExt;
use iced::widget::{column, container, row, stack, text_editor};
use iced::window::Id;
use iced::{Element, Length, Padding, Subscription, Task};
use sha2::{Digest, Sha256};

use crate::config::{self, Settings};
use crate::db::Db;
use crate::engine::{EngineCmd, EngineEvent, EngineHandle, EventRx, TaskAdvancedOptions};
use crate::i18n::{Fluent, Locale, Tr};
use crate::message::{
    AddField, AddTab, CloseDialogChoice, ConfirmAction, Message, Page, PathPickerId, SettingKey,
    SettingsCategory, SortField, SortOrder, TaskFilter, WindowCmd,
};
use crate::task::{DownloadTask, TaskStatus};
use crate::ui::add_dialog::AddDialogState;
use crate::ui::category_bar::Counts;
use crate::ui::components::file_tree::FileTreeNode;
use crate::ui::components::path_picker::PathPickerAction;
use crate::ui::components::toast::{Toast, ToastKind, ToastPosition};
use crate::ui::components::torrent_upload::{self, TorrentUploadAction};
use crate::ui::details_dialog::DetailsDialogState;
use crate::ui::icons::{CATEGORY_W, SIDEBAR_W};
use crate::ui::settings_page::SettingsUiState;
use crate::ui::theme::{self, ThemeMode};
pub struct Remotrix {
    page: Page,
    task_filter: TaskFilter,
    settings_cat: SettingsCategory,
    tasks: HashMap<String, DownloadTask>,
    task_order: Vec<String>,
    handle: EngineHandle,
    event_rx_slot: Arc<Mutex<Option<EventRx>>>,
    add_dialog: AddDialogState,
    about_dialog_visible: bool,
    settings: Settings,
    fluent: Fluent,
    theme: iced::Theme,
    maximized: bool,
    show_close_dialog: bool,
    window_id: Option<Id>,
    sort_menu_open: bool,
    sort_field: SortField,
    sort_order: SortOrder,
    search_query: String,
    aria2_version: Option<String>,
    aria2_check_msg: Option<String>,
    update_pending: Option<String>,
    aria2_status: Option<(String, String)>,
    aria2_fetch_error: Option<String>,
    logo_handle: iced::widget::image::Handle,
    ua_editor: text_editor::Content,
    torrent_files: HashMap<String, PathBuf>,
    torrent_followed: HashSet<String>,
    db: Option<Db>,
    dirty: HashSet<String>,
    details: DetailsDialogState,
    details_pending_select: Option<(String, Vec<u64>)>,
    details_select_gen: u64,
    window_size: iced::Size,
    last_resize: Option<iced::Size>,
    geometry_dirty: bool,
    pending_close: bool,
    closing: bool,
    confirm: Option<ConfirmAction>,
    applied_settings: Settings,
    settings_ui: SettingsUiState,
    global_speed: Option<(u64, u64)>,
    paused_gids: HashSet<String>,
    synced_gids: HashSet<String>,
    sync_done: bool,
    active_count: usize,
    toasts: Vec<crate::ui::components::toast::Toast>,
    next_toast_id: u64,
    hovered_toast_id: Option<u64>,
    downloading_toast_id: Option<u64>,
    startup_error_toast_id: Option<u64>,
    startup_starting_toast_shown: bool,
}

pub fn init() -> (Remotrix, Task<Message>) {
    config::announce();
    let settings = config::load();

    let ua_editor = text_editor::Content::with_text(&settings.aria2.user_agent);

    let window_w = settings.window_width;
    let window_h = settings.window_height;
    let window_maximized = settings.window_maximized;

    let (handle, event_rx) = crate::engine::spawn_engine();

    let settings_ui = SettingsUiState::new(&settings);
    let add_dialog = AddDialogState::new(settings.download_dir.clone());
    let fluent = Fluent::new(settings.locale);

    let theme = theme::build_iced(
        settings_accent(&settings),
        theme::resolve_mode(settings.theme_mode, None),
    );
    let logo_handle =
        iced::widget::image::Handle::from_bytes(&include_bytes!("../assets/icon.png")[..]);

    let db = crate::config::db_path().and_then(|p| Db::open(&p).ok());
    let (tasks, task_order) = if let Some(ref db) = db {
        let loaded = db.load_all();
        let order: Vec<String> = loaded.iter().map(|t| t.gid.clone()).collect();
        let map: HashMap<String, DownloadTask> =
            loaded.into_iter().map(|t| (t.gid.clone(), t)).collect();
        (map, order)
    } else {
        (HashMap::new(), Vec::new())
    };
    let active_count = tasks
        .values()
        .filter(|t| t.status == TaskStatus::Active)
        .count();

    let state = Remotrix {
        page: Page::Tasks,
        task_filter: TaskFilter::All,
        settings_cat: SettingsCategory::General,
        tasks,
        task_order,
        handle,
        event_rx_slot: Arc::new(Mutex::new(Some(event_rx))),
        add_dialog,
        about_dialog_visible: false,
        applied_settings: settings.clone(),
        settings,
        fluent,
        theme,
        maximized: window_maximized,
        show_close_dialog: false,
        window_id: None,
        sort_menu_open: false,
        sort_field: SortField::AddedTime,
        sort_order: SortOrder::Desc,
        search_query: String::new(),
        aria2_version: None,
        aria2_check_msg: None,
        update_pending: None,
        aria2_status: None,
        aria2_fetch_error: None,
        logo_handle,
        ua_editor,
        torrent_files: HashMap::new(),
        torrent_followed: HashSet::new(),
        db,
        dirty: HashSet::new(),
        details: DetailsDialogState::new(),
        details_pending_select: None,
        details_select_gen: 0,
        window_size: iced::Size::new(window_w, window_h),
        last_resize: None,
        geometry_dirty: false,
        pending_close: false,
        closing: false,
        confirm: None,
        settings_ui,
        global_speed: None,
        paused_gids: HashSet::new(),
        synced_gids: HashSet::new(),
        sync_done: false,
        active_count,
        toasts: Vec::new(),
        next_toast_id: 0,
        hovered_toast_id: None,
        downloading_toast_id: None,
        startup_error_toast_id: None,
        startup_starting_toast_shown: false,
    };

    (state, Task::none())
}

pub fn app_title(_state: &Remotrix) -> String {
    "Remotrix".to_string()
}

pub fn theme(state: &Remotrix) -> iced::Theme {
    state.theme.clone()
}

fn settings_accent(settings: &Settings) -> iced::Color {
    theme::accent_color(&settings.theme_color)
}

fn rebuild_theme(state: &mut Remotrix) {
    let dark = theme::resolve_mode(state.settings.theme_mode, None);
    state.theme = theme::build_iced(settings_accent(&state.settings), dark);
}

fn sync_geometry_to_settings(state: &mut Remotrix) {
    state.settings.window_width = state.window_size.width;
    state.settings.window_height = state.window_size.height;
    state.settings.window_maximized = state.maximized;
}

fn revert_apply_settings(state: &mut Remotrix) {
    state.settings.download_dir = state.applied_settings.download_dir.clone();
    state
        .settings_ui
        .download_picker
        .set_value(state.settings.download_dir.to_string_lossy());
    state.settings.max_concurrent = state.applied_settings.max_concurrent;
    state.settings.download_limit_kb = state.applied_settings.download_limit_kb;
    state.settings.upload_limit_kb = state.applied_settings.upload_limit_kb;
    state.settings.split = state.applied_settings.split;
    state.settings.nav_to_tasks_after_add = state.applied_settings.nav_to_tasks_after_add;
    state.settings.delete_torrent_after_complete =
        state.applied_settings.delete_torrent_after_complete;
    state.settings.cleanup_completed_on_close = state.applied_settings.cleanup_completed_on_close;
    state.settings.remove_task_if_files_missing =
        state.applied_settings.remove_task_if_files_missing;
    state.settings.aria2 = state.applied_settings.aria2.clone();
    state
        .settings_ui
        .ed2k_server_list_picker
        .set_value(state.settings.aria2.ed2k_server_list.clone());
    state
        .settings_ui
        .ed2k_node_list_picker
        .set_value(state.settings.aria2.ed2k_node_list.clone());
    state.ua_editor = text_editor::Content::with_text(&state.settings.aria2.user_agent);
}

fn clear_all_local(state: &mut Remotrix) {
    state.tasks.clear();
    state.task_order.clear();
    state.dirty.clear();
    state.active_count = 0;
    state.paused_gids.clear();
    if let Some(ref db) = state.db {
        db.delete_all();
    }
}

fn remove_task_local(state: &mut Remotrix, gid: &str) {
    if let Some(t) = state.tasks.get(gid) {
        if t.status == TaskStatus::Active {
            state.active_count = state.active_count.saturating_sub(1);
        }
    }
    let _ = state.torrent_files.remove(gid);
    state.torrent_followed.remove(gid);
    state.paused_gids.remove(gid);
    state.tasks.remove(gid);
    state.task_order.retain(|g| g != gid);
    state.dirty.remove(gid);
    if let Some(ref db) = state.db {
        db.delete(gid);
    }
}

fn clear_completed_local(state: &mut Remotrix, gids: &[String]) {
    if let Some(ref db) = state.db {
        db.clear_completed(gids);
    }
    if !gids.is_empty()
        && state
            .handle
            .cmd_tx
            .send(EngineCmd::PurgeResults(gids.to_vec()))
            .is_err()
    {
        tracing::warn!("ui: purge results cmd send failed");
    }
    for gid in gids {
        state.dirty.remove(gid);
    }
    state
        .tasks
        .retain(|_k, t| !matches!(t.status, TaskStatus::Completed | TaskStatus::Removed));
    state.task_order.retain(|gid| state.tasks.contains_key(gid));
}

fn flush_dirty(state: &mut Remotrix) {
    if state.dirty.is_empty() {
        return;
    }
    let batch: Vec<(String, u64, u64, u64, u64, u64, String)> = state
        .dirty
        .iter()
        .filter_map(|gid| {
            state.tasks.get(gid).map(|t| {
                let status = match t.status {
                    TaskStatus::Waiting => "waiting",
                    TaskStatus::Active => "active",
                    TaskStatus::Paused => "paused",
                    TaskStatus::Completed => "complete",
                    TaskStatus::Error => "error",
                    TaskStatus::Removed => "removed",
                };
                (
                    gid.clone(),
                    t.downloaded,
                    t.total,
                    t.speed,
                    t.upload_speed,
                    t.connections,
                    status.to_string(),
                )
            })
        })
        .collect();
    if let Some(ref db) = state.db {
        db.flush(&batch);
    }
    state.dirty.clear();
}

fn begin_close(state: &mut Remotrix) -> Task<Message> {
    if state.closing {
        return Task::none();
    }
    state.closing = true;
    state.show_close_dialog = false;
    state.details_select_gen += 1;
    if let Some((gid, files)) = state.details_pending_select.take() {
        let _ = state
            .handle
            .cmd_tx
            .send(EngineCmd::SelectFiles { gid, files });
    }
    if state.settings.cleanup_completed_on_close {
        let completed: Vec<String> = state
            .tasks
            .iter()
            .filter(|(_, t)| matches!(t.status, TaskStatus::Completed | TaskStatus::Removed))
            .map(|(gid, _)| gid.clone())
            .collect();
        clear_completed_local(state, &completed);
    }
    tracing::info!("ui: shutdown requested");
    if state.handle.cmd_tx.send(EngineCmd::Shutdown).is_err() {
        tracing::warn!("ui: shutdown cmd send failed");
    }
    shutdown_timeout_task()
}

fn shutdown_timeout_task() -> Task<Message> {
    Task::perform(
        async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
        },
        |_| Message::ShutdownTimeout,
    )
}

fn finalize_close(state: &mut Remotrix) -> Task<Message> {
    if !state.closing {
        return Task::none();
    }
    state.closing = false;
    flush_dirty(state);
    if state.geometry_dirty {
        state.pending_close = true;
        if let Some(id) = state.window_id {
            return iced::window::is_maximized(id)
                .then(|max| Task::done(Message::WindowMaximized(max)));
        }
    }
    sync_geometry_to_settings(state);
    config::save(&state.settings);
    if let Some(id) = state.window_id {
        iced::window::close::<Message>(id)
    } else {
        Task::none()
    }
}

fn read_clipboard(state: &Remotrix) -> Task<Message> {
    if !state.settings.detect_clipboard_on_start {
        return Task::none();
    }
    iced::clipboard::read().map(Message::ClipboardRead)
}

pub fn update(state: &mut Remotrix, message: Message) -> Task<Message> {
    match message {
        Message::NavigatePage(page) => {
            state.settings_ui.download_picker.close_history();
            if page == Page::Tasks
                && state.page == Page::Settings
                && !state.settings.apply_fields_equal(&state.applied_settings)
            {
                state.confirm = Some(ConfirmAction::LeaveSettings { target: page });
            } else {
                state.page = page;
            }
        }
        Message::SetTaskFilter(filter) => {
            state.task_filter = filter;
        }
        Message::SetSettingsCategory(cat) => {
            state.settings_ui.download_picker.close_history();
            state.settings_cat = cat;
        }
        Message::OpenAddDialog => {
            state.add_dialog.save_picker.close_history();
            state
                .add_dialog
                .open(state.settings.download_dir.clone(), state.settings.split);
        }
        Message::CancelAdd => {
            state.add_dialog.save_picker.close_history();
            state.add_dialog.close();
        }
        Message::SelectAddTab(tab) => {
            state.add_dialog.active_tab = tab;
        }
        Message::TorrentUpload(event) => {
            if let Some(TorrentUploadAction::Browse) = state.add_dialog.handle_torrent_event(event)
            {
                return pick_path(PathPickerId::Torrent);
            }
        }
        Message::TorrentTreeExpand(path) => {
            state.add_dialog.toggle_torrent_expand(&path);
        }
        Message::TorrentTreeToggle(path) => {
            state.add_dialog.toggle_torrent_node(&path);
        }
        Message::TorrentFilesSelectAll => {
            state.add_dialog.set_all_torrent_files(true);
        }
        Message::TorrentFilesSelectNone => {
            state.add_dialog.set_all_torrent_files(false);
        }
        Message::TorrentFilesScroll(off) => {
            state.add_dialog.torrent_scroll_offset = off;
        }
        Message::TorrentFilesTogglePanel => {
            state.add_dialog.toggle_torrent_panel();
        }
        Message::FileHovered(_) => {
            if state.add_dialog.is_visible() && state.add_dialog.active_tab == AddTab::Torrent {
                state.add_dialog.torrent_upload.set_dragging(true);
            }
        }
        Message::FilesHoveredLeft => {
            if state.add_dialog.is_visible() {
                state.add_dialog.torrent_upload.set_dragging(false);
            }
        }
        Message::FileDropped(path) => {
            if state.add_dialog.is_visible() {
                state.add_dialog.torrent_upload.set_dragging(false);
                if torrent_upload::is_valid_torrent_file(&path) {
                    state
                        .add_dialog
                        .set_torrent_path(path.to_string_lossy().to_string());
                    state.add_dialog.active_tab = AddTab::Torrent;
                } else {
                    let (_, task) = spawn_toast(
                        state,
                        ToastKind::Warning,
                        state.fluent.get(Tr::InvalidTorrent),
                        Some(Duration::from_secs(4)),
                        false,
                    );
                    return task;
                }
            }
        }
        Message::UrlEditor(action) => {
            state.add_dialog.url_editor.perform(action);
        }
        Message::PathPicker(id, event) => {
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
                None => {}
            }
        }
        Message::PathPicked(id, maybe_path) => {
            tracing::debug!(?id, picked = maybe_path.is_some(), "ui: path picked");
            if let Some(p) = maybe_path {
                apply_path(state, id, p);
            }
        }
        Message::CopyPath(s) => {
            if !s.is_empty() {
                return iced::clipboard::write::<Message>(s);
            }
        }
        Message::SplitChanged(value) => {
            if let Ok(n) = value.parse::<u16>() {
                state.add_dialog.split = n.max(1);
            }
        }
        Message::ToggleAdvanced(value) => {
            state.add_dialog.advanced_open = value;
        }
        Message::AddFieldChanged(field, value) => {
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
        }
        Message::AddDownload => {
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
                    state.add_dialog.close();
                    if nav {
                        state.page = Page::Tasks;
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
                    state.add_dialog.close();
                    if nav {
                        state.page = Page::Tasks;
                    }
                } else {
                    tracing::debug!("ui: add download skipped (no urls after filter)");
                }
            }
        }
        Message::PauseTask(gid) => {
            state.paused_gids.insert(gid.clone());
            if state.handle.cmd_tx.send(EngineCmd::Pause(gid)).is_err() {
                tracing::warn!("ui: pause cmd send failed");
            }
        }
        Message::ResumeTask(gid) => {
            state.paused_gids.remove(&gid);
            if state.handle.cmd_tx.send(EngineCmd::Resume(gid)).is_err() {
                tracing::warn!("ui: resume cmd send failed");
            }
        }
        Message::RemoveTask(gid) => {
            state.paused_gids.remove(&gid);
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
            state.confirm = None;
        }
        Message::DeleteTask(gid) => {
            state.paused_gids.remove(&gid);
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
            state.confirm = None;
        }
        Message::StartAll => {
            state.paused_gids.clear();
            if state.handle.cmd_tx.send(EngineCmd::ResumeAll).is_err() {
                tracing::warn!("ui: resume all cmd send failed");
            }
        }
        Message::PauseAll => {
            state.paused_gids.extend(
                state
                    .tasks
                    .values()
                    .filter(|t| t.status == TaskStatus::Active)
                    .map(|t| t.gid.clone()),
            );
            if state.handle.cmd_tx.send(EngineCmd::PauseAll).is_err() {
                tracing::warn!("ui: pause all cmd send failed");
            }
        }
        Message::DeleteAll => {
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::RemoveAll { delete_files: true })
                .is_err()
            {
                tracing::warn!("ui: remove all cmd send failed");
            }
            clear_all_local(state);
            state.confirm = None;
        }
        Message::RemoveAllRecords => {
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
            state.confirm = None;
        }
        Message::ClearCompleted => {
            let completed: Vec<String> = state
                .tasks
                .iter()
                .filter(|(_, t)| matches!(t.status, TaskStatus::Completed | TaskStatus::Removed))
                .map(|(gid, _)| gid.clone())
                .collect();
            clear_completed_local(state, &completed);
            state.confirm = None;
        }
        Message::Refresh => {
            if state.handle.cmd_tx.send(EngineCmd::Snapshot).is_err() {
                tracing::warn!("ui: snapshot cmd send failed");
            }
        }
        Message::SortSelected(field) => {
            state.sort_field = field;
        }
        Message::ToggleSortMenu => {
            state.sort_menu_open = !state.sort_menu_open;
        }
        Message::CloseSortMenu => {
            state.sort_menu_open = false;
        }
        Message::ToggleSortOrder => {
            state.sort_order = match state.sort_order {
                SortOrder::Asc => SortOrder::Desc,
                SortOrder::Desc => SortOrder::Asc,
            };
        }
        Message::SearchChanged(query) => {
            state.search_query = query;
        }
        Message::OpenAbout => {
            state.about_dialog_visible = true;
        }
        Message::CloseAbout => {
            state.about_dialog_visible = false;
        }
        Message::SettingChanged(key, value) => match key {
            SettingKey::MaxConcurrent => {
                if let Ok(n) = value.parse::<u32>() {
                    state.settings.max_concurrent = n.max(1);
                }
            }
            SettingKey::Split => {
                if let Ok(n) = value.parse::<u16>() {
                    state.settings.split = n.max(1);
                }
            }
            SettingKey::DownloadLimit => {
                state.settings.download_limit_kb = value.parse().unwrap_or(0);
            }
            SettingKey::UploadLimit => {
                state.settings.upload_limit_kb = value.parse().unwrap_or(0);
            }
            SettingKey::ThemeMode => {
                state.settings.theme_mode = match value.as_str() {
                    "dark" => ThemeMode::Dark,
                    "light" => ThemeMode::Light,
                    _ => ThemeMode::System,
                };
                rebuild_theme(state);
            }
            SettingKey::Locale => {
                state.settings.locale = match value.as_str() {
                    "zh-CN" => Locale::ZhCN,
                    _ => Locale::EnUS,
                };
                state.fluent = Fluent::new(state.settings.locale);
            }
            SettingKey::MaxConnectionPerServer => {
                if let Ok(n) = value.parse::<u32>() {
                    state.settings.aria2.max_connection_per_server = n.max(1);
                }
            }
            SettingKey::MinSplitSize => {
                if let Ok(n) = value.parse::<u64>() {
                    state.settings.aria2.min_split_size_mb = n;
                }
            }
            SettingKey::AutoFileRenaming => {
                state.settings.aria2.auto_file_renaming = value == "true";
            }
            SettingKey::AllowOverwrite => {
                state.settings.aria2.allow_overwrite = value == "true";
            }
            SettingKey::Continue => {
                state.settings.aria2.r#continue = value == "true";
            }
            SettingKey::CheckIntegrity => {
                state.settings.aria2.check_integrity = value == "true";
            }
            SettingKey::MaxDownloadLimit => {
                state.settings.aria2.max_download_limit_kb = value.parse().unwrap_or(0);
            }
            SettingKey::MaxUploadLimit => {
                state.settings.aria2.max_upload_limit_kb = value.parse().unwrap_or(0);
            }
            SettingKey::LowestSpeedLimit => {
                state.settings.aria2.lowest_speed_limit_kb = value.parse().unwrap_or(0);
            }
            SettingKey::UserAgent => {
                state.settings.aria2.user_agent = value;
            }
            SettingKey::ProxyServer => {
                state.settings.aria2.proxy_server = value;
            }
            SettingKey::ProxyUsername => {
                state.settings.aria2.proxy_username = value;
            }
            SettingKey::ProxyPassword => {
                state.settings.aria2.proxy_password = value;
            }
            SettingKey::MaxTries => {
                if let Ok(n) = value.parse::<u32>() {
                    state.settings.aria2.max_tries = n;
                }
            }
            SettingKey::RetryWait => {
                if let Ok(n) = value.parse::<u32>() {
                    state.settings.aria2.retry_wait = n;
                }
            }
            SettingKey::ConnectTimeout => {
                if let Ok(n) = value.parse::<u32>() {
                    state.settings.aria2.connect_timeout = n;
                }
            }
            SettingKey::BtTracker => {
                state.settings.aria2.bt_tracker = value;
            }
            SettingKey::SeedRatio => {
                if let Ok(n) = value.parse::<f64>() {
                    state.settings.aria2.seed_ratio = n.max(0.0);
                }
            }
            SettingKey::SeedTime => {
                if let Ok(n) = value.parse::<u32>() {
                    state.settings.aria2.seed_time = n;
                }
            }
            SettingKey::EnableDht => {
                state.settings.aria2.enable_dht = value == "true";
            }
            SettingKey::BtRequireCrypto => {
                state.settings.aria2.bt_require_crypto = value == "true";
            }
            SettingKey::BtEnableLpd => {
                state.settings.aria2.bt_enable_lpd = value == "true";
            }
            SettingKey::EnablePeerExchange => {
                state.settings.aria2.enable_peer_exchange = value == "true";
            }
            SettingKey::BtAutoDownload => {
                state.settings.aria2.bt_auto_download = value == "true";
            }
            SettingKey::FileAllocation => {
                state.settings.aria2.file_allocation = value;
            }
            SettingKey::DiskCache => {
                if let Ok(n) = value.parse::<u64>() {
                    state.settings.aria2.disk_cache_mb = n;
                }
            }
            SettingKey::EnableProxy => {
                state.settings.aria2.proxy_enabled = value == "true";
            }
            SettingKey::NavToTasksAfterAdd => {
                state.settings.nav_to_tasks_after_add = value == "true";
            }
            SettingKey::DeleteTorrentAfterComplete => {
                state.settings.delete_torrent_after_complete = value == "true";
            }
            SettingKey::CleanupCompletedOnClose => {
                state.settings.cleanup_completed_on_close = value == "true";
            }
            SettingKey::RemoveTaskIfFilesMissing => {
                state.settings.remove_task_if_files_missing = value == "true";
            }
            SettingKey::DetectClipboardOnStart => {
                state.settings.detect_clipboard_on_start = value == "true";
            }
            SettingKey::ClipboardHttp => {
                state.settings.clipboard_types.http = value == "true";
            }
            SettingKey::ClipboardFtp => {
                state.settings.clipboard_types.ftp = value == "true";
            }
            SettingKey::ClipboardMagnet => {
                state.settings.clipboard_types.magnet = value == "true";
            }
            SettingKey::ClipboardEd2k => {
                state.settings.clipboard_types.ed2k = value == "true";
            }
            SettingKey::ClipboardThunder => {
                state.settings.clipboard_types.thunder = value == "true";
            }
            SettingKey::ClipboardBtInfohash => {
                state.settings.clipboard_types.bt_infohash = value == "true";
            }
            SettingKey::Ed2kServer => {
                state.settings.aria2.ed2k_server = value;
            }
            SettingKey::Ed2kListenPort => {
                if let Ok(n) = value.parse::<u16>() {
                    state.settings.aria2.ed2k_listen_port = n;
                }
            }
            SettingKey::Ed2kUdpListenPort => {
                if let Ok(n) = value.parse::<u16>() {
                    state.settings.aria2.ed2k_udp_listen_port = n;
                }
            }
            SettingKey::Ed2kUploadSlots => {
                if let Ok(n) = value.parse::<u16>() {
                    state.settings.aria2.ed2k_upload_slots = n.max(1);
                }
            }
        },
        Message::ApplySettings => {
            config::save(&state.settings);
            let opts = state.settings.to_aria2_task_options();
            tracing::info!("ui: apply settings");
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::ApplyAria2Options { options: opts })
                .is_err()
            {
                tracing::warn!("ui: apply aria2 options cmd send failed");
            }
            if !state
                .settings
                .aria2
                .ed2k_equal(&state.applied_settings.aria2)
                && state.handle.cmd_tx.send(EngineCmd::RestartEngine).is_err()
            {
                tracing::warn!("ui: restart engine cmd send failed");
            }
            state.applied_settings = state.settings.clone();
        }
        Message::ResetSettings => {
            revert_apply_settings(state);
            config::save(&state.settings);
        }
        Message::Engine(event) => match event {
            EngineEvent::EngineReady => {
                tracing::info!("engine ready");
                state.aria2_fetch_error = None;
                state.synced_gids.clear();
                state.sync_done = false;
                if let Some(id) = state.downloading_toast_id.take() {
                    dismiss_toast(state, id);
                }
                if let Some(id) = state.startup_error_toast_id.take() {
                    dismiss_toast(state, id);
                }
                state.startup_starting_toast_shown = false;
                state.aria2_status = Some(("ready".to_string(), state.fluent.get(Tr::Aria2Ready)));
                let (_, task) = spawn_toast(
                    state,
                    ToastKind::Success,
                    state.fluent.get(Tr::EngineStarted),
                    Some(Duration::from_secs(3)),
                    false,
                );
                return task;
            }
            EngineEvent::EngineStopped => {
                tracing::info!("engine stopped");
                state.global_speed = None;
                state.paused_gids.clear();
                if state.closing {
                    return finalize_close(state);
                }
            }
            EngineEvent::SyncComplete => {
                tracing::info!("engine sync complete");
                if state.sync_done {
                    return Task::none();
                }
                state.sync_done = true;
                for (gid, t) in state.tasks.iter() {
                    if t.status == TaskStatus::Completed
                        && !t.url.is_empty()
                        && crate::engine::is_torrent_url(&t.url)
                    {
                        state.torrent_followed.insert(gid.clone());
                    }
                }
                let purge: Vec<String> = state
                    .tasks
                    .iter()
                    .filter(|(gid, t)| {
                        !state.synced_gids.contains(*gid)
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
                    remove_task_local(state, gid);
                    tracing::info!(?gid, "ui: purged non-terminal ghost task");
                }
                let split = state.settings.split;
                let ghost: Vec<(String, String, PathBuf, bool)> = state
                    .tasks
                    .iter()
                    .filter(|(gid, t)| {
                        !state.synced_gids.contains(*gid)
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
                        )
                    })
                    .collect();
                for (gid, url, save_dir, paused) in ghost {
                    if paused {
                        state.paused_gids.insert(gid.clone());
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
            }
            EngineEvent::FilesMissing { gids } => {
                let removed: Vec<String> = gids
                    .iter()
                    .filter(|g| state.tasks.contains_key(*g))
                    .cloned()
                    .collect();
                for gid in &removed {
                    tracing::info!(?gid, "ui: removed task with missing files");
                    remove_task_local(state, gid);
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
                    let (_, task) = spawn_toast(
                        state,
                        ToastKind::Normal,
                        state.fluent.get(Tr::FilesMissingRemoved),
                        Some(Duration::from_secs(3)),
                        false,
                    );
                    return task;
                }
            }
            EngineEvent::Added {
                gid,
                name,
                url,
                dir,
                info_hash,
            } => {
                tracing::info!(?gid, ?name, "ui: task added");
                state.synced_gids.insert(gid.clone());
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                if let Some(existing) = state.tasks.get_mut(&gid) {
                    existing.name = name;
                    existing.url = url;
                    existing.save_dir = PathBuf::from(dir);
                    if info_hash.is_some() {
                        existing.info_hash = info_hash;
                    }
                    state.dirty.insert(gid.clone());
                } else {
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
                    };
                    state.tasks.insert(gid.clone(), task);
                    state.task_order.insert(0, gid.clone());
                    if let Some(ref db) = state.db {
                        db.upsert_meta(
                            &gid,
                            &state.tasks[&gid].name,
                            &state.tasks[&gid].url,
                            &state.tasks[&gid].save_dir.to_string_lossy(),
                            "waiting",
                            now,
                            &state.tasks[&gid].info_hash.clone().unwrap_or_default(),
                        );
                    }
                }
                state.dirty.insert(gid);
            }
            EngineEvent::TorrentAdded { gid, path } => {
                state.torrent_files.insert(gid, path);
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
            } => {
                state.synced_gids.insert(gid.clone());
                if status == "complete"
                    && state.settings.delete_torrent_after_complete
                    && state.torrent_files.contains_key(&gid)
                {
                    if let Some(path) = state.torrent_files.remove(&gid) {
                        let _ = std::fs::remove_file(&path);
                    }
                }
                if !state.tasks.contains_key(&gid)
                    && !matches!(status.as_str(), "complete" | "error" | "removed")
                {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let task_status = TaskStatus::from_engine(&status);
                    if task_status == TaskStatus::Active {
                        state.active_count += 1;
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
                        },
                    );
                    state.task_order.insert(0, gid.clone());
                    if let Some(ref db) = state.db {
                        db.upsert_meta(
                            &gid,
                            &name,
                            "",
                            "",
                            &status,
                            now,
                            info_hash.as_deref().unwrap_or_default(),
                        );
                    }
                }
                if let Some(t) = state.tasks.get_mut(&gid) {
                    let was_active = t.status == TaskStatus::Active;
                    t.name = name;
                    if info_hash.is_some() {
                        t.info_hash = info_hash;
                    }
                    if total == 0 && t.total > 0 {
                        t.status = TaskStatus::from_engine(&status);
                        t.speed = speed;
                        t.upload_speed = upload_speed;
                        t.connections = connections;
                    } else {
                        t.downloaded = downloaded;
                        t.total = total;
                        t.speed = speed;
                        t.upload_speed = upload_speed;
                        t.status = TaskStatus::from_engine(&status);
                        t.connections = connections;
                    }
                    if state.paused_gids.contains(&gid) {
                        t.status = TaskStatus::Paused;
                    }
                    if t.status == TaskStatus::Paused {
                        t.speed = 0;
                        t.upload_speed = 0;
                    }
                    if was_active != (t.status == TaskStatus::Active) {
                        if t.status == TaskStatus::Active {
                            state.active_count += 1;
                        } else {
                            state.active_count = state.active_count.saturating_sub(1);
                        }
                    }
                    state.dirty.insert(gid.clone());
                }
                if status == "complete" && state.sync_done {
                    if let Some(t) = state.tasks.get(&gid) {
                        if !t.url.is_empty()
                            && crate::engine::is_torrent_url(&t.url)
                            && state.settings.aria2.bt_auto_download
                        {
                            if state.torrent_followed.insert(gid.clone()) {
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
                                tracing::info!(
                                    ?gid,
                                    "ui: auto-adding downloaded torrent as new task"
                                );
                            } else {
                                state.torrent_followed.remove(&gid);
                            }
                        }
                    }
                }
            }
            EngineEvent::Removed(gid) => {
                tracing::info!(?gid, "ui: task removed");
                remove_task_local(state, &gid);
            }
            EngineEvent::TaskDetails { gid, details } => {
                tracing::debug!(?gid, "task details received");
                if state.details.gid.as_deref() == Some(&gid) {
                    let first_load = state.details.loading;
                    state.details.details = Some(details);
                    state.details.loading = false;
                    let save_dir = state.tasks.get(&gid).map(|t| t.save_dir.clone());
                    let tree =
                        details_files_tree(state.details.details.as_ref(), save_dir.as_deref());
                    state.details.files_tree = tree;
                    if first_load || state.details.files_tree.is_empty() {
                        state.details.files_expanded.clear();
                        crate::ui::components::file_tree::collect_dir_paths(
                            &state.details.files_tree,
                            &mut state.details.files_expanded,
                        );
                    }
                }
            }
            EngineEvent::TaskDetailsFailed { gid } => {
                tracing::debug!(?gid, "task details failed");
                if state.details.gid.as_deref() == Some(&gid) {
                    state.details.loading = false;
                }
            }
            EngineEvent::SelectFilesFailed { gid } => {
                tracing::warn!(?gid, "change file selection failed");
                let (_, task) = spawn_toast(
                    state,
                    ToastKind::Warning,
                    state.fluent.get(crate::i18n::Tr::SelectFilesFailed),
                    Some(Duration::from_secs(4)),
                    false,
                );
                return task;
            }
            EngineEvent::Aria2Version { version } => {
                tracing::info!(?version, "aria2 version received");
                state.aria2_version = Some(version.clone());
                state.aria2_check_msg = None;
                if state.settings.update.should_auto_check("aria2-next")
                    && state
                        .handle
                        .cmd_tx
                        .send(EngineCmd::CheckAria2Update)
                        .is_err()
                {
                    tracing::warn!("auto-check update cmd send failed");
                }
            }
            EngineEvent::Aria2CheckResult { current, latest: _ } => {
                state.aria2_check_msg = Some(format!(
                    "{} v{current}",
                    state.fluent.get(crate::i18n::Tr::UpToDate)
                ));
            }
            EngineEvent::Aria2UpdateApplied { version } => {
                state.update_pending = None;
                state.aria2_version = Some(version.clone());
                state.aria2_check_msg = Some(format!(
                    "{} v{version}",
                    state.fluent.get(crate::i18n::Tr::UpdatedTo)
                ));
            }
            EngineEvent::Aria2UpdateFailed { error } => {
                state.aria2_check_msg = Some(error);
            }
            EngineEvent::Aria2UpdateStaged { version } => {
                state.update_pending = Some(version);
                state.aria2_check_msg = None;
            }
            EngineEvent::Aria2FetchFailed { error } => {
                let msg = format!("{}: {error}", state.fluent.get(Tr::EngineStartFailed));
                state.aria2_fetch_error = Some(error);
                state.startup_starting_toast_shown = false;
                if let Some(id) = state.downloading_toast_id.take() {
                    dismiss_toast(state, id);
                }
                if let Some(id) = state.startup_error_toast_id.take() {
                    dismiss_toast(state, id);
                }
                let (id, task) = spawn_toast(state, ToastKind::Error, msg, None, true);
                state.startup_error_toast_id = Some(id);
                return task;
            }
            EngineEvent::EngineDegraded { reason } => {
                state.aria2_fetch_error = Some(reason);
            }
            EngineEvent::GlobalSpeed { download, upload } => {
                state.global_speed = Some((download, upload));
            }
            EngineEvent::Aria2Status { stage, message } => {
                if stage == "ready" {
                    state.aria2_fetch_error = None;
                }
                if stage == "ready" || stage == "starting" {
                    if let Some(id) = state.downloading_toast_id.take() {
                        dismiss_toast(state, id);
                    }
                }
                let mut toast_task = None;
                if stage == "downloading" && state.downloading_toast_id.is_none() {
                    let (id, task) = spawn_toast(
                        state,
                        ToastKind::Normal,
                        state.fluent.get(Tr::DownloadingAria2),
                        None,
                        false,
                    );
                    state.downloading_toast_id = Some(id);
                    toast_task = Some(task);
                }
                if stage == "starting" && !state.startup_starting_toast_shown {
                    state.startup_starting_toast_shown = true;
                    let (_, task) = spawn_toast(
                        state,
                        ToastKind::Normal,
                        state.fluent.get(Tr::EngineStarting),
                        Some(Duration::from_secs(3)),
                        false,
                    );
                    toast_task = Some(task);
                }
                state.aria2_status = Some((stage, message));
                if let Some(task) = toast_task {
                    return task;
                }
            }
        },
        Message::WindowResized(size) => {
            state.last_resize = Some(size);
            state.geometry_dirty = true;
        }
        Message::WindowOpened(id) => {
            if state.window_id.is_none() {
                state.window_id = Some(id);
                return read_clipboard(state);
            }
            return Task::none();
        }
        Message::WindowFocused(id) => {
            if state.window_id.is_none() || state.window_id == Some(id) {
                return read_clipboard(state);
            }
            return Task::none();
        }
        Message::ClipboardRead(content) => {
            let Some(text) = content else {
                return Task::none();
            };
            let trimmed = text.trim().to_string();
            let prefs = state.settings.clipboard_types;
            let hash = hex::encode(Sha256::digest(
                format!(
                    "{trimmed}|{}{}{}{}{}{}",
                    prefs.http,
                    prefs.ftp,
                    prefs.magnet,
                    prefs.ed2k,
                    prefs.thunder,
                    prefs.bt_infohash
                )
                .as_bytes(),
            ));
            return Task::perform(
                async move {
                    let payload = crate::clipboard_watch::parse_clipboard(&trimmed, prefs);
                    (payload, hash)
                },
                |(payload, hash)| Message::ClipboardParsed(payload, hash),
            );
        }
        Message::ClipboardParsed(payload, hash) => {
            let Some(payload) = payload else {
                return Task::none();
            };
            if state.add_dialog.is_visible() {
                return Task::none();
            }
            if hash == state.settings.last_clipboard_hash {
                return Task::none();
            }
            state.settings.last_clipboard_hash = hash;
            config::save(&state.settings);
            state.add_dialog.open_with(
                state.settings.download_dir.clone(),
                state.settings.split,
                payload,
            );
            let (_, task) = spawn_toast(
                state,
                ToastKind::Normal,
                state.fluent.get(Tr::ClipboardDetected),
                Some(Duration::from_secs(3)),
                false,
            );
            return task;
        }
        Message::DragWindow => {
            if let Some(id) = state.window_id {
                return iced::window::drag::<Message>(id);
            }
        }
        Message::ResizeWindow(direction) => {
            if let Some(id) = state.window_id {
                return iced::window::drag_resize::<Message>(id, direction);
            }
        }
        Message::WindowAction(cmd) => {
            if let Some(id) = state.window_id {
                return match cmd {
                    WindowCmd::Minimize => iced::window::minimize::<Message>(id, true),
                    WindowCmd::ToggleMaximize => {
                        state.maximized = !state.maximized;
                        iced::window::toggle_maximize::<Message>(id)
                    }
                };
            }
        }
        Message::CloseRequested => {
            if state.closing {
                return Task::none();
            }
            state.show_close_dialog = true;
        }
        Message::CloseDialog(choice) => {
            state.show_close_dialog = false;
            return match choice {
                CloseDialogChoice::Close => begin_close(state),
                CloseDialogChoice::Cancel => Task::none(),
                CloseDialogChoice::MinimizeToTray => Task::none(),
            };
        }
        Message::ShutdownRequested => {
            return begin_close(state);
        }
        Message::ShutdownTimeout => {
            if state.closing {
                tracing::warn!("engine did not stop in time, closing anyway");
            }
            return finalize_close(state);
        }
        Message::PersistWindowGeometry => {
            if state.geometry_dirty {
                if let Some(id) = state.window_id {
                    return iced::window::is_maximized(id)
                        .then(|max| Task::done(Message::WindowMaximized(max)));
                }
            }
        }
        Message::WindowMaximized(max) => {
            state.maximized = max;
            if let Some(s) = state.last_resize {
                if !max {
                    state.window_size = s;
                }
                state.last_resize = None;
            }
            sync_geometry_to_settings(state);
            config::save(&state.settings);
            state.geometry_dirty = false;
            if state.pending_close {
                state.pending_close = false;
                if let Some(id) = state.window_id {
                    return iced::window::close::<Message>(id);
                }
            }
        }
        Message::ThemeModeChanged(mode) => {
            state.settings.theme_mode = mode;
            rebuild_theme(state);
            config::save(&state.settings);
        }
        Message::ThemeColorChanged(color) => {
            state.settings.theme_color = theme::color_to_hex(color);
            rebuild_theme(state);
            config::save(&state.settings);
        }
        Message::LocaleChanged(locale) => {
            state.settings.locale = locale;
            state.fluent = Fluent::new(locale);
            config::save(&state.settings);
        }
        Message::SpeedUnitChanged(key, unit) => {
            state.settings_ui.speed_units.insert(key, unit);
        }
        Message::UaEditor(action) => {
            state.ua_editor.perform(action);
            state.settings.aria2.user_agent = state.ua_editor.text();
        }
        Message::CheckAria2Update => {
            state.aria2_check_msg = None;
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::CheckAria2Update)
                .is_err()
            {
                tracing::warn!("check update cmd send failed");
            }
        }
        Message::RetryAria2Fetch => {
            state.aria2_fetch_error = None;
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::RetryAria2Fetch)
                .is_err()
            {
                tracing::warn!("retry fetch cmd send failed");
            }
        }
        Message::RestartEngine => {
            if state.handle.cmd_tx.send(EngineCmd::RestartEngine).is_err() {
                tracing::warn!("restart engine cmd send failed");
            }
        }
        Message::SetAutoCheck(enabled) => {
            state.settings.update.set_ignored("aria2-next", !enabled);
            config::save(&state.settings);
        }
        Message::CheckMissingFiles => {
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::CheckMissingFiles)
                .is_err()
            {
                tracing::warn!("check missing files cmd send failed");
            }
        }
        Message::OpenTaskDetails(gid) => {
            state.details_select_gen = 0;
            state.details_pending_select = None;
            state.details.open(gid.clone());
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::FetchTaskDetails(gid))
                .is_err()
            {
                tracing::warn!("fetch task details cmd send failed");
            }
        }
        Message::CloseTaskDetails => {
            state.details_select_gen += 1;
            if let Some((gid, files)) = state.details_pending_select.take() {
                let _ = state
                    .handle
                    .cmd_tx
                    .send(EngineCmd::SelectFiles { gid, files });
            }
            state.details.close();
        }
        Message::RefreshTaskDetails => {
            if state.details.is_visible() {
                if let Some(ref gid) = state.details.gid {
                    if state
                        .handle
                        .cmd_tx
                        .send(EngineCmd::FetchTaskDetails(gid.clone()))
                        .is_err()
                    {
                        tracing::warn!("refresh task details cmd send failed");
                    }
                }
            }
        }
        Message::FlushDirty => {
            flush_dirty(state);
        }
        Message::SelectDetailsTab(tab) => {
            state.details.active_tab = tab;
        }
        Message::DetailsTreeExpand(path) => {
            if state.details.files_expanded.contains(&path) {
                state.details.files_expanded.remove(&path);
            } else {
                state.details.files_expanded.insert(path);
            }
        }
        Message::DetailsTreeToggle(path) => {
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
            return schedule_details_select_flush(state);
        }
        Message::DetailsFilesSelectAll => {
            if let Some(ref mut details) = state.details.details {
                for file in &mut details.files {
                    file.selected = true;
                }
            }
            return schedule_details_select_flush(state);
        }
        Message::DetailsFilesSelectNone => {
            if let Some(ref mut details) = state.details.details {
                for file in &mut details.files {
                    file.selected = false;
                }
                if let Some(first) = details.files.iter_mut().min_by_key(|f| f.index) {
                    first.selected = true;
                }
            }
            return schedule_details_select_flush(state);
        }
        Message::DetailsFilesScroll(off) => {
            state.details.files_scroll_offset = off;
        }
        Message::DetailsFilesFlush(gen) => {
            if gen != state.details_select_gen {
                return Task::none();
            }
            if let Some((gid, files)) = state.details_pending_select.take() {
                let _ = state.handle.cmd_tx.send(EngineCmd::SelectFiles {
                    gid: gid.clone(),
                    files,
                });
                let _ = state.handle.cmd_tx.send(EngineCmd::FetchTaskDetails(gid));
            }
        }
        Message::OpenTaskFile(gid) => {
            let Some(t) = state.tasks.get(&gid).cloned() else {
                return Task::none();
            };
            let has_hash = t
                .info_hash
                .as_deref()
                .map(|h| !h.is_empty())
                .unwrap_or(false);
            let is_bt = has_hash
                || crate::engine::is_magnet_url(&t.url)
                || crate::engine::is_torrent_url(&t.url);
            if is_bt {
                state.add_dialog.save_picker.close_history();
                state
                    .add_dialog
                    .open(state.settings.download_dir.clone(), state.settings.split);
                let link = if !t.url.is_empty() {
                    t.url.clone()
                } else {
                    format!(
                        "magnet:?xt=urn:btih:{}",
                        t.info_hash.as_deref().unwrap_or_default()
                    )
                };
                state.add_dialog.set_urls(vec![link]);
                return Task::none();
            }
            let path = t.save_dir.join(&t.name);
            if path.exists() {
                return Task::perform(
                    async move {
                        let _ = open::that(&path);
                    },
                    |_| Message::Noop,
                );
            }
            let (_, task) = spawn_toast(
                state,
                ToastKind::Warning,
                state.fluent.get(Tr::FileMissing),
                Some(Duration::from_secs(4)),
                false,
            );
            return task;
        }
        Message::OpenTaskFolder(gid) => {
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
        }
        Message::CopyTaskLink(gid) => {
            let Some(t) = state.tasks.get(&gid) else {
                return Task::none();
            };
            if !t.url.is_empty() {
                return iced::clipboard::write::<Message>(t.url.clone());
            }
            if let Some(hash) = t.info_hash.as_deref() {
                if !hash.is_empty() {
                    return iced::clipboard::write::<Message>(format!(
                        "magnet:?xt=urn:btih:{hash}"
                    ));
                }
            }
        }
        Message::RequestConfirm(action) => {
            state.confirm = Some(action);
        }
        Message::ConfirmCancel => {
            state.confirm = None;
        }
        Message::ApplyAndLeaveSettings => {
            if let Some(ConfirmAction::LeaveSettings { target }) = state.confirm.take() {
                config::save(&state.settings);
                let opts = state.settings.to_aria2_task_options();
                let _ = state
                    .handle
                    .cmd_tx
                    .send(EngineCmd::ApplyAria2Options { options: opts });
                if !state
                    .settings
                    .aria2
                    .ed2k_equal(&state.applied_settings.aria2)
                {
                    let _ = state.handle.cmd_tx.send(EngineCmd::RestartEngine);
                }
                state.applied_settings = state.settings.clone();
                state.page = target;
            }
        }
        Message::DiscardAndLeaveSettings => {
            if let Some(ConfirmAction::LeaveSettings { target }) = state.confirm.take() {
                revert_apply_settings(state);
                config::save(&state.settings);
                state.page = target;
            }
        }
        Message::ShowToast(mut toast) => {
            toast.id = state.next_toast_id;
            state.next_toast_id += 1;
            push_toast(state, toast);
        }
        Message::DismissToast(id) => {
            dismiss_toast(state, id);
        }
        Message::ToastHovered(id) => {
            state.hovered_toast_id = Some(id);
        }
        Message::ToastUnhovered(id) => {
            if state.hovered_toast_id == Some(id) {
                state.hovered_toast_id = None;
            }
        }
        Message::ToastTick => {
            const TICK: Duration = Duration::from_millis(200);
            let mut expired = Vec::new();
            for toast in state.toasts.iter_mut() {
                if let Some(rem) = toast.remaining.as_mut() {
                    if Some(toast.id) != state.hovered_toast_id {
                        if *rem <= TICK {
                            *rem = Duration::ZERO;
                            expired.push(toast.id);
                        } else {
                            *rem -= TICK;
                        }
                    }
                }
            }
            for id in expired {
                dismiss_toast(state, id);
            }
        }
        Message::Noop => {}
    }
    Task::none()
}

pub fn view(state: &Remotrix) -> Element<'_, Message> {
    let counts = Counts {
        all: state.tasks.len(),
        downloading: state
            .tasks
            .values()
            .filter(|t| {
                matches!(
                    t.status,
                    TaskStatus::Active | TaskStatus::Waiting | TaskStatus::Paused
                )
            })
            .count(),
        completed: state
            .tasks
            .values()
            .filter(|t| matches!(t.status, TaskStatus::Completed))
            .count(),
    };

    let t = &state.theme;
    let titlebar = crate::ui::title_bar::view(t, state.maximized);
    let left_col = crate::ui::sidebar::view(&state.fluent, t, state.page, &state.logo_handle);

    let mid_col = crate::ui::category_bar::view(
        &state.fluent,
        t,
        state.page,
        state.task_filter,
        state.settings_cat,
        &counts,
    );

    let right_col: Element<'_, Message> = match state.page {
        Page::Tasks => {
            let query = state.search_query.trim().to_lowercase();
            let filtered: Vec<DownloadTask> = state
                .task_order
                .iter()
                .filter_map(|gid| state.tasks.get(gid))
                .filter(|t| match state.task_filter {
                    TaskFilter::All => true,
                    TaskFilter::Downloading => matches!(
                        t.status,
                        TaskStatus::Active | TaskStatus::Waiting | TaskStatus::Paused
                    ),
                    TaskFilter::Completed => matches!(t.status, TaskStatus::Completed),
                })
                .filter(|t| {
                    query.is_empty()
                        || t.name.to_lowercase().contains(&query)
                        || t.url.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();
            let sorted = crate::ui::sort::sort_tasks(&filtered, state.sort_field, state.sort_order);
            crate::ui::task_list::view(
                &state.fluent,
                t,
                &sorted,
                state.sort_field,
                state.sort_order,
                state.sort_menu_open,
                &state.search_query,
            )
        }
        Page::Settings => crate::ui::settings_page::view(
            &state.fluent,
            t,
            &state.settings,
            &state.settings_ui,
            state.settings_cat,
            &state.applied_settings,
            state.aria2_version.as_deref(),
            state.aria2_check_msg.as_deref(),
            state
                .aria2_status
                .as_ref()
                .map(|(s, m)| (s.as_str(), m.as_str())),
            state.aria2_fetch_error.as_deref(),
            state.update_pending.as_deref(),
            &state.ua_editor,
            &state.settings.path_history,
        ),
    };

    let content = row![]
        .push(
            container(left_col)
                .width(Length::Fixed(SIDEBAR_W))
                .height(Length::Fill),
        )
        .push(
            container(mid_col)
                .width(Length::Fixed(CATEGORY_W))
                .height(Length::Fill),
        )
        .push(
            container(right_col)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill);

    let body = column![]
        .push(titlebar)
        .push(content)
        .width(Length::Fill)
        .height(Length::Fill);

    let base = container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::style::base_background);

    let framed: iced::Element<'_, Message> = if state.maximized {
        #[allow(clippy::useless_conversion)]
        {
            iced::widget::opaque(base).into()
        }
    } else {
        stack![iced::widget::opaque(base), crate::ui::resize_frame::view(),]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    };
    let (dl, up) = if state.active_count > 0 {
        state.global_speed.unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    let hud_overlay = container(crate::ui::components::speed_hud::view(
        t,
        state.active_count > 0,
        dl,
        up,
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Horizontal::Right)
    .align_y(Vertical::Bottom)
    .padding(Padding {
        top: 0.0,
        right: 16.0,
        bottom: 20.0,
        left: 0.0,
    });
    let base_layer: iced::Element<'_, Message> = stack![framed, hud_overlay]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

    let add_layer: iced::Element<'_, Message> = if state.add_dialog.is_visible() {
        crate::ui::add_dialog::view(
            &state.fluent,
            t,
            &state.add_dialog,
            &state.settings.path_history,
        )
    } else {
        iced::widget::Space::new().into()
    };

    let about_layer: iced::Element<'_, Message> = if state.about_dialog_visible {
        crate::ui::about_dialog::view(&state.fluent, t, state.aria2_version.as_deref())
    } else {
        iced::widget::Space::new().into()
    };

    let close_layer: iced::Element<'_, Message> = if state.show_close_dialog {
        crate::ui::close_dialog::view(&state.fluent, t)
    } else {
        iced::widget::Space::new().into()
    };

    let details_layer: iced::Element<'_, Message> = if state.details.is_visible() {
        let task = state
            .details
            .gid
            .as_deref()
            .and_then(|g| state.tasks.get(g));
        crate::ui::details_dialog::view(&state.fluent, t, task, &state.details)
    } else {
        iced::widget::Space::new().into()
    };

    let confirm_layer: iced::Element<'_, Message> = if let Some(action) = &state.confirm {
        crate::ui::confirm_dialog::view(&state.fluent, t, action)
    } else {
        iced::widget::Space::new().into()
    };

    let toast_layer: iced::Element<'_, Message> = if !state.toasts.is_empty() {
        crate::ui::components::toast::view(t, &state.toasts)
    } else {
        iced::widget::Space::new().into()
    };

    let stacked: iced::Element<'_, Message> = stack![
        base_layer,
        add_layer,
        about_layer,
        close_layer,
        details_layer,
        confirm_layer,
        toast_layer,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

    container(stacked)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

struct EventSlot(Arc<Mutex<Option<EventRx>>>);

impl Hash for EventSlot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl PartialEq for EventSlot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for EventSlot {}

impl Clone for EventSlot {
    fn clone(&self) -> Self {
        EventSlot(self.0.clone())
    }
}

fn build_engine_stream(slot: &EventSlot) -> impl iced::futures::Stream<Item = Message> {
    let rx = {
        let mut guard = slot.0.lock().expect("event rx slot poisoned");
        guard.take()
    };
    iced::stream::channel(
        64,
        move |mut sender: iced::futures::channel::mpsc::Sender<Message>| async move {
            if let Some(mut rx) = rx {
                while let Some(ev) = rx.recv().await {
                    let _ = sender.send(Message::Engine(ev)).await;
                }
            }
        },
    )
}

fn signal_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(
        4,
        |mut sender: iced::futures::channel::mpsc::Sender<Message>| async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                let mut term = signal(SignalKind::terminate()).ok();
                let mut int = signal(SignalKind::interrupt()).ok();
                tokio::select! {
                    _ = async {
                        if let Some(ref mut t) = term {
                            let _ = t.recv().await;
                        }
                    }, if term.is_some() => {}
                    _ = async {
                        if let Some(ref mut i) = int {
                            let _ = i.recv().await;
                        }
                    }, if int.is_some() => {}
                    else => {
                        std::future::pending::<()>().await;
                    }
                }
                tracing::info!("termination signal received");
            }
            #[cfg(not(unix))]
            {
                std::future::pending::<()>().await;
            }
            let _ = sender.send(Message::ShutdownRequested).await;
        },
    )
}

pub fn subscription(state: &Remotrix) -> Subscription<Message> {
    let engine =
        Subscription::run_with(EventSlot(state.event_rx_slot.clone()), build_engine_stream);

    let open = iced::window::open_events().map(Message::WindowOpened);
    let close = iced::window::close_requests().map(|_id| Message::CloseRequested);
    let focus = iced::event::listen_with(|event, _status, window| match event {
        iced::event::Event::Window(iced::window::Event::Focused) => {
            Some(Message::WindowFocused(window))
        }
        _ => None,
    });

    let files = iced::event::listen_with(|event, _status, _window| match event {
        iced::event::Event::Window(iced::window::Event::FileHovered(path)) => {
            Some(Message::FileHovered(path))
        }
        iced::event::Event::Window(iced::window::Event::FileDropped(path)) => {
            Some(Message::FileDropped(path))
        }
        iced::event::Event::Window(iced::window::Event::FilesHoveredLeft) => {
            Some(Message::FilesHoveredLeft)
        }
        _ => None,
    });

    let flush = iced::time::every(Duration::from_millis(1000)).map(|_| Message::FlushDirty);

    let resizes = iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size));
    let persist_periodic =
        iced::time::every(Duration::from_millis(2000)).map(|_| Message::PersistWindowGeometry);

    let refresh = if state.details.is_visible() {
        iced::time::every(Duration::from_millis(2000)).map(|_| Message::RefreshTaskDetails)
    } else {
        Subscription::none()
    };

    let toast_tick = if state.toasts.iter().any(|t| t.remaining.is_some()) {
        iced::time::every(Duration::from_millis(200)).map(|_| Message::ToastTick)
    } else {
        Subscription::none()
    };

    let signals = Subscription::run_with((), |_| signal_stream());

    let missing_check = if state.settings.remove_task_if_files_missing && state.sync_done {
        iced::time::every(Duration::from_secs(30)).map(|_| Message::CheckMissingFiles)
    } else {
        Subscription::none()
    };

    Subscription::batch(vec![
        engine,
        open,
        close,
        focus,
        files,
        flush,
        resizes,
        persist_periodic,
        refresh,
        toast_tick,
        signals,
        missing_check,
    ])
}

fn picker_mut(
    state: &mut Remotrix,
    id: PathPickerId,
) -> &mut crate::ui::components::path_picker::PathPicker {
    match id {
        PathPickerId::DownloadDir => &mut state.settings_ui.download_picker,
        PathPickerId::SaveDir => &mut state.add_dialog.save_picker,
        PathPickerId::Torrent => unreachable!("torrent upload is not a PathPicker"),
        PathPickerId::Ed2kServerList => &mut state.settings_ui.ed2k_server_list_picker,
        PathPickerId::Ed2kNodeList => &mut state.settings_ui.ed2k_node_list_picker,
    }
}

fn apply_path(state: &mut Remotrix, id: PathPickerId, p: PathBuf) {
    let s = p.to_string_lossy().to_string();
    match id {
        PathPickerId::DownloadDir => {
            state.settings.record_path(id.history_key(), &s);
            state.settings.download_dir = p.clone();
            state
                .settings_ui
                .download_picker
                .set_value(p.to_string_lossy());
            state.settings_ui.download_picker.close_history();
        }
        PathPickerId::SaveDir => {
            state.settings.record_path(id.history_key(), &s);
            state.add_dialog.save_picker.set_value(p.to_string_lossy());
            state.add_dialog.save_picker.close_history();
            config::save(&state.settings);
        }
        PathPickerId::Torrent => {
            if torrent_upload::is_valid_torrent_file(&p) {
                state
                    .add_dialog
                    .set_torrent_path(p.to_string_lossy().to_string());
                config::save(&state.settings);
            } else {
                let mut toast =
                    Toast::new(ToastKind::Warning, state.fluent.get(Tr::InvalidTorrent))
                        .position(ToastPosition::Top)
                        .close_after(Some(Duration::from_secs(4)));
                toast.id = state.next_toast_id;
                state.next_toast_id += 1;
                push_toast(state, toast);
            }
        }
        PathPickerId::Ed2kServerList => {
            state.settings.aria2.ed2k_server_list = p.to_string_lossy().into_owned();
            state
                .settings_ui
                .ed2k_server_list_picker
                .set_value(p.to_string_lossy());
            state.settings_ui.ed2k_server_list_picker.close_history();
        }
        PathPickerId::Ed2kNodeList => {
            state.settings.aria2.ed2k_node_list = p.to_string_lossy().into_owned();
            state
                .settings_ui
                .ed2k_node_list_picker
                .set_value(p.to_string_lossy());
            state.settings_ui.ed2k_node_list_picker.close_history();
        }
    }
}

fn details_files_tree(
    details: Option<&crate::task::TaskDetails>,
    save_dir: Option<&std::path::Path>,
) -> Vec<FileTreeNode> {
    let Some(details) = details else {
        return Vec::new();
    };
    let tuples: Vec<(u64, String, u64)> = details
        .files
        .iter()
        .map(|f| {
            let rel = std::path::Path::new(&f.path)
                .strip_prefix(save_dir.unwrap_or(std::path::Path::new("")))
                .map(|p| p.to_string_lossy().into_owned())
                .ok()
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| {
                    std::path::Path::new(&f.path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&f.path)
                        .to_string()
                });
            (f.index, rel, f.length)
        })
        .collect();
    crate::ui::components::file_tree::build_tree(&tuples)
}

fn selected_details_indices(state: &Remotrix) -> Vec<u64> {
    state
        .details
        .details
        .as_ref()
        .map(|d| {
            d.files
                .iter()
                .filter(|f| f.selected)
                .map(|f| f.index)
                .collect()
        })
        .unwrap_or_default()
}

fn schedule_details_select_flush(state: &mut Remotrix) -> Task<Message> {
    let Some(gid) = state.details.gid.clone() else {
        return Task::none();
    };
    let selected = selected_details_indices(state);
    if selected.is_empty() {
        return Task::none();
    }
    state.details_pending_select = Some((gid, selected));
    state.details_select_gen += 1;
    let gen = state.details_select_gen;
    Task::perform(
        async move {
            tokio::time::sleep(Duration::from_millis(350)).await;
            gen
        },
        Message::DetailsFilesFlush,
    )
}

fn push_toast(state: &mut Remotrix, toast: Toast) {
    const CAP: usize = 6;
    let pos = toast.position;
    let removed_hovered = matches!(
        state.hovered_toast_id,
        Some(h)
            if state.toasts.iter().any(|t| t.id == h && t.position == pos && t.close_after.is_some())
    );
    state
        .toasts
        .retain(|t| !(t.position == pos && t.close_after.is_some()));
    if removed_hovered {
        state.hovered_toast_id = None;
    }
    let at_pos = state.toasts.iter().filter(|t| t.position == pos).count();
    if at_pos >= CAP {
        if let Some(idx) = state.toasts.iter().position(|t| t.position == pos) {
            state.toasts.remove(idx);
        }
    }
    let mut toast = toast;
    toast.remaining = toast.close_after;
    state.toasts.push(toast);
}

fn spawn_toast(
    state: &mut Remotrix,
    kind: ToastKind,
    message: String,
    close_after: Option<Duration>,
    show_close: bool,
) -> (u64, Task<Message>) {
    let id = state.next_toast_id;
    state.next_toast_id += 1;
    let mut toast = Toast::new(kind, message)
        .position(ToastPosition::Top)
        .close_after(close_after);
    if show_close {
        toast = toast.show_close();
    }
    toast.id = id;
    push_toast(state, toast);
    (id, Task::none())
}

fn dismiss_toast(state: &mut Remotrix, id: u64) {
    state.toasts.retain(|t| t.id != id);
    if state.hovered_toast_id == Some(id) {
        state.hovered_toast_id = None;
    }
}

fn pick_path(id: PathPickerId) -> Task<Message> {
    let task = async move {
        let dialog = rfd::AsyncFileDialog::new();
        let picked = match id {
            PathPickerId::DownloadDir => dialog.set_title("Select folder").pick_folder().await,
            PathPickerId::SaveDir => dialog.set_title("Select folder").pick_folder().await,
            PathPickerId::Torrent => {
                dialog
                    .set_title("Select torrent file")
                    .add_filter("Torrent", &["torrent"])
                    .pick_file()
                    .await
            }
            PathPickerId::Ed2kServerList => {
                dialog
                    .set_title("Select server.met")
                    .add_filter("ED2K server list", &["met"])
                    .add_filter("All files", &["*"])
                    .pick_file()
                    .await
            }
            PathPickerId::Ed2kNodeList => {
                dialog
                    .set_title("Select nodes.dat")
                    .add_filter("Kad nodes", &["dat"])
                    .add_filter("All files", &["*"])
                    .pick_file()
                    .await
            }
        };
        picked.map(|h| h.path().to_path_buf())
    };
    Task::perform(task, move |maybe| Message::PathPicked(id, maybe))
}
