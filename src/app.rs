use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::futures::SinkExt;
use iced::widget::{column, container, row, stack, text_editor};
use iced::window::Id;
use iced::{Element, Length, Subscription, Task};

use crate::config::{self, Settings};
use crate::db::Db;
use crate::engine::{EngineCmd, EngineEvent, EngineHandle, EventRx};
use crate::i18n::{Fluent, Locale};
use crate::message::{
    CloseDialogChoice, ConfirmAction, FileKind, Message, Page, SettingKey, SettingsCategory,
    SortField, SortOrder, TaskFilter, WindowCmd,
};
use crate::task::{DownloadTask, TaskStatus};
use crate::ui::add_dialog::AddDialogState;
use crate::ui::category_bar::Counts;
use crate::ui::details_dialog::DetailsDialogState;
use crate::ui::icons::{CATEGORY_W, SIDEBAR_W};
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
    aria2_version: Option<String>,
    aria2_check_msg: Option<String>,
    update_pending: Option<String>,
    aria2_status: Option<(String, String)>,
    aria2_fetch_error: Option<String>,
    logo_handle: iced::widget::image::Handle,
    ua_editor: text_editor::Content,
    headers_editor: text_editor::Content,
    torrent_files: HashMap<String, PathBuf>,
    pending_torrent_path: Option<PathBuf>,
    db: Option<Db>,
    dirty: HashSet<String>,
    details: DetailsDialogState,
    window_size: iced::Size,
    last_resize: Option<iced::Size>,
    geometry_dirty: bool,
    pending_close: bool,
    confirm: Option<ConfirmAction>,
    settings_dirty: bool,
    applied_settings: Settings,
}

pub fn init() -> (Remotrix, Task<Message>) {
    config::announce();
    let settings = config::load();

    let ua_editor = text_editor::Content::with_text(&settings.aria2.user_agent);
    let headers_editor = text_editor::Content::with_text(&settings.aria2.headers.join("\n"));

    let window_w = settings.window_width;
    let window_h = settings.window_height;
    let window_maximized = settings.window_maximized;

    let (handle, event_rx) = crate::engine::spawn_engine();

    let add_dialog = AddDialogState::new(settings.download_dir.clone());
    let fluent = Fluent::new(settings.locale);

    let theme = theme::build_iced(effective_theme_id(&settings));
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
        aria2_version: None,
        aria2_check_msg: None,
        update_pending: None,
        aria2_status: None,
        aria2_fetch_error: None,
        logo_handle,
        ua_editor,
        headers_editor,
        torrent_files: HashMap::new(),
        pending_torrent_path: None,
        db,
        dirty: HashSet::new(),
        details: DetailsDialogState::new(),
        window_size: iced::Size::new(window_w, window_h),
        last_resize: None,
        geometry_dirty: false,
        pending_close: false,
        confirm: None,
        settings_dirty: false,
    };

    (state, Task::none())
}

pub fn app_title(_state: &Remotrix) -> String {
    "Remotrix".to_string()
}

pub fn theme(state: &Remotrix) -> iced::Theme {
    state.theme.clone()
}

fn effective_theme_id(settings: &Settings) -> &str {
    if theme::resolve_mode(settings.theme_mode, None) {
        &settings.dark_theme
    } else {
        &settings.light_theme
    }
}

fn rebuild_theme(state: &mut Remotrix) {
    state.theme = theme::build_iced(effective_theme_id(&state.settings));
}

fn sync_geometry_to_settings(state: &mut Remotrix) {
    state.settings.window_width = state.window_size.width;
    state.settings.window_height = state.window_size.height;
    state.settings.window_maximized = state.maximized;
}

fn revert_apply_settings(state: &mut Remotrix) {
    state.settings.max_concurrent = state.applied_settings.max_concurrent;
    state.settings.download_limit_kb = state.applied_settings.download_limit_kb;
    state.settings.upload_limit_kb = state.applied_settings.upload_limit_kb;
    state.settings.split = state.applied_settings.split;
    state.settings.nav_to_tasks_after_add = state.applied_settings.nav_to_tasks_after_add;
    state.settings.delete_torrent_after_complete =
        state.applied_settings.delete_torrent_after_complete;
    state.settings.aria2 = state.applied_settings.aria2.clone();
    state.ua_editor = text_editor::Content::with_text(&state.settings.aria2.user_agent);
    state.headers_editor =
        text_editor::Content::with_text(&state.settings.aria2.headers.join("\n"));
    state.settings_dirty = false;
}

