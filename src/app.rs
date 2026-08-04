use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iced::alignment::{Horizontal, Vertical};
use iced::futures::SinkExt;
use iced::widget::{column, container, row, stack, text_editor};
use iced::window::Id;
use iced::{Element, Length, Padding, Subscription, Task};

use crate::config::{self, Settings};
use crate::db::Db;
use crate::engine::{EngineCmd, EngineEvent, EngineHandle, EventRx, TaskAdvancedOptions};
use crate::i18n::{Fluent, Tr};
use crate::message::{
    AddField, AddMsg, AddTab, CloseDialogChoice, ConfirmAction, DialogMsg, EngineMsg, Message,
    NavMsg, Page, PathPickerId, SettingKey, SettingValue, SettingsCategory, SettingsMsg, SortField,
    SortMsg, SortOrder, TaskFilter, TaskMsg, ToastMsg, WindowCmd, WindowMsg,
};
use crate::task::{DownloadTask, TaskStatus};
use crate::ui::add_dialog::AddDialogState;
use crate::ui::category_bar::Counts;
use crate::ui::components::file_tree::FileTreeNode;
use crate::ui::components::path_picker::PathPickerAction;
use crate::ui::components::toast::{Toast, ToastGroup, ToastKind};
use crate::ui::components::torrent_upload::{self, TorrentUploadAction};
use crate::ui::details_dialog::DetailsDialogState;
use crate::ui::icons::{CATEGORY_W, SIDEBAR_W};
use crate::ui::settings_page::SettingsUiState;
use crate::ui::theme;

struct ToastManager {
    toasts: Vec<crate::ui::components::toast::Toast>,
    next_toast_id: u64,
    hovered_toast_id: Option<u64>,
}

impl ToastManager {
    fn new() -> Self {
        Self {
            toasts: Vec::new(),
            next_toast_id: 0,
            hovered_toast_id: None,
        }
    }

    fn push(&mut self, mut toast: crate::ui::components::toast::Toast) -> u64 {
        const CAP: usize = 6;
        let id = self.next_toast_id;
        self.next_toast_id += 1;
        toast.id = id;
        let pos = toast.position;
        let group = toast.group;
        let removed_hovered = matches!(
            self.hovered_toast_id,
            Some(h)
                if self.toasts.iter().any(|t| t.id == h && t.position == pos && t.group == group && t.close_after.is_some())
        );
        self.toasts
            .retain(|t| !(t.position == pos && t.group == group && t.close_after.is_some()));
        if removed_hovered {
            self.hovered_toast_id = None;
        }
        let at_pos = self.toasts.iter().filter(|t| t.position == pos).count();
        if at_pos >= CAP {
            if let Some(idx) = self.toasts.iter().position(|t| t.position == pos) {
                self.toasts.remove(idx);
            }
        }
        toast.remaining = toast.close_after;
        self.toasts.push(toast);
        id
    }

    fn spawn(
        &mut self,
        group: ToastGroup,
        kind: ToastKind,
        message: String,
        close_after: Option<Duration>,
        show_close: bool,
    ) -> u64 {
        let mut toast = Toast::new(kind, message)
            .group(group)
            .close_after(close_after);
        if show_close {
            toast = toast.show_close();
        }
        self.push(toast)
    }

    fn dismiss(&mut self, id: u64) {
        self.toasts.retain(|t| t.id != id);
        if self.hovered_toast_id == Some(id) {
            self.hovered_toast_id = None;
        }
    }

    fn hover(&mut self, id: u64) {
        self.hovered_toast_id = Some(id);
    }

    fn unhover(&mut self, id: u64) {
        if self.hovered_toast_id == Some(id) {
            self.hovered_toast_id = None;
        }
    }

    fn tick(&mut self) {
        const TICK: Duration = Duration::from_millis(200);
        let mut expired = Vec::new();
        for toast in self.toasts.iter_mut() {
            if let Some(rem) = toast.remaining.as_mut() {
                if Some(toast.id) != self.hovered_toast_id {
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
            self.dismiss(id);
        }
    }
}

struct EngineUiState {
    aria2_version: Option<String>,
    aria2_check_msg: Option<String>,
    update_pending: Option<String>,
    aria2_status: Option<(String, String)>,
    aria2_fetch_error: Option<String>,
    downloading_toast_id: Option<u64>,
    startup_error_toast_id: Option<u64>,
    startup_starting_toast_shown: bool,
}

impl EngineUiState {
    fn new() -> Self {
        Self {
            aria2_version: None,
            aria2_check_msg: None,
            update_pending: None,
            aria2_status: None,
            aria2_fetch_error: None,
            downloading_toast_id: None,
            startup_error_toast_id: None,
            startup_starting_toast_shown: false,
        }
    }
}

struct WindowState {
    maximized: bool,
    show_close_dialog: bool,
    window_id: Option<Id>,
    window_size: iced::Size,
    last_resize: Option<iced::Size>,
    geometry_dirty: bool,
    pending_close: bool,
    closing: bool,
}

impl WindowState {
    fn new(window_size: iced::Size, maximized: bool) -> Self {
        Self {
            maximized,
            show_close_dialog: false,
            window_id: None,
            window_size,
            last_resize: None,
            geometry_dirty: false,
            pending_close: false,
            closing: false,
        }
    }
}

struct EngineRestartState {
    engine_restart_pending: bool,
    engine_restart_in_progress: bool,
    restart_resume_gids: HashSet<String>,
}

impl EngineRestartState {
    fn new() -> Self {
        Self {
            engine_restart_pending: false,
            engine_restart_in_progress: false,
            restart_resume_gids: HashSet::new(),
        }
    }
}

struct TaskTracking {
    paused_gids: HashSet<String>,
    synced_gids: HashSet<String>,
    removed_gids: HashMap<String, Instant>,
    sync_done: bool,
    active_count: usize,
    dirty: HashSet<String>,
    completion_toasted: HashSet<String>,
    torrent_files: HashMap<String, PathBuf>,
    torrent_followed: HashSet<String>,
}

impl TaskTracking {
    fn new(active_count: usize) -> Self {
        Self {
            paused_gids: HashSet::new(),
            synced_gids: HashSet::new(),
            removed_gids: HashMap::new(),
            sync_done: false,
            active_count,
            dirty: HashSet::new(),
            completion_toasted: HashSet::new(),
            torrent_files: HashMap::new(),
            torrent_followed: HashSet::new(),
        }
    }
}

pub struct Remotrix {
    page: Page,
    task_filter: TaskFilter,
    settings_cat: SettingsCategory,
    tasks: HashMap<String, DownloadTask>,
    task_order: Vec<String>,
    handle: EngineHandle,
    event_rx_slot: Arc<Mutex<Option<EventRx>>>,
    add_dialog: AddDialogState,
    drop_hover: bool,
    about_dialog_visible: bool,
    settings: Settings,
    fluent: Fluent,
    theme: iced::Theme,
    sort_menu_open: bool,
    sort_field: SortField,
    sort_order: SortOrder,
    search_query: String,
    ua_editor: text_editor::Content,
    bt_tracker_editor: text_editor::Content,
    db: Option<Db>,
    details: DetailsDialogState,
    confirm: Option<ConfirmAction>,
    applied_settings: Settings,
    applied_font_family: String,
    restart_pending: bool,
    settings_ui: SettingsUiState,
    global_speed: Option<(u64, u64)>,
    toasts: ToastManager,
    engine_ui: EngineUiState,
    window: WindowState,
    restart: EngineRestartState,
    tracking: TaskTracking,
}

pub fn init() -> (Remotrix, Task<Message>) {
    config::announce();
    let settings = config::load();
    std::thread::spawn(|| {
        crate::ui::theme::system_font_families();
    });

    let ua_editor = text_editor::Content::with_text(&settings.aria2.user_agent);
    let bt_tracker_editor =
        text_editor::Content::with_text(&crate::trackers::to_lines(&settings.aria2.bt_tracker));

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
    let (db, db_open_failed) = match crate::config::db_path() {
        Some(p) => match Db::open(&p) {
            Ok(d) => (Some(d), false),
            Err(e) => {
                tracing::error!(error = %e, "db open failed");
                (None, true)
            }
        },
        None => (None, false),
    };
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

    let mut state = Remotrix {
        page: Page::Tasks,
        task_filter: TaskFilter::All,
        settings_cat: SettingsCategory::General,
        tasks,
        task_order,
        handle,
        event_rx_slot: Arc::new(Mutex::new(Some(event_rx))),
        add_dialog,
        drop_hover: false,
        about_dialog_visible: false,
        applied_settings: settings.clone(),
        applied_font_family: settings.font_family.clone(),
        restart_pending: false,
        settings,
        fluent,
        theme,
        sort_menu_open: false,
        sort_field: SortField::AddedTime,
        sort_order: SortOrder::Desc,
        search_query: String::new(),
        ua_editor,
        bt_tracker_editor,
        db,
        details: DetailsDialogState::new(),
        confirm: None,
        settings_ui,
        global_speed: None,
        toasts: ToastManager::new(),
        engine_ui: EngineUiState::new(),
        window: WindowState::new(iced::Size::new(window_w, window_h), window_maximized),
        restart: EngineRestartState::new(),
        tracking: TaskTracking::new(active_count),
    };

    if db_open_failed {
        state.toasts.spawn(
            ToastGroup::General,
            ToastKind::Warning,
            state.fluent.get(crate::i18n::Tr::DatabaseError),
            Some(Duration::from_secs(6)),
            true,
        );
    }

    (
        state,
        Task::done(Message::Settings(SettingsMsg::CheckTrackerAutoSync {
            startup: true,
        })),
    )
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
    state.settings.window_width = state.window.window_size.width;
    state.settings.window_height = state.window.window_size.height;
    state.settings.window_maximized = state.window.maximized;
}

fn revert_apply_settings(state: &mut Remotrix) {
    state.settings = state.applied_settings.clone();
    state
        .settings_ui
        .download_picker
        .set_value(state.settings.download_dir.to_string_lossy());
    state
        .settings_ui
        .ed2k_server_list_picker
        .set_value(state.settings.aria2.ed2k_server_list.clone());
    state
        .settings_ui
        .ed2k_node_list_picker
        .set_value(state.settings.aria2.ed2k_node_list.clone());
    state.ua_editor = text_editor::Content::with_text(&state.settings.aria2.user_agent);
    state.bt_tracker_editor = text_editor::Content::with_text(&crate::trackers::to_lines(
        &state.settings.aria2.bt_tracker,
    ));
    crate::logging::set_app_level(&state.settings.log.app_level);
}

fn apply_settings(state: &mut Remotrix) -> bool {
    config::save(&state.settings);
    let opts = state.settings.effective_task_options();
    tracing::info!(
        app_log_level = %state.settings.log.app_level,
        engine_log_level = %state.settings.log.engine_level,
        "ui: apply settings"
    );
    if state
        .handle
        .cmd_tx
        .send(EngineCmd::ApplyAria2Options { options: opts })
        .is_err()
    {
        tracing::warn!("ui: apply aria2 options cmd send failed");
    }
    if state
        .handle
        .cmd_tx
        .send(EngineCmd::ReloadSchedules)
        .is_err()
    {
        tracing::warn!("ui: reload schedules cmd send failed");
    }
    let restart_needed = !state
        .settings
        .aria2
        .ed2k_equal(&state.applied_settings.aria2);
    if restart_needed && state.handle.cmd_tx.send(EngineCmd::RestartEngine).is_err() {
        tracing::warn!("ui: restart engine cmd send failed");
    }
    state.restart.engine_restart_pending = restart_needed
        || state.settings.log.engine_level != state.applied_settings.log.engine_level;
    state.applied_settings = state.settings.clone();
    restart_needed
}

fn clear_all_local(state: &mut Remotrix) {
    for gid in state.tasks.keys() {
        state
            .tracking
            .removed_gids
            .insert(gid.clone(), Instant::now());
    }
    state.tasks.clear();
    state.task_order.clear();
    state.tracking.dirty.clear();
    state.tracking.active_count = 0;
    state.tracking.paused_gids.clear();
    if let Some(ref db) = state.db {
        db.delete_all();
    }
}

fn remove_task_local(state: &mut Remotrix, gid: &str) {
    if let Some(t) = state.tasks.get(gid) {
        if t.status == TaskStatus::Active {
            state.tracking.active_count = state.tracking.active_count.saturating_sub(1);
        }
    }
    state
        .tracking
        .removed_gids
        .insert(gid.to_string(), Instant::now());
    let _ = state.tracking.torrent_files.remove(gid);
    state.tracking.torrent_followed.remove(gid);
    state.tracking.completion_toasted.remove(gid);
    state.tracking.paused_gids.remove(gid);
    state.tasks.remove(gid);
    state.task_order.retain(|g| g != gid);
    state.tracking.dirty.remove(gid);
    if let Some(ref db) = state.db {
        db.delete(gid);
    }
}

const REMOVED_GID_GRACE: Duration = Duration::from_secs(60);

fn gid_recently_removed(state: &mut Remotrix, gid: &str) -> bool {
    match state.tracking.removed_gids.get(gid) {
        Some(&removed_at) if removed_at.elapsed() < REMOVED_GID_GRACE => true,
        Some(_) => {
            state.tracking.removed_gids.remove(gid);
            false
        }
        None => false,
    }
}

fn resolve_metadata_name(path: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    crate::torrent_meta::parse_torrent(&bytes).map(|m| m.name)
}

fn apply_task_name(db: &Option<Db>, gid: &str, t: &mut DownloadTask, incoming: String) {
    if incoming.starts_with("[METADATA]") {
        let placeholder =
            t.name.is_empty() || t.name.starts_with("[METADATA]") || t.name == "magnet:";
        if placeholder {
            let path = t.save_dir.join(&incoming);
            let size = std::fs::metadata(&path).ok().map(|m| m.len());
            if size.is_some() && size != t.metadata_probe_size {
                t.metadata_probe_size = size;
                if let Some(real) = resolve_metadata_name(&path) {
                    let real = std::path::Path::new(&real)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                        .to_string();
                    if !real.is_empty() {
                        t.name = real;
                        if let Some(ref db) = db {
                            db.update_name(gid, &t.name);
                        }
                        return;
                    }
                }
            }
        } else {
            return;
        }
        if !t.name.starts_with("[METADATA]") {
            t.name = incoming;
            if let Some(ref db) = db {
                db.update_name(gid, &t.name);
            }
        }
        return;
    }
    if t.name != incoming {
        t.name = incoming;
        if let Some(ref db) = db {
            db.update_name(gid, &t.name);
        }
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
        state.tracking.dirty.remove(gid);
    }
    state
        .tasks
        .retain(|_k, t| !matches!(t.status, TaskStatus::Completed | TaskStatus::Removed));
    state.task_order.retain(|gid| state.tasks.contains_key(gid));
}

fn flush_dirty(state: &mut Remotrix) {
    if state.tracking.dirty.is_empty() {
        return;
    }
    let batch: Vec<(String, u64, u64, u64, u64, u64, String)> = state
        .tracking
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
    state.tracking.dirty.clear();
}

fn begin_close(state: &mut Remotrix) -> Task<Message> {
    if state.window.closing {
        return Task::none();
    }
    state.window.closing = true;
    state.window.show_close_dialog = false;
    state.details.select_gen += 1;
    if let Some((gid, files)) = state.details.pending_select.take() {
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
    let hide = state
        .window
        .window_id
        .map(|id| iced::window::set_mode::<Message>(id, iced::window::Mode::Hidden))
        .unwrap_or_else(Task::none);
    hide.chain(shutdown_timeout_task())
}

fn shutdown_timeout_task() -> Task<Message> {
    Task::perform(
        async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
        },
        |_| Message::Window(WindowMsg::ShutdownTimeout),
    )
}

fn engine_restart_safety_timeout_task() -> Task<Message> {
    Task::perform(
        async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
        },
        |_| Message::Engine(EngineMsg::EngineRestartSafetyTimeout),
    )
}

fn engine_restart_cooldown_task() -> Task<Message> {
    Task::perform(
        async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
        },
        |_| Message::Engine(EngineMsg::EngineRestartCooldownFinished),
    )
}