pub fn update(state: &mut Remotrix, message: Message) -> Task<Message> {
    match message {
        Message::NavigatePage(page) => {
            if page == Page::Tasks && state.page == Page::Settings && state.settings_dirty {
                state.confirm = Some(ConfirmAction::LeaveSettings { target: page });
            } else {
                state.page = page;
            }
        }
        Message::SetTaskFilter(filter) => {
            state.task_filter = filter;
        }
        Message::SetSettingsCategory(cat) => {
            state.settings_cat = cat;
        }
        Message::OpenAddDialog => {
            state
                .add_dialog
                .open(state.settings.download_dir.clone(), state.settings.split);
        }
        Message::CancelAdd => {
            state.add_dialog.close();
        }
        Message::AddUrlChanged(value) => {
            state.add_dialog.url = value;
        }
        Message::SaveDirChanged(value) => {
            state.add_dialog.save_dir = PathBuf::from(value);
        }
        Message::BrowseSaveDir => {
            tracing::debug!("ui: browse save dir");
            return pick_folder(FileKind::SaveDir);
        }
        Message::BrowseTorrent => {
            tracing::debug!("ui: browse torrent");
            return pick_file(FileKind::Torrent);
        }
        Message::FilePicked(kind, maybe_path) => {
            tracing::debug!(?kind, picked = maybe_path.is_some(), "ui: file picked");
            match kind {
                FileKind::SaveDir => {
                    if let Some(p) = maybe_path {
                        state.add_dialog.save_dir = p;
                    }
                }
                FileKind::Torrent => {
                    if let Some(p) = maybe_path {
                        state.add_dialog.torrent_path = Some(p.clone());
                        if state.add_dialog.url.trim().is_empty() {
                            state.add_dialog.url = p
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_string();
                        }
                    }
                }
            }
        }
        Message::SplitChanged(value) => {
            if let Ok(n) = value.parse::<u16>() {
                state.add_dialog.split = n.max(1);
            }
        }
        Message::AddDownload => {
            if state.add_dialog.can_submit() {
                let nav = state.settings.nav_to_tasks_after_add;

                if let Some(tpath) = state.add_dialog.torrent_path.clone() {
                    state.pending_torrent_path = Some(tpath.clone());
                    if state
                        .handle
                        .cmd_tx
                        .send(EngineCmd::AddTorrent {
                            path: tpath,
                            save_dir: state.add_dialog.save_dir.clone(),
                            split: state.add_dialog.split,
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
                    .url
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                if !urls.is_empty() {
                    if state
                        .handle
                        .cmd_tx
                        .send(EngineCmd::AddDownload {
                            urls: urls.clone(),
                            save_dir: state.add_dialog.save_dir.clone(),
                            split: state.add_dialog.split,
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
            if state.handle.cmd_tx.send(EngineCmd::Pause(gid)).is_err() {
                tracing::warn!("ui: pause cmd send failed");
            }
        }
        Message::ResumeTask(gid) => {
            if state.handle.cmd_tx.send(EngineCmd::Resume(gid)).is_err() {
                tracing::warn!("ui: resume cmd send failed");
            }
        }
        Message::RemoveTask(gid) => {
            if state.handle.cmd_tx.send(EngineCmd::Remove(gid)).is_err() {
                tracing::warn!("ui: remove cmd send failed");
            }
            state.confirm = None;
        }
        Message::StartAll => {
            if state.handle.cmd_tx.send(EngineCmd::ResumeAll).is_err() {
                tracing::warn!("ui: resume all cmd send failed");
            }
        }
        Message::PauseAll => {
            if state.handle.cmd_tx.send(EngineCmd::PauseAll).is_err() {
                tracing::warn!("ui: pause all cmd send failed");
            }
        }
        Message::DeleteAll => {
            if state.handle.cmd_tx.send(EngineCmd::RemoveAll).is_err() {
                tracing::warn!("ui: remove all cmd send failed");
            }
            state.tasks.clear();
            state.task_order.clear();
            state.dirty.clear();
            if let Some(ref db) = state.db {
                db.delete_all();
            }
            state.confirm = None;
        }
        Message::ClearCompleted => {
            let completed: Vec<String> = state
                .tasks
                .iter()
                .filter(|(_, t)| matches!(t.status, TaskStatus::Completed | TaskStatus::Removed))
                .map(|(gid, _)| gid.clone())
                .collect();
            if let Some(ref db) = state.db {
                db.clear_completed(&completed);
            }
            for gid in &completed {
                state.dirty.remove(gid);
            }
            state
                .tasks
                .retain(|_k, t| !matches!(t.status, TaskStatus::Completed | TaskStatus::Removed));
            state.task_order.retain(|gid| state.tasks.contains_key(gid));
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
        Message::OpenAbout => {
            state.about_dialog_visible = true;
        }
        Message::CloseAbout => {
            state.about_dialog_visible = false;
        }
        Message::SettingChanged(key, value) => {
            if key == SettingKey::DownloadDir {
                return pick_folder(FileKind::SaveDir);
            }
            state.settings_dirty = true;
            match key {
                SettingKey::DownloadDir => unreachable!(),
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
                    state.settings.aria2.min_split_size = value;
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
                SettingKey::Headers => {
                    state.settings.aria2.headers = value
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                SettingKey::AllProxy => {
                    state.settings.aria2.all_proxy = value;
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
                SettingKey::EnableProxy => {
                    state.settings.aria2.proxy_enabled = value == "true";
                }
                SettingKey::NavToTasksAfterAdd => {
                    state.settings.nav_to_tasks_after_add = value == "true";
                }
                SettingKey::DeleteTorrentAfterComplete => {
                    state.settings.delete_torrent_after_complete = value == "true";
                }
            }
        }
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
            state.applied_settings = state.settings.clone();
            state.settings_dirty = false;
        }
        Message::Engine(event) => match event {
            EngineEvent::EngineReady => {
                tracing::info!("engine ready");
                state.aria2_fetch_error = None;
            }
            EngineEvent::EngineStopped => {
                tracing::info!("engine stopped");
            }
            EngineEvent::Added {
                gid,
                name,
                url,
                dir,
            } => {
                tracing::info!(?gid, ?name, "ui: task added");
                if let Some(tpath) = state.pending_torrent_path.take() {
                    state.torrent_files.insert(gid.clone(), tpath);
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                if let Some(existing) = state.tasks.get_mut(&gid) {
                    let _ = existing;
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
                        status: TaskStatus::Waiting,
                        connections: 0,
                        added_at: now,
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
                        );
                    }
                }
                state.dirty.insert(gid);
            }
            EngineEvent::Progress {
                gid,
                downloaded,
                total,
                speed,
                status,
                connections,
            } => {
                if status == "complete"
                    && state.settings.delete_torrent_after_complete
                    && state.torrent_files.contains_key(&gid)
                {
                    if let Some(path) = state.torrent_files.remove(&gid) {
                        let _ = std::fs::remove_file(&path);
                    }
                }
                if let Some(t) = state.tasks.get_mut(&gid) {
                    t.downloaded = downloaded;
                    t.total = total;
                    t.speed = speed;
                    t.status = TaskStatus::from_engine(&status);
                    t.connections = connections;
                    state.dirty.insert(gid);
                }
            }
            EngineEvent::Removed(gid) => {
                tracing::info!(?gid, "ui: task removed");
                state.tasks.remove(&gid);
                state.task_order.retain(|g| g != &gid);
                state.dirty.remove(&gid);
                if let Some(ref db) = state.db {
                    db.delete(&gid);
                }
            }
            EngineEvent::TaskDetails { gid, details } => {
                tracing::debug!(?gid, "task details received");
                if state.details.gid.as_deref() == Some(&gid) {
                    state.details.details = Some(details);
                    state.details.loading = false;
                }
            }
            EngineEvent::TaskDetailsFailed { gid } => {
                tracing::debug!(?gid, "task details failed");
                if state.details.gid.as_deref() == Some(&gid) {
                    state.details.loading = false;
                }
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
                state.aria2_fetch_error = Some(error);
            }
            EngineEvent::EngineDegraded { reason } => {
                state.aria2_fetch_error = Some(reason);
            }
            EngineEvent::Aria2Status { stage, message } => {
                if stage == "ready" {
                    state.aria2_fetch_error = None;
                }
                state.aria2_status = Some((stage, message));
            }
        },
        Message::WindowResized(size) => {
            state.last_resize = Some(size);
            state.geometry_dirty = true;
        }
        Message::WindowOpened(id) => {
            if state.window_id.is_none() {
                state.window_id = Some(id);
            }
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
            state.show_close_dialog = true;
        }
        Message::CloseDialog(choice) => {
            state.show_close_dialog = false;
            return match choice {
                CloseDialogChoice::Close => {
                    tracing::info!("ui: shutdown requested");
                    if state.handle.cmd_tx.send(EngineCmd::Shutdown).is_err() {
                        tracing::warn!("ui: shutdown cmd send failed");
                    }
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
                CloseDialogChoice::Cancel => Task::none(),
                CloseDialogChoice::MinimizeToTray => Task::none(),
            };
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
        Message::LightThemeChanged(id) => {
            state.settings.light_theme = id;
            if !theme::resolve_mode(state.settings.theme_mode, None) {
                rebuild_theme(state);
            }
            config::save(&state.settings);
        }
        Message::DarkThemeChanged(id) => {
            state.settings.dark_theme = id;
            if theme::resolve_mode(state.settings.theme_mode, None) {
                rebuild_theme(state);
            }
            config::save(&state.settings);
        }
        Message::LocaleChanged(locale) => {
            state.settings.locale = locale;
            state.fluent = Fluent::new(locale);
            config::save(&state.settings);
        }
        Message::UaEditor(action) => {
            state.ua_editor.perform(action);
            state.settings.aria2.user_agent = state.ua_editor.text();
            state.settings_dirty = true;
        }
        Message::HeadersEditor(action) => {
            state.headers_editor.perform(action);
            state.settings.aria2.headers = state
                .headers_editor
                .text()
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            state.settings_dirty = true;
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
        Message::OpenTaskDetails(gid) => {
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
            if state.dirty.is_empty() {
                return Task::none();
            }
            let batch: Vec<(String, u64, u64, u64, u64, String)> = state
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
        Message::SelectDetailsTab(tab) => {
            state.details.active_tab = tab;
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
            let url = state
                .tasks
                .get(&gid)
                .map(|t| t.url.clone())
                .unwrap_or_default();
            if !url.is_empty() {
                return iced::clipboard::write::<Message>(url);
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
                state.applied_settings = state.settings.clone();
                state.settings_dirty = false;
                state.page = target;
            }
        }
        Message::DiscardAndLeaveSettings => {
            if let Some(ConfirmAction::LeaveSettings { target }) = state.confirm.take() {
                revert_apply_settings(state);
                state.page = target;
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
            )
        }
        Page::Settings => crate::ui::settings_page::view(
            &state.fluent,
            t,
            &state.settings,
            state.settings_cat,
            state.aria2_version.as_deref(),
            state.aria2_check_msg.as_deref(),
            state
                .aria2_status
                .as_ref()
                .map(|(s, m)| (s.as_str(), m.as_str())),
            state.aria2_fetch_error.as_deref(),
            state.update_pending.as_deref(),
            &state.ua_editor,
            &state.headers_editor,
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
    let mut stacked = framed;

    if state.add_dialog.is_visible() {
        stacked = stack![
            stacked,
            crate::ui::add_dialog::view(&state.fluent, t, &state.add_dialog),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }
    if state.about_dialog_visible {
        stacked = stack![
            stacked,
            crate::ui::about_dialog::view(&state.fluent, t, state.aria2_version.as_deref()),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }
    if state.show_close_dialog {
        stacked = stack![stacked, crate::ui::close_dialog::view(&state.fluent, t),]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    }
    if state.details.is_visible() {
        let task = state
            .details
            .gid
            .as_deref()
            .and_then(|g| state.tasks.get(g));
        stacked = stack![
            stacked,
            crate::ui::details_dialog::view(&state.fluent, t, task, &state.details),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }
    if let Some(ref action) = state.confirm {
        stacked = stack![
            stacked,
            crate::ui::confirm_dialog::view(&state.fluent, t, action),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

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

pub fn subscription(state: &Remotrix) -> Subscription<Message> {
    let engine =
        Subscription::run_with(EventSlot(state.event_rx_slot.clone()), build_engine_stream);

    let open = iced::window::open_events().map(Message::WindowOpened);
    let close = iced::window::close_requests().map(|_id| Message::CloseRequested);

    let flush = iced::time::every(Duration::from_millis(1000)).map(|_| Message::FlushDirty);

    let resizes = iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size));
    let persist_periodic =
        iced::time::every(Duration::from_millis(2000)).map(|_| Message::PersistWindowGeometry);

    let refresh = if state.details.is_visible() {
        iced::time::every(Duration::from_millis(2000)).map(|_| Message::RefreshTaskDetails)
    } else {
        Subscription::none()
    };

    Subscription::batch(vec![
        engine,
        open,
        close,
        flush,
        resizes,
        persist_periodic,
        refresh,
    ])
}

fn pick_folder(kind: FileKind) -> Task<Message> {
    let prompt = match kind {
        FileKind::SaveDir => "Select download folder",
        _ => "Select folder",
    };
    Task::perform(
        async move {
            rfd::AsyncFileDialog::new()
                .set_title(prompt)
                .pick_folder()
                .await
                .map(|h| h.path().to_path_buf())
        },
        move |maybe| Message::FilePicked(kind, maybe),
    )
}

fn pick_file(kind: FileKind) -> Task<Message> {
    Task::perform(
        async move {
            rfd::AsyncFileDialog::new()
                .add_filter("Torrent", &["torrent"])
                .pick_file()
                .await
                .map(|h| h.path().to_path_buf())
        },
        move |maybe| Message::FilePicked(kind, maybe),
    )
}