fn finalize_close(state: &mut Remotrix) -> Task<Message> {
    if !state.window.closing {
        return Task::none();
    }
    state.window.closing = false;
    flush_dirty(state);
    if state.window.geometry_dirty {
        state.window.pending_close = true;
        if let Some(id) = state.window.window_id {
            return iced::window::is_maximized(id)
                .then(|max| Task::done(Message::Window(WindowMsg::WindowMaximized(max))));
        }
    }
    sync_geometry_to_settings(state);
    let mut save = state.applied_settings.clone();
    save.window_width = state.settings.window_width;
    save.window_height = state.settings.window_height;
    save.window_maximized = state.settings.window_maximized;
    save.path_history = state.settings.path_history.clone();
    save.last_clipboard_hash = state.settings.last_clipboard_hash.clone();
    save.update = state.settings.update.clone();
    config::save(&save);
    spawn_restart_if_pending(state);
    if let Some(id) = state.window.window_id {
        iced::window::close::<Message>(id)
    } else {
        Task::none()
    }
}

fn spawn_restart_if_pending(state: &mut Remotrix) {
    if state.restart_pending {
        state.restart_pending = false;
        spawn_detached_self();
    }
}

fn spawn_detached_self() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(err) = std::process::Command::new(exe)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        tracing::warn!(error = %err, "ui: failed to spawn restart process");
    }
}

fn read_clipboard(state: &Remotrix) -> Task<Message> {
    if !state.settings.detect_clipboard_on_start {
        return Task::none();
    }
    iced::clipboard::read().map(|content| Message::Window(WindowMsg::ClipboardRead(content)))
}

pub fn update(state: &mut Remotrix, message: Message) -> Task<Message> {
    match message {
        Message::Nav(NavMsg::NavigatePage(page)) => {
            state.settings_ui.download_picker.close_history();
            if page == Page::Tasks
                && state.page == Page::Settings
                && state.settings != state.applied_settings
            {
                state.confirm = Some(ConfirmAction::LeaveSettings { target: page });
            } else {
                state.page = page;
            }
        }
        Message::Nav(NavMsg::SetTaskFilter(filter)) => {
            state.task_filter = filter;
        }
        Message::Nav(NavMsg::SetSettingsCategory(cat)) => {
            state.settings_ui.download_picker.close_history();
            state.settings_cat = cat;
        }
        Message::Add(AddMsg::OpenAddDialog) => {
            state.add_dialog.save_picker.close_history();
            state
                .add_dialog
                .open(state.settings.download_dir.clone(), state.settings.split);
        }
        Message::Add(AddMsg::CancelAdd) => {
            state.add_dialog.save_picker.close_history();
            state.add_dialog.close();
        }
        Message::Add(AddMsg::SelectAddTab(tab)) => {
            state.add_dialog.active_tab = tab;
        }
        Message::Add(AddMsg::TorrentUpload(event)) => {
            if let Some(TorrentUploadAction::Browse) = state.add_dialog.handle_torrent_event(event)
            {
                return pick_path(PathPickerId::Torrent);
            }
        }
        Message::Add(AddMsg::TorrentTreeExpand(path)) => {
            state.add_dialog.toggle_torrent_expand(&path);
        }
        Message::Add(AddMsg::TorrentTreeToggle(path)) => {
            state.add_dialog.toggle_torrent_node(&path);
        }
        Message::Add(AddMsg::TorrentFilesSelectAll) => {
            state.add_dialog.set_all_torrent_files(true);
        }
        Message::Add(AddMsg::TorrentFilesSelectNone) => {
            state.add_dialog.set_all_torrent_files(false);
        }
        Message::Add(AddMsg::TorrentFilesScroll(off)) => {
            state.add_dialog.torrent_scroll_offset = off;
        }
        Message::Add(AddMsg::TorrentFilesTogglePanel) => {
            state.add_dialog.toggle_torrent_panel();
        }
        Message::Add(AddMsg::FileHovered) => {
            state.drop_hover = true;
            if state.add_dialog.is_visible() && state.add_dialog.active_tab == AddTab::Torrent {
                state.add_dialog.torrent_upload.set_dragging(true);
            }
        }
        Message::Add(AddMsg::FilesHoveredLeft) => {
            state.drop_hover = false;
            if state.add_dialog.is_visible() {
                state.add_dialog.torrent_upload.set_dragging(false);
            }
        }
        Message::Add(AddMsg::FileDropped(path)) => {
            state.drop_hover = false;
            if state.add_dialog.is_visible() {
                state.add_dialog.torrent_upload.set_dragging(false);
            }
            if state.window.show_close_dialog
                || state.about_dialog_visible
                || state.confirm.is_some()
            {
                return Task::none();
            }
            let prefs = state.settings.clipboard_types;
            let path_str = path.to_string_lossy().to_string();
            return Task::perform(
                async move { crate::clipboard_watch::parse_clipboard(&path_str, prefs) },
                |payload| Message::Window(WindowMsg::DroppedFileParsed(payload)),
            );
        }
        Message::Window(WindowMsg::DroppedFileParsed(payload)) => {
            if state.window.show_close_dialog
                || state.about_dialog_visible
                || state.confirm.is_some()
            {
                return Task::none();
            }
            let Some(payload) = payload else {
                spawn_toast(
                    state,
                    ToastGroup::Task,
                    ToastKind::Warning,
                    state.fluent.get(Tr::NoDownloadableContent),
                    Some(Duration::from_secs(4)),
                    false,
                );
                return Task::none();
            };
            if let crate::clipboard_watch::ClipboardPayload::Torrent(ref path) = payload {
                if !torrent_upload::is_valid_torrent_file(path) {
                    spawn_toast(
                        state,
                        ToastGroup::Task,
                        ToastKind::Warning,
                        state.fluent.get(Tr::InvalidTorrent),
                        Some(Duration::from_secs(4)),
                        false,
                    );
                    return Task::none();
                }
            }
            if state.add_dialog.is_visible() {
                state.add_dialog.apply_payload(payload);
                return Task::none();
            }
            state.add_dialog.open_with(
                state.settings.download_dir.clone(),
                state.settings.split,
                payload,
            );
            spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::DropDetected),
                Some(Duration::from_secs(3)),
                false,
            );
            return Task::none();
        }
        Message::Add(AddMsg::UrlEditor(action)) => {
            state.add_dialog.url_editor.perform(action);
        }
        Message::Add(AddMsg::PathPicker(id, event)) => {
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
        Message::Add(AddMsg::PathPicked(id, maybe_path)) => {
            tracing::debug!(?id, picked = maybe_path.is_some(), "ui: path picked");
            if let Some(p) = maybe_path {
                apply_path(state, id, p);
            }
        }
        Message::Task(TaskMsg::CopyPath(s)) => {
            if !s.is_empty() {
                return iced::clipboard::write::<Message>(s);
            }
        }
        Message::Add(AddMsg::SplitChanged(value)) => {
            if let Ok(n) = value.parse::<u16>() {
                state.add_dialog.split = n.max(1);
            }
        }
        Message::Add(AddMsg::ToggleAdvanced(value)) => {
            state.add_dialog.advanced_open = value;
        }
        Message::Add(AddMsg::AddFieldChanged(field, value)) => {
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
        Message::Add(AddMsg::AddDownload) => {
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
        Message::Task(TaskMsg::PauseTask(gid)) => {
            state.tracking.paused_gids.insert(gid.clone());
            if state.handle.cmd_tx.send(EngineCmd::Pause(gid)).is_err() {
                tracing::warn!("ui: pause cmd send failed");
            }
        }
        Message::Task(TaskMsg::ResumeTask(gid)) => {
            state.tracking.paused_gids.remove(&gid);
            if state.handle.cmd_tx.send(EngineCmd::Resume(gid)).is_err() {
                tracing::warn!("ui: resume cmd send failed");
            }
        }
        Message::Task(TaskMsg::RedownloadTask(gid)) => {
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
        }
        Message::Task(TaskMsg::RemoveTask(gid)) => {
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
            state.confirm = None;
            let _ = spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::TaskRemoved),
                Some(Duration::from_secs(3)),
                false,
            );
        }
        Message::Task(TaskMsg::DeleteTask(gid)) => {
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
            state.confirm = None;
            let _ = spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::TaskDeleted),
                Some(Duration::from_secs(3)),
                false,
            );
        }
        Message::Task(TaskMsg::StartAll) => {
            state.tracking.paused_gids.clear();
            if state.handle.cmd_tx.send(EngineCmd::ResumeAll).is_err() {
                tracing::warn!("ui: resume all cmd send failed");
            }
        }
        Message::Task(TaskMsg::PauseAll) => {
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
        }
        Message::Task(TaskMsg::DeleteAll) => {
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
            let _ = spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::TasksDeleted),
                Some(Duration::from_secs(3)),
                false,
            );
        }
        Message::Task(TaskMsg::RemoveAllRecords) => {
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
            let _ = spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::TasksRemoved),
                Some(Duration::from_secs(3)),
                false,
            );
        }
        Message::Task(TaskMsg::ClearCompleted) => {
            let completed: Vec<String> = state
                .tasks
                .iter()
                .filter(|(_, t)| matches!(t.status, TaskStatus::Completed | TaskStatus::Removed))
                .map(|(gid, _)| gid.clone())
                .collect();
            clear_completed_local(state, &completed);
            state.confirm = None;
        }
        Message::Task(TaskMsg::Refresh) => {
            if state.handle.cmd_tx.send(EngineCmd::Snapshot).is_err() {
                tracing::warn!("ui: snapshot cmd send failed");
            }
        }
        Message::Sort(SortMsg::SortSelected(field)) => {
            state.sort_field = field;
        }
        Message::Sort(SortMsg::ToggleSortMenu) => {
            state.sort_menu_open = !state.sort_menu_open;
        }
        Message::Sort(SortMsg::CloseSortMenu) => {
            state.sort_menu_open = false;
        }
        Message::Sort(SortMsg::ToggleSortOrder) => {
            state.sort_order = match state.sort_order {
                SortOrder::Asc => SortOrder::Desc,
                SortOrder::Desc => SortOrder::Asc,
            };
        }
        Message::Sort(SortMsg::SearchChanged(query)) => {
            state.search_query = query;
        }
        Message::Dialog(DialogMsg::OpenAbout) => {
            state.about_dialog_visible = true;
        }
        Message::Dialog(DialogMsg::CloseAbout) => {
            state.about_dialog_visible = false;
        }
        Message::Settings(SettingsMsg::SettingChanged(key, value)) => match key {
            SettingKey::MaxConcurrent => {
                if let SettingValue::Num(n) = value {
                    state.settings.max_concurrent = n.max(1) as u32;
                }
            }
            SettingKey::Split => {
                if let SettingValue::Num(n) = value {
                    state.settings.split = n.max(1) as u16;
                }
            }
            SettingKey::DownloadLimit => {
                if let SettingValue::Num(n) = value {
                    state.settings.download_limit_kb = n;
                }
            }
            SettingKey::UploadLimit => {
                if let SettingValue::Num(n) = value {
                    state.settings.upload_limit_kb = n;
                }
            }
            SettingKey::MaxConnectionPerServer => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.max_connection_per_server = n.max(1) as u32;
                }
            }
            SettingKey::MinSplitSize => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.min_split_size_mb = n;
                }
            }
            SettingKey::AutoFileRenaming => {
                if let SettingValue::Bool(b) = value {
                    state.settings.aria2.auto_file_renaming = b;
                }
            }
            SettingKey::AllowOverwrite => {
                if let SettingValue::Bool(b) = value {
                    state.settings.aria2.allow_overwrite = b;
                }
            }
            SettingKey::Continue => {
                if let SettingValue::Bool(b) = value {
                    state.settings.aria2.r#continue = b;
                }
            }
            SettingKey::CheckIntegrity => {
                if let SettingValue::Bool(b) = value {
                    state.settings.aria2.check_integrity = b;
                }
            }
            SettingKey::MaxDownloadLimit => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.max_download_limit_kb = n;
                }
            }
            SettingKey::MaxUploadLimit => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.max_upload_limit_kb = n;
                }
            }
            SettingKey::LowestSpeedLimit => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.lowest_speed_limit_kb = n;
                }
            }
            SettingKey::ProxyServer => {
                if let SettingValue::Text(s) = value {
                    state.settings.aria2.proxy_server = s;
                }
            }
            SettingKey::ProxyUsername => {
                if let SettingValue::Text(s) = value {
                    state.settings.aria2.proxy_username = s;
                }
            }
            SettingKey::ProxyPassword => {
                if let SettingValue::Text(s) = value {
                    state.settings.aria2.proxy_password = s;
                }
            }
            SettingKey::MaxTries => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.max_tries = n as u32;
                }
            }
            SettingKey::RetryWait => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.retry_wait = n as u32;
                }
            }
            SettingKey::ConnectTimeout => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.connect_timeout = n as u32;
                }
            }
            SettingKey::TrackerAutoSync => {
                if let SettingValue::Bool(b) = value {
                    state.settings.tracker.auto_sync = b;
                }
            }
            SettingKey::TrackerSyncInterval => {
                if let SettingValue::Num(n) = value {
                    state.settings.tracker.sync_interval_hours = n as u32;
                }
            }
            SettingKey::SeedRatio => {
                if let SettingValue::NumF(n) = value {
                    state.settings.aria2.seed_ratio = n.max(0.0);
                }
            }
            SettingKey::SeedTime => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.seed_time = n as u32;
                }
            }
            SettingKey::EnableDht => {
                if let SettingValue::Bool(b) = value {
                    state.settings.aria2.enable_dht = b;
                }
            }
            SettingKey::BtRequireCrypto => {
                if let SettingValue::Bool(b) = value {
                    state.settings.aria2.bt_require_crypto = b;
                }
            }
            SettingKey::BtEnableLpd => {
                if let SettingValue::Bool(b) = value {
                    state.settings.aria2.bt_enable_lpd = b;
                }
            }
            SettingKey::EnablePeerExchange => {
                if let SettingValue::Bool(b) = value {
                    state.settings.aria2.enable_peer_exchange = b;
                }
            }
            SettingKey::BtAutoDownload => {
                if let SettingValue::Bool(b) = value {
                    state.settings.aria2.bt_auto_download = b;
                }
            }
            SettingKey::FileAllocation => {
                if let SettingValue::Text(s) = value {
                    state.settings.aria2.file_allocation = s;
                }
            }
            SettingKey::DiskCache => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.disk_cache_mb = n;
                }
            }
            SettingKey::EnableProxy => {
                if let SettingValue::Bool(b) = value {
                    state.settings.aria2.proxy_enabled = b;
                }
            }
            SettingKey::NavToTasksAfterAdd => {
                if let SettingValue::Bool(b) = value {
                    state.settings.nav_to_tasks_after_add = b;
                }
            }
            SettingKey::DeleteTorrentAfterComplete => {
                if let SettingValue::Bool(b) = value {
                    state.settings.delete_torrent_after_complete = b;
                }
            }
            SettingKey::CleanupCompletedOnClose => {
                if let SettingValue::Bool(b) = value {
                    state.settings.cleanup_completed_on_close = b;
                }
            }
            SettingKey::RemoveTaskIfFilesMissing => {
                if let SettingValue::Bool(b) = value {
                    state.settings.remove_task_if_files_missing = b;
                }
            }
            SettingKey::DetectClipboardOnStart => {
                if let SettingValue::Bool(b) = value {
                    state.settings.detect_clipboard_on_start = b;
                }
            }
            SettingKey::ClipboardHttp => {
                if let SettingValue::Bool(b) = value {
                    state.settings.clipboard_types.http = b;
                }
            }
            SettingKey::ClipboardFtp => {
                if let SettingValue::Bool(b) = value {
                    state.settings.clipboard_types.ftp = b;
                }
            }
            SettingKey::ClipboardMagnet => {
                if let SettingValue::Bool(b) = value {
                    state.settings.clipboard_types.magnet = b;
                }
            }
            SettingKey::ClipboardEd2k => {
                if let SettingValue::Bool(b) = value {
                    state.settings.clipboard_types.ed2k = b;
                }
            }
            SettingKey::ClipboardThunder => {
                if let SettingValue::Bool(b) = value {
                    state.settings.clipboard_types.thunder = b;
                }
            }
            SettingKey::ClipboardBtInfohash => {
                if let SettingValue::Bool(b) = value {
                    state.settings.clipboard_types.bt_infohash = b;
                }
            }
            SettingKey::Ed2kServer => {
                if let SettingValue::Text(s) = value {
                    state.settings.aria2.ed2k_server = s;
                }
            }
            SettingKey::Ed2kListenPort => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.ed2k_listen_port = n as u16;
                }
            }
            SettingKey::Ed2kUdpListenPort => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.ed2k_udp_listen_port = n as u16;
                }
            }
            SettingKey::Ed2kUploadSlots => {
                if let SettingValue::Num(n) = value {
                    state.settings.aria2.ed2k_upload_slots = n.max(1) as u16;
                }
            }
            SettingKey::SpeedLimitScheduleEnabled => {
                if let SettingValue::Bool(b) = value {
                    state.settings.speed_limit_schedule.enabled = b;
                }
            }
            SettingKey::ScheduleStart => {
                if let SettingValue::Text(s) = value {
                    if crate::scheduler::parse_hhmm(&s).is_some() {
                        state.settings.speed_limit_schedule.start = s;
                    }
                }
            }
            SettingKey::ScheduleEnd => {
                if let SettingValue::Text(s) = value {
                    if crate::scheduler::parse_hhmm(&s).is_some() {
                        state.settings.speed_limit_schedule.end = s;
                    }
                }
            }
            SettingKey::AppLogLevel => {
                if let SettingValue::Text(s) = value {
                    state.settings.log.app_level = crate::logging::normalize_app_level(&s);
                    crate::logging::set_app_level(&state.settings.log.app_level);
                }
            }
            SettingKey::EngineLogLevel => {
                if let SettingValue::Text(s) = value {
                    state.settings.log.engine_level = crate::logging::normalize_engine_level(&s);
                }
            }
        },
        Message::Settings(SettingsMsg::ApplySettings) => {
            apply_settings(state);
        }
        Message::Settings(SettingsMsg::ResetSettings) => {
            revert_apply_settings(state);
            config::save(&state.settings);
        }
        Message::Settings(SettingsMsg::ClearLogs) => match crate::logging::clear_logs() {
            Ok(count) => {
                tracing::info!(count, "ui: cleared log files");
                let toast = Toast::new(ToastKind::Success, state.fluent.get(Tr::LogsCleared))
                    .group(ToastGroup::Logs)
                    .close_after(Some(Duration::from_secs(3)));
                state.toasts.push(toast);
            }
            Err(e) => {
                tracing::warn!(?e, "ui: clear log files failed");
                let toast = Toast::new(ToastKind::Error, state.fluent.get(Tr::LogsClearFailed))
                    .group(ToastGroup::Logs)
                    .close_after(Some(Duration::from_secs(5)));
                state.toasts.push(toast);
            }
        },
        Message::Engine(EngineMsg::Event(event)) => match event {
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
                    let gids: Vec<String> =
                        state.restart.restart_resume_gids.iter().cloned().collect();
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
                return Task::none();
            }
            EngineEvent::EngineStopped => {
                tracing::info!("engine stopped");
                state.global_speed = None;
                state.tracking.paused_gids.clear();
                if state.window.closing {
                    return finalize_close(state);
                }
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
                    remove_task_local(state, gid);
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
            }
            EngineEvent::Added {
                gid,
                name,
                url,
                dir,
                info_hash,
            } => {
                tracing::info!(?gid, ?name, "ui: task added");
                state.tracking.synced_gids.insert(gid.clone());
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                if let Some(existing) = state.tasks.get_mut(&gid) {
                    apply_task_name(&state.db, &gid, existing, name);
                    existing.url = url;
                    existing.save_dir = PathBuf::from(dir);
                    if info_hash.is_some() {
                        existing.info_hash = info_hash;
                    }
                    state.tracking.dirty.insert(gid.clone());
                } else if !gid_recently_removed(state, &gid) {
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
                state.tracking.dirty.insert(gid);
            }
            EngineEvent::TorrentAdded { gid, path } => {
                state.tracking.torrent_files.insert(gid, path);
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
                state.tracking.synced_gids.insert(gid.clone());
                let was_completed = state
                    .tasks
                    .get(&gid)
                    .map(|t| t.status == TaskStatus::Completed)
                    .unwrap_or(false);
                if status == "complete"
                    && state.settings.delete_torrent_after_complete
                    && state.tracking.torrent_files.contains_key(&gid)
                {
                    if let Some(path) = state.tracking.torrent_files.remove(&gid) {
                        let _ = std::fs::remove_file(&path);
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
                    apply_task_name(&state.db, &gid, t, name);
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
                    if state.tracking.paused_gids.contains(&gid) {
                        t.status = TaskStatus::Paused;
                    }
                    if t.status == TaskStatus::Paused {
                        t.speed = 0;
                        t.upload_speed = 0;
                    }
                    if was_active != (t.status == TaskStatus::Active) {
                        if t.status == TaskStatus::Active {
                            state.tracking.active_count += 1;
                        } else {
                            state.tracking.active_count =
                                state.tracking.active_count.saturating_sub(1);
                        }
                    }
                    state.tracking.dirty.insert(gid.clone());
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
                                tracing::info!(
                                    ?gid,
                                    "ui: auto-adding downloaded torrent as new task"
                                );
                            } else {
                                state.tracking.torrent_followed.remove(&gid);
                            }
                        }
                    }
                }
                if was_completed && status != "complete" {
                    state.tracking.completion_toasted.remove(&gid);
                }
                if status == "complete" && !was_completed {
                    if let Some(t) = state.tasks.get(&gid) {
                        let name = t.name.clone();
                        if state.tracking.completion_toasted.insert(gid.clone()) {
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
                            return Task::none();
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
                spawn_toast(
                    state,
                    ToastGroup::Task,
                    ToastKind::Warning,
                    state.fluent.get(crate::i18n::Tr::SelectFilesFailed),
                    Some(Duration::from_secs(4)),
                    false,
                );
                return Task::none();
            }
            EngineEvent::Aria2Version { version } => {
                tracing::info!(?version, "aria2 version received");
                state.engine_ui.aria2_version = Some(version.clone());
                state.engine_ui.aria2_check_msg = None;
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
            EngineEvent::Aria2CheckResult { current } => {
                state.engine_ui.aria2_check_msg = Some(format!(
                    "{} v{current}",
                    state.fluent.get(crate::i18n::Tr::UpToDate)
                ));
            }
            EngineEvent::Aria2UpdateApplied { version } => {
                state.engine_ui.update_pending = None;
                state.engine_ui.aria2_version = Some(version.clone());
                state.engine_ui.aria2_check_msg = Some(format!(
                    "{} v{version}",
                    state.fluent.get(crate::i18n::Tr::UpdatedTo)
                ));
            }
            EngineEvent::Aria2UpdateFailed { error } => {
                state.engine_ui.aria2_check_msg = Some(error);
            }
            EngineEvent::Aria2UpdateStaged { version } => {
                state.engine_ui.update_pending = Some(version);
                state.engine_ui.aria2_check_msg = None;
            }
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
                return Task::none();
            }
            EngineEvent::EngineDegraded { reason } => {
                state.engine_ui.aria2_fetch_error = Some(reason);
                if state.restart.engine_restart_in_progress {
                    state.restart.engine_restart_in_progress = false;
                    state.restart.restart_resume_gids.clear();
                }
            }
            EngineEvent::GlobalSpeed { download, upload } => {
                state.global_speed = Some((download, upload));
            }
            EngineEvent::Aria2Status { stage, message } => {
                if stage == "ready" {
                    state.engine_ui.aria2_fetch_error = None;
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
            }
        },
        Message::Window(WindowMsg::WindowResized(size)) => {
            state.window.last_resize = Some(size);
            state.window.geometry_dirty = true;
        }
        Message::Window(WindowMsg::WindowOpened(id)) => {
            if state.window.window_id.is_none() {
                state.window.window_id = Some(id);
                return read_clipboard(state);
            }
            return Task::none();
        }
        Message::Window(WindowMsg::WindowFocused(id)) => {
            if state.window.window_id.is_none() || state.window.window_id == Some(id) {
                return read_clipboard(state);
            }
            return Task::none();
        }
        Message::Window(WindowMsg::ClipboardRead(content)) => {
            let Some(text) = content else {
                return Task::none();
            };
            let trimmed = text.trim().to_string();
            let prefs = state.settings.clipboard_types;
            return Task::perform(
                async move {
                    let payload = crate::clipboard_watch::parse_clipboard(&trimmed, prefs);
                    let hash = crate::clipboard_watch::payload_hash(&payload);
                    (payload, hash)
                },
                |(payload, hash)| Message::Window(WindowMsg::ClipboardParsed(payload, hash)),
            );
        }
        Message::Window(WindowMsg::ClipboardParsed(payload, hash)) => {
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
            spawn_toast(
                state,
                ToastGroup::Task,
                ToastKind::Normal,
                state.fluent.get(Tr::ClipboardDetected),
                Some(Duration::from_secs(3)),
                false,
            );
            return Task::none();
        }
        Message::Window(WindowMsg::DragWindow) => {
            if let Some(id) = state.window.window_id {
                return iced::window::drag::<Message>(id);
            }
        }
        Message::Window(WindowMsg::ResizeWindow(direction)) => {
            if let Some(id) = state.window.window_id {
                return iced::window::drag_resize::<Message>(id, direction);
            }
        }
        Message::Window(WindowMsg::WindowAction(cmd)) => {
            if let Some(id) = state.window.window_id {
                return match cmd {
                    WindowCmd::Minimize => iced::window::minimize::<Message>(id, true),
                    WindowCmd::ToggleMaximize => {
                        state.window.maximized = !state.window.maximized;
                        iced::window::toggle_maximize::<Message>(id)
                    }
                };
            }
        }
        Message::Window(WindowMsg::CloseRequested) => {
            if state.window.closing {
                return Task::none();
            }
            state.window.show_close_dialog = true;
        }
        Message::Window(WindowMsg::CloseDialog(choice)) => {
            state.window.show_close_dialog = false;
            return match choice {
                CloseDialogChoice::Close => begin_close(state),
                CloseDialogChoice::Cancel => Task::none(),
            };
        }
        Message::Window(WindowMsg::ShutdownRequested) => {
            return begin_close(state);
        }
        Message::Window(WindowMsg::ShutdownTimeout) => {
            if state.window.closing {
                tracing::warn!("engine did not stop in time, closing anyway");
                if state.handle.cmd_tx.send(EngineCmd::ForceKill).is_err() {
                    tracing::warn!("force-kill cmd send failed");
                }
            }
            return finalize_close(state);
        }
        Message::Window(WindowMsg::PersistWindowGeometry) => {
            if state.window.geometry_dirty {
                if let Some(id) = state.window.window_id {
                    return iced::window::is_maximized(id)
                        .then(|max| Task::done(Message::Window(WindowMsg::WindowMaximized(max))));
                }
            }
        }
        Message::Window(WindowMsg::WindowMaximized(max)) => {
            state.window.maximized = max;
            if let Some(s) = state.window.last_resize {
                if !max {
                    state.window.window_size = s;
                }
                state.window.last_resize = None;
            }
            sync_geometry_to_settings(state);
            config::save(&state.settings);
            state.window.geometry_dirty = false;
            if state.window.pending_close {
                state.window.pending_close = false;
                spawn_restart_if_pending(state);
                if let Some(id) = state.window.window_id {
                    return iced::window::close::<Message>(id);
                }
            }
        }
        Message::Settings(SettingsMsg::ThemeModeChanged(mode)) => {
            state.settings.theme_mode = mode;
            rebuild_theme(state);
            config::save(&state.settings);
            state.applied_settings.theme_mode = mode;
        }
        Message::Settings(SettingsMsg::ThemeColorChanged(color)) => {
            state.settings.theme_color = theme::color_to_hex(color);
            rebuild_theme(state);
            config::save(&state.settings);
            state.applied_settings.theme_color = state.settings.theme_color.clone();
        }
        Message::Settings(SettingsMsg::LocaleChanged(locale)) => {
            state.settings.locale = locale;
            state.fluent = Fluent::new(locale);
            config::save(&state.settings);
            state.applied_settings.locale = locale;
        }
        Message::Settings(SettingsMsg::FontFamilyChanged(family)) => {
            state.settings.font_family = family;
        }
        Message::Settings(SettingsMsg::RestartApp) => {
            state.restart_pending = true;
            return begin_close(state);
        }
        Message::Settings(SettingsMsg::SpeedUnitChanged(key, unit)) => {
            state.settings_ui.speed_units.insert(key, unit);
        }
        Message::Settings(SettingsMsg::UaEditor(action)) => {
            state.ua_editor.perform(action);
            state.settings.aria2.user_agent = state.ua_editor.text();
        }
        Message::Settings(SettingsMsg::BtTrackerEditor(action)) => {
            state.bt_tracker_editor.perform(action);
            state.settings.aria2.bt_tracker = state.bt_tracker_editor.text();
        }
        Message::Settings(SettingsMsg::TrackerSourceToggled { source, enabled }) => {
            if enabled {
                if !state.settings.tracker.sources.contains(&source) {
                    state.settings.tracker.sources.push(source);
                }
            } else {
                state.settings.tracker.sources.retain(|s| s != &source);
            }
        }
        Message::Settings(SettingsMsg::TrackerCustomInputChanged(v)) => {
            state.settings_ui.custom_tracker_input = v;
        }
        Message::Settings(SettingsMsg::TrackerCustomAdd) => {
            let input = state.settings_ui.custom_tracker_input.trim().to_string();
            if input.is_empty() {
                return Task::none();
            }
            let is_http = input.starts_with("http://") || input.starts_with("https://");
            if !is_http || reqwest::Url::parse(&input).is_err() {
                let toast = Toast::new(
                    ToastKind::Warning,
                    state.fluent.get(Tr::BtTrackerSourceInvalidUrl),
                )
                .group(ToastGroup::Tracker)
                .close_after(Some(Duration::from_secs(4)));
                state.toasts.push(toast);
                return Task::none();
            }
            if !state.settings.tracker.custom_urls.contains(&input) {
                state.settings.tracker.custom_urls.push(input.clone());
            }
            if !state.settings.tracker.sources.contains(&input) {
                state.settings.tracker.sources.push(input);
            }
            state.settings_ui.custom_tracker_input.clear();
        }
        Message::Settings(SettingsMsg::TrackerCustomRemove(url)) => {
            state.settings.tracker.custom_urls.retain(|u| u != &url);
            state.settings.tracker.sources.retain(|u| u != &url);
        }
        Message::Settings(SettingsMsg::SyncTrackers) => {
            if state.settings_ui.syncing_trackers {
                return Task::none();
            }
            let urls = state.settings.tracker.sources.clone();
            if urls.is_empty() {
                let toast = Toast::new(
                    ToastKind::Warning,
                    state.fluent.get(Tr::BtTrackerSelectSource),
                )
                .group(ToastGroup::Tracker)
                .close_after(Some(Duration::from_secs(4)));
                state.toasts.push(toast);
                return Task::none();
            }
            return start_tracker_fetch(state, urls);
        }
        Message::Settings(SettingsMsg::TrackersSynced { fetched, failures }) => {
            if !state.settings_ui.syncing_trackers {
                if let Some(id) = state.settings_ui.tracker_sync_toast_id.take() {
                    dismiss_toast(state, id);
                }
                return Task::none();
            }
            state.settings_ui.syncing_trackers = false;
            if let Some(id) = state.settings_ui.tracker_sync_toast_id.take() {
                dismiss_toast(state, id);
            }
            let ok = fetched.len();
            let failed = failures.len();
            let total = ok + failed;
            let mut lines: Vec<String> = Vec::new();
            let mut seen = HashSet::new();
            for body in &fetched {
                for line in crate::trackers::parse_lines(body) {
                    if seen.insert(line.clone()) {
                        lines.push(line);
                    }
                }
            }
            if lines.is_empty() && !failures.is_empty() {
                let toast = Toast::new(ToastKind::Error, state.fluent.get(Tr::BtTrackerSyncFailed))
                    .group(ToastGroup::Tracker)
                    .close_after(Some(Duration::from_secs(5)));
                state.toasts.push(toast);
                return Task::none();
            }
            let text = crate::trackers::to_lines(&lines.join("\n"));
            let count = crate::trackers::count(&text);
            state.bt_tracker_editor = text_editor::Content::with_text(&text);
            state.settings.aria2.bt_tracker = text;
            state.applied_settings.aria2.bt_tracker = state.settings.aria2.bt_tracker.clone();
            let now_ms = chrono::Local::now().timestamp_millis();
            state.settings.tracker.last_sync_time = Some(now_ms);
            state.applied_settings.tracker.last_sync_time = Some(now_ms);
            config::save(&state.applied_settings);
            let opts = state.settings.effective_task_options();
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::ApplyAria2Options { options: opts })
                .is_err()
            {
                tracing::warn!("ui: apply aria2 options cmd send failed");
            }
            let msg = if failures.is_empty() {
                let mut args = std::collections::HashMap::new();
                args.insert(std::borrow::Cow::from("count"), (count as i64).into());
                state.fluent.get_args(Tr::BtTrackerSyncSucceed, &args)
            } else {
                let mut args = std::collections::HashMap::new();
                args.insert(std::borrow::Cow::from("ok"), (ok as i64).into());
                args.insert(std::borrow::Cow::from("total"), (total as i64).into());
                args.insert(std::borrow::Cow::from("failed"), (failed as i64).into());
                state.fluent.get_args(Tr::BtTrackerSyncPartial, &args)
            };
            let toast = Toast::new(
                if failures.is_empty() {
                    ToastKind::Success
                } else {
                    ToastKind::Warning
                },
                msg,
            )
            .group(ToastGroup::Tracker)
            .close_after(Some(Duration::from_secs(4)));
            state.toasts.push(toast);
        }
        Message::Settings(SettingsMsg::TrackerSyncTimedOut) => {
            if !state.settings_ui.syncing_trackers {
                return Task::none();
            }
            state.settings_ui.syncing_trackers = false;
            if let Some(id) = state.settings_ui.tracker_sync_toast_id.take() {
                dismiss_toast(state, id);
            }
            let toast = Toast::new(ToastKind::Error, state.fluent.get(Tr::BtTrackerSyncTimeout))
                .group(ToastGroup::Tracker)
                .close_after(Some(Duration::from_secs(5)));
            state.toasts.push(toast);
        }
        Message::Settings(SettingsMsg::CheckTrackerAutoSync { startup }) => {
            if state.settings_ui.syncing_trackers {
                return Task::none();
            }
            if state.settings.aria2.bt_tracker != state.applied_settings.aria2.bt_tracker {
                return Task::none();
            }
            let now_ms = chrono::Local::now().timestamp_millis();
            if !crate::trackers::sync_due(
                state.settings.tracker.auto_sync,
                state.settings.tracker.sync_interval_hours,
                state.settings.tracker.last_sync_time,
                startup,
                now_ms,
            ) {
                return Task::none();
            }
            let urls = state.settings.tracker.sources.clone();
            if urls.is_empty() {
                return Task::none();
            }
            return start_tracker_fetch(state, urls);
        }
        Message::Engine(EngineMsg::CheckAria2Update) => {
            state.engine_ui.aria2_check_msg = None;
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::CheckAria2Update)
                .is_err()
            {
                tracing::warn!("check update cmd send failed");
            }
        }
        Message::Engine(EngineMsg::RetryAria2Fetch) => {
            state.engine_ui.aria2_fetch_error = None;
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::RetryAria2Fetch)
                .is_err()
            {
                tracing::warn!("retry fetch cmd send failed");
            }
        }
        Message::Engine(EngineMsg::RestartEngine) => {
            if state.restart.engine_restart_in_progress {
                return Task::none();
            }
            let has_active = state.tasks.values().any(|t| t.status == TaskStatus::Active);
            state.confirm = Some(ConfirmAction::RestartEngine { has_active });
        }
        Message::Engine(EngineMsg::ConfirmRestartEngine) => {
            state.confirm = None;
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
            return engine_restart_safety_timeout_task();
        }
        Message::Engine(EngineMsg::EngineRestartCooldownFinished) => {
            state.restart.engine_restart_in_progress = false;
            state.restart.restart_resume_gids.clear();
        }
        Message::Engine(EngineMsg::EngineRestartSafetyTimeout) => {
            if state.restart.engine_restart_in_progress {
                state.restart.engine_restart_in_progress = false;
                state.restart.restart_resume_gids.clear();
            }
        }
        Message::Settings(SettingsMsg::SetAutoCheck(enabled)) => {
            state.settings.update.set_ignored("aria2-next", !enabled);
            config::save(&state.settings);
        }
        Message::Settings(SettingsMsg::ToggleScheduleDaysMenu) => {
            state.settings_ui.schedule_days_menu_open = !state.settings_ui.schedule_days_menu_open;
        }
        Message::Settings(SettingsMsg::ScheduleDayToggled { day, enabled }) => {
            let weekdays = &mut state.settings.speed_limit_schedule.weekdays;
            if enabled {
                if !weekdays.contains(&day) {
                    weekdays.push(day);
                    weekdays.sort_unstable();
                }
            } else {
                weekdays.retain(|d| *d != day);
            }
        }
        Message::Task(TaskMsg::OpenTaskDetails(gid)) => {
            state.details.select_gen = 0;
            state.details.pending_select = None;
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
        Message::Task(TaskMsg::CloseTaskDetails) => {
            state.details.select_gen += 1;
            if let Some((gid, files)) = state.details.pending_select.take() {
                let _ = state
                    .handle
                    .cmd_tx
                    .send(EngineCmd::SelectFiles { gid, files });
            }
            state.details.close();
        }
        Message::Task(TaskMsg::RefreshTaskDetails) => {
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
        Message::Window(WindowMsg::FlushDirty) => {
            flush_dirty(state);
        }
        Message::Nav(NavMsg::SelectDetailsTab(tab)) => {
            state.details.active_tab = tab;
        }
        Message::Task(TaskMsg::DetailsTreeExpand(path)) => {
            if state.details.files_expanded.contains(&path) {
                state.details.files_expanded.remove(&path);
            } else {
                state.details.files_expanded.insert(path);
            }
        }
        Message::Task(TaskMsg::DetailsTreeToggle(path)) => {
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
        Message::Task(TaskMsg::DetailsFilesSelectAll) => {
            if let Some(ref mut details) = state.details.details {
                for file in &mut details.files {
                    file.selected = true;
                }
            }
            return schedule_details_select_flush(state);
        }
        Message::Task(TaskMsg::DetailsFilesSelectNone) => {
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
        Message::Task(TaskMsg::DetailsFilesScroll(off)) => {
            state.details.files_scroll_offset = off;
        }
        Message::Task(TaskMsg::DetailsFilesFlush(gen)) => {
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
        }
        Message::Task(TaskMsg::OpenTaskFile(gid)) => {
            let Some(t) = state.tasks.get(&gid).cloned() else {
                return Task::none();
            };
            let path = t.save_dir.join(&t.name);
            if path.exists()
                && (crate::engine::is_torrent_url(&t.name) || t.name.starts_with("[METADATA]"))
            {
                let default_dir = if t.save_dir.as_os_str().is_empty() {
                    state.settings.download_dir.clone()
                } else {
                    t.save_dir.clone()
                };
                state.add_dialog.save_picker.close_history();
                state.add_dialog.open(default_dir, state.settings.split);
                state
                    .add_dialog
                    .set_torrent_path(path.to_string_lossy().to_string());
                state.add_dialog.active_tab = AddTab::Torrent;
                return Task::none();
            }
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
            if path.exists() {
                return Task::perform(
                    async move {
                        let _ = open::that(&path);
                    },
                    |_| Message::Noop,
                );
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
            return Task::none();
        }
        Message::Task(TaskMsg::OpenTaskFolder(gid)) => {
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
        Message::Task(TaskMsg::CopyTaskLink(gid)) => {
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
        Message::Dialog(DialogMsg::RequestConfirm(action)) => {
            state.confirm = Some(action);
        }
        Message::Dialog(DialogMsg::ConfirmCancel) => {
            state.confirm = None;
        }
        Message::Settings(SettingsMsg::ApplyAndLeaveSettings) => {
            if let Some(ConfirmAction::LeaveSettings { target }) = state.confirm.take() {
                apply_settings(state);
                state.page = target;
            }
        }
        Message::Settings(SettingsMsg::DiscardAndLeaveSettings) => {
            if let Some(ConfirmAction::LeaveSettings { target }) = state.confirm.take() {
                revert_apply_settings(state);
                config::save(&state.settings);
                state.page = target;
            }
        }
        Message::Toast(ToastMsg::DismissToast(id)) => {
            dismiss_toast(state, id);
        }
        Message::Toast(ToastMsg::ToastHovered(id)) => {
            state.toasts.hover(id);
        }
        Message::Toast(ToastMsg::ToastUnhovered(id)) => {
            state.toasts.unhover(id);
        }
        Message::Toast(ToastMsg::ToastTick) => {
            state.toasts.tick();
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
    let titlebar = crate::ui::title_bar::view(t, state.window.maximized);
    let left_col = crate::ui::sidebar::view(&state.fluent, t, state.page);

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
            let filtered: Vec<&DownloadTask> = state
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
        Page::Settings => {
            let ctx = crate::ui::settings_page::SettingsPageContext {
                fluent: &state.fluent,
                theme: t,
                settings: &state.settings,
                settings_ui: &state.settings_ui,
                category: state.settings_cat,
                applied_settings: &state.applied_settings,
                engine_restart_pending: state.restart.engine_restart_pending,
                engine_restart_in_progress: state.restart.engine_restart_in_progress,
                aria2_version: state.engine_ui.aria2_version.as_deref(),
                aria2_check_msg: state.engine_ui.aria2_check_msg.as_deref(),
                aria2_status: state
                    .engine_ui
                    .aria2_status
                    .as_ref()
                    .map(|(s, m)| (s.as_str(), m.as_str())),
                aria2_fetch_error: state.engine_ui.aria2_fetch_error.as_deref(),
                update_pending: state.engine_ui.update_pending.as_deref(),
                ua_editor: &state.ua_editor,
                bt_tracker_editor: &state.bt_tracker_editor,
                path_history: &state.settings.path_history,
                font_restart_required: state.settings.font_family != state.applied_font_family,
            };
            crate::ui::settings_page::view(&ctx)
        }
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

    let framed: iced::Element<'_, Message> = if state.window.maximized {
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
    let (dl, up) = if state.tracking.active_count > 0 {
        state.global_speed.unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    let hud_overlay = container(crate::ui::components::speed_hud::view(
        t,
        state.tracking.active_count > 0,
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
        crate::ui::about_dialog::view(&state.fluent, t, state.engine_ui.aria2_version.as_deref())
    } else {
        iced::widget::Space::new().into()
    };

    let close_layer: iced::Element<'_, Message> = if state.window.show_close_dialog {
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

    let drop_overlay_layer: iced::Element<'_, Message> = if state.drop_hover
        && !(state.window.show_close_dialog
            || state.about_dialog_visible
            || state.confirm.is_some())
    {
        crate::ui::components::drop_overlay::view(&state.fluent, t)
    } else {
        iced::widget::Space::new().into()
    };

    let toast_layer: iced::Element<'_, Message> = if !state.toasts.toasts.is_empty() {
        crate::ui::components::toast::view(t, &state.toasts.toasts)
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
        drop_overlay_layer,
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
                    let _ = sender.send(Message::Engine(EngineMsg::Event(ev))).await;
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
            let _ = sender
                .send(Message::Window(WindowMsg::ShutdownRequested))
                .await;
        },
    )
}

pub fn subscription(state: &Remotrix) -> Subscription<Message> {
    let engine =
        Subscription::run_with(EventSlot(state.event_rx_slot.clone()), build_engine_stream);

    let open = iced::window::open_events().map(|id| Message::Window(WindowMsg::WindowOpened(id)));
    let close =
        iced::window::close_requests().map(|_id| Message::Window(WindowMsg::CloseRequested));
    let focus = iced::event::listen_with(|event, _status, window| match event {
        iced::event::Event::Window(iced::window::Event::Focused) => {
            Some(Message::Window(WindowMsg::WindowFocused(window)))
        }
        _ => None,
    });

    let files = iced::event::listen_with(|event, _status, _window| match event {
        iced::event::Event::Window(iced::window::Event::FileHovered(_path)) => {
            Some(Message::Add(AddMsg::FileHovered))
        }
        iced::event::Event::Window(iced::window::Event::FileDropped(path)) => {
            Some(Message::Add(AddMsg::FileDropped(path)))
        }
        iced::event::Event::Window(iced::window::Event::FilesHoveredLeft) => {
            Some(Message::Add(AddMsg::FilesHoveredLeft))
        }
        _ => None,
    });

    let flush = iced::time::every(Duration::from_millis(1000))
        .map(|_| Message::Window(WindowMsg::FlushDirty));

    let resizes = iced::window::resize_events()
        .map(|(_id, size)| Message::Window(WindowMsg::WindowResized(size)));
    let persist_periodic = iced::time::every(Duration::from_millis(2000))
        .map(|_| Message::Window(WindowMsg::PersistWindowGeometry));

    let refresh = if state.details.is_visible() {
        iced::time::every(Duration::from_millis(2000))
            .map(|_| Message::Task(TaskMsg::RefreshTaskDetails))
    } else {
        Subscription::none()
    };

    let toast_tick = if state.toasts.toasts.iter().any(|t| t.remaining.is_some()) {
        iced::time::every(Duration::from_millis(200)).map(|_| Message::Toast(ToastMsg::ToastTick))
    } else {
        Subscription::none()
    };

    let signals = Subscription::run_with((), |_| signal_stream());

    let tracker_auto_sync = if state.settings.tracker.auto_sync {
        iced::time::every(Duration::from_secs(3600))
            .map(|_| Message::Settings(SettingsMsg::CheckTrackerAutoSync { startup: false }))
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
        tracker_auto_sync,
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
                let toast = Toast::new(ToastKind::Warning, state.fluent.get(Tr::InvalidTorrent))
                    .group(ToastGroup::Task)
                    .close_after(Some(Duration::from_secs(4)));
                state.toasts.push(toast);
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
    state.details.pending_select = Some((gid, selected));
    state.details.select_gen += 1;
    let gen = state.details.select_gen;
    Task::perform(
        async move {
            tokio::time::sleep(Duration::from_millis(350)).await;
            gen
        },
        |n| Message::Task(TaskMsg::DetailsFilesFlush(n)),
    )
}

fn spawn_toast(
    state: &mut Remotrix,
    group: ToastGroup,
    kind: ToastKind,
    message: String,
    close_after: Option<Duration>,
    show_close: bool,
) -> u64 {
    state
        .toasts
        .spawn(group, kind, message, close_after, show_close)
}

fn dismiss_toast(state: &mut Remotrix, id: u64) {
    state.toasts.dismiss(id);
}

fn start_tracker_fetch(state: &mut Remotrix, urls: Vec<String>) -> Task<Message> {
    state.settings_ui.syncing_trackers = true;
    let id = spawn_toast(
        state,
        ToastGroup::Tracker,
        ToastKind::Normal,
        state.fluent.get(Tr::BtTrackerSyncing),
        None,
        true,
    );
    state.settings_ui.tracker_sync_toast_id = Some(id);
    let fetch = Task::perform(
        async move { crate::trackers::fetch_sources(&urls).await },
        |(fetched, failures)| Message::Settings(SettingsMsg::TrackersSynced { fetched, failures }),
    );
    const SYNC_TIMEOUT: Duration = Duration::from_secs(30);
    let timeout = Task::perform(
        async move {
            tokio::time::sleep(SYNC_TIMEOUT).await;
        },
        |_| Message::Settings(SettingsMsg::TrackerSyncTimedOut),
    );
    Task::batch([fetch, timeout])
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
    Task::perform(task, move |maybe| {
        Message::Add(AddMsg::PathPicked(id, maybe))
    })
}
