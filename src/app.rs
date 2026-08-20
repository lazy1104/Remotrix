use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iced::alignment::{Horizontal, Vertical};
use iced::futures::SinkExt;
use iced::widget::{column, container, float, mouse_area, row, stack, text_editor};
use iced::window::Id;
use iced::{Element, Length, Padding, Subscription, Task, Vector};

use crate::config::{self, Settings};
use crate::db::Db;
use crate::engine::{EngineCmd, EngineHandle, EventRx};
use crate::i18n::{Fluent, Tr};
use crate::message::{
    AddField, AddMsg, ConfirmAction, CtxTarget, EngineMsg, ExtensionMsg, Message, Page,
    PathPickerId, SettingsCategory, SettingsMsg, SortField, SortMsg, SortOrder, TaskFilter,
    TaskMsg, ToastMsg, WindowMsg,
};
use crate::task::{DownloadTask, TaskStatus};
use crate::ui::add_dialog::AddDialogState;
use crate::ui::category_bar::Counts;
use crate::ui::components::ctx_menu::{self, CtxCursor, CtxMirrors};
use crate::ui::components::file_tree::FileTreeNode;
use crate::ui::components::toast::{Toast, ToastGroup, ToastKind};
use crate::ui::components::torrent_upload::{self};
use crate::ui::details_dialog::DetailsDialogState;
use crate::ui::icons::{CATEGORY_W, SIDEBAR_W};
use crate::ui::settings_page::SettingsUiState;
use crate::ui::theme;

pub(crate) struct ToastManager {
    pub(crate) toasts: Vec<crate::ui::components::toast::Toast>,
    pub(crate) next_toast_id: u64,
    pub(crate) hovered_toast_id: Option<u64>,
}

impl ToastManager {
    pub(crate) fn new() -> Self {
        Self {
            toasts: Vec::new(),
            next_toast_id: 0,
            hovered_toast_id: None,
        }
    }

    pub(crate) fn push(&mut self, mut toast: crate::ui::components::toast::Toast) -> u64 {
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

    pub(crate) fn spawn(
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

    pub(crate) fn hover(&mut self, id: u64) {
        self.hovered_toast_id = Some(id);
    }

    pub(crate) fn unhover(&mut self, id: u64) {
        if self.hovered_toast_id == Some(id) {
            self.hovered_toast_id = None;
        }
    }

    pub(crate) fn tick(&mut self) {
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

pub(crate) struct EngineUiState {
    pub(crate) aria2_version: Option<String>,
    pub(crate) update_check_in_flight: bool,
    pub(crate) aria2_status: Option<(String, String)>,
    pub(crate) aria2_fetch_error: Option<String>,
    pub(crate) downloading_toast_id: Option<u64>,
    pub(crate) startup_error_toast_id: Option<u64>,
    pub(crate) startup_starting_toast_shown: bool,
    pub(crate) degraded_notified: bool,
    pub(crate) aria2_downloading: bool,
    pub(crate) aria2_downloading_version: Option<String>,
    pub(crate) aria2_download_progress: Option<(u64, u64)>,
    pub(crate) aria2_download_silent: bool,
}

impl EngineUiState {
    pub(crate) fn new() -> Self {
        Self {
            aria2_version: None,
            update_check_in_flight: false,
            aria2_status: None,
            aria2_fetch_error: None,
            downloading_toast_id: None,
            startup_error_toast_id: None,
            startup_starting_toast_shown: false,
            degraded_notified: false,
            aria2_downloading: false,
            aria2_downloading_version: None,
            aria2_download_progress: None,
            aria2_download_silent: false,
        }
    }
}

pub(crate) struct UpdateDialogState {
    pub(crate) offers: Vec<crate::ui::update_dialog::UpdateOffer>,
    pub(crate) changelogs: Vec<crate::ui::update_dialog::ChangelogState>,
    pub(crate) active_tab: usize,
}

pub(crate) struct WindowState {
    pub(crate) maximized: bool,
    pub(crate) show_close_dialog: bool,
    pub(crate) close_dialog_anim: Option<crate::ui::animation::Animated<f32>>,
    pub(crate) close_dialog_dismissing: bool,
    pub(crate) window_id: Option<Id>,
    pub(crate) window_size: iced::Size,
    pub(crate) last_resize: Option<iced::Size>,
    pub(crate) geometry_dirty: bool,
    pub(crate) pending_close: bool,
    pub(crate) closing: bool,
    pub(crate) hidden_to_tray: bool,
    pub(crate) wayland: bool,
    #[cfg(target_os = "windows")]
    pub(crate) resizing: bool,
    #[cfg(target_os = "windows")]
    pub(crate) resize_quiet: Option<Instant>,
}

impl WindowState {
    pub(crate) fn new(window_size: iced::Size, maximized: bool) -> Self {
        Self {
            maximized,
            show_close_dialog: false,
            close_dialog_anim: None,
            close_dialog_dismissing: false,
            window_id: None,
            window_size,
            last_resize: None,
            geometry_dirty: false,
            pending_close: false,
            closing: false,
            hidden_to_tray: false,
            wayland: is_wayland(),
            #[cfg(target_os = "windows")]
            resizing: false,
            #[cfg(target_os = "windows")]
            resize_quiet: None,
        }
    }
}

fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|t| t.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
        || std::env::var("WAYLAND_DISPLAY").is_ok()
}

pub(crate) struct EngineRestartState {
    pub(crate) engine_restart_pending: bool,
    pub(crate) engine_restart_in_progress: bool,
    pub(crate) restart_resume_gids: HashSet<String>,
}

impl EngineRestartState {
    pub(crate) fn new() -> Self {
        Self {
            engine_restart_pending: false,
            engine_restart_in_progress: false,
            restart_resume_gids: HashSet::new(),
        }
    }
}

pub(crate) struct TaskTracking {
    pub(crate) paused_gids: HashSet<String>,
    pub(crate) synced_gids: HashSet<String>,
    pub(crate) removed_gids: HashMap<String, Instant>,
    pub(crate) sync_done: bool,
    pub(crate) active_count: usize,
    pub(crate) dirty: HashSet<String>,
    pub(crate) completion_toasted: HashSet<String>,
    pub(crate) error_notified: HashSet<String>,
    pub(crate) torrent_files: HashMap<String, PathBuf>,
    pub(crate) torrent_followed: HashSet<String>,
}

impl TaskTracking {
    pub(crate) fn new(active_count: usize) -> Self {
        Self {
            paused_gids: HashSet::new(),
            synced_gids: HashSet::new(),
            removed_gids: HashMap::new(),
            sync_done: false,
            active_count,
            dirty: HashSet::new(),
            completion_toasted: HashSet::new(),
            error_notified: HashSet::new(),
            torrent_files: HashMap::new(),
            torrent_followed: HashSet::new(),
        }
    }
}

pub(crate) struct CtxMenuState {
    pub(crate) target: CtxTarget,
    pub(crate) position: iced::Point,
    pub(crate) clipboard: Option<String>,
}

pub struct Remotrix {
    pub(crate) page: Page,
    pub(crate) task_filter: TaskFilter,
    pub(crate) settings_cat: SettingsCategory,
    pub(crate) tasks: HashMap<String, DownloadTask>,
    pub(crate) task_order: Vec<String>,
    pub(crate) handle: EngineHandle,
    pub(crate) event_rx_slot: Arc<Mutex<Option<EventRx>>>,
    pub(crate) wake_rx_slot: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Message>>>>,
    pub(crate) _primary: app_single_instance::PrimaryHandle,
    pub(crate) notifiers: crate::notify::Notifiers,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    pub(crate) notify_tx: tokio::sync::mpsc::UnboundedSender<crate::notify::NotifyEvent>,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    pub(crate) notify_rx_slot:
        Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<crate::notify::NotifyEvent>>>>,
    pub(crate) tray: crate::tray::TrayManager,
    pub(crate) tray_rx_slot: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Message>>>>,
    pub(crate) ext_msg_rx_slot: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Message>>>>,
    pub(crate) extension_msg_tx: tokio::sync::mpsc::UnboundedSender<Message>,
    pub(crate) stat_cache: Arc<std::sync::Mutex<crate::extension_api::GlobalStatCache>>,
    pub(crate) add_dialog: AddDialogState,
    pub(crate) drop_hover: bool,
    pub(crate) about_dialog_visible: bool,
    pub(crate) settings: Settings,
    pub(crate) fluent: Fluent,
    pub(crate) theme: iced::Theme,
    pub(crate) sort_menu_open: bool,
    pub(crate) sort_field: SortField,
    pub(crate) sort_order: SortOrder,
    pub(crate) search_query: String,
    pub(crate) ua_editor: text_editor::Content,
    pub(crate) bt_tracker_editor: text_editor::Content,
    pub(crate) db: Option<Db>,
    pub(crate) details: DetailsDialogState,
    pub(crate) confirm: Option<ConfirmAction>,
    pub(crate) applied_settings: Settings,
    pub(crate) settings_dirty: bool,
    pub(crate) applied_font_family: String,
    pub(crate) restart_pending: bool,
    pub(crate) settings_ui: SettingsUiState,
    pub(crate) ctx_menu: Option<CtxMenuState>,
    pub(crate) ctx_open: Option<(CtxTarget, iced::Point)>,
    pub(crate) last_cursor: iced::Point,
    pub(crate) input_cursors: CtxMirrors,
    pub(crate) global_speed: Option<(u64, u64)>,
    pub(crate) toasts: ToastManager,
    pub(crate) engine_ui: EngineUiState,
    pub(crate) window: WindowState,
    pub(crate) restart: EngineRestartState,
    pub(crate) tracking: TaskTracking,
    pub(crate) sleep_block_active: bool,
    pub(crate) update_dialog: Option<UpdateDialogState>,
    pub(crate) app_update_in_flight: bool,
    pub(crate) progress_anim: HashMap<String, crate::ui::animation::Animated<f32>>,
    pub(crate) card_anim: HashMap<String, crate::ui::animation::Animated<f32>>,
    pub(crate) pending_removals: HashSet<String>,
    pub(crate) filter_pill: crate::ui::animation::Animated<f32>,
    pub(crate) hud_anim: crate::ui::animation::Animated<f32>,
    pub(crate) add_dialog_anim: crate::ui::animation::DialogAnim,
    pub(crate) about_dialog_anim: crate::ui::animation::DialogAnim,
    pub(crate) details_anim: crate::ui::animation::DialogAnim,
    pub(crate) confirm_anim: crate::ui::animation::DialogAnim,
    pub(crate) update_dialog_anim: crate::ui::animation::DialogAnim,
    pub(crate) shutdown: crate::shutdown::ShutdownControl,
    pub(crate) port_status: std::collections::HashMap<
        crate::port_guard::PortKind,
        (u16, crate::port_guard::PortStatus),
    >,
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

    let (wake_tx, wake_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let _primary = app_single_instance::start_primary(crate::APP_ID, move || {
        let _ = wake_tx.send(Message::ShowRequested);
    });

    let notifiers = crate::notify::Notifiers::new();
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let (notify_tx, notify_rx) =
        tokio::sync::mpsc::unbounded_channel::<crate::notify::NotifyEvent>();
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let notify_rx_slot: Arc<
        Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<crate::notify::NotifyEvent>>>,
    > = Arc::new(Mutex::new(Some(notify_rx)));

    let (tray_tx, tray_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let tray_rx_slot: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Message>>>> =
        Arc::new(Mutex::new(Some(tray_rx)));
    let tray = crate::tray::TrayManager::new(tray_tx, true);

    let (ext_msg_tx, ext_msg_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let ext_msg_rx_slot: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Message>>>> =
        Arc::new(Mutex::new(Some(ext_msg_rx)));
    let stat_cache = Arc::new(std::sync::Mutex::new(
        crate::extension_api::GlobalStatCache::default(),
    ));

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
        wake_rx_slot: Arc::new(Mutex::new(Some(wake_rx))),
        _primary,
        notifiers,
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        notify_tx,
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        notify_rx_slot,
        tray,
        tray_rx_slot,
        ext_msg_rx_slot,
        extension_msg_tx: ext_msg_tx.clone(),
        stat_cache,
        add_dialog,
        drop_hover: false,
        about_dialog_visible: false,
        applied_settings: settings.clone(),
        settings_dirty: false,
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
        ctx_menu: None,
        ctx_open: None,
        last_cursor: iced::Point::ORIGIN,
        input_cursors: init_ctx_mirrors(),
        global_speed: None,
        toasts: ToastManager::new(),
        engine_ui: EngineUiState::new(),
        window: WindowState::new(iced::Size::new(window_w, window_h), window_maximized),
        restart: EngineRestartState::new(),
        tracking: TaskTracking::new(active_count),
        sleep_block_active: false,
        update_dialog: None,
        app_update_in_flight: false,
        progress_anim: HashMap::new(),
        card_anim: HashMap::new(),
        pending_removals: HashSet::new(),
        filter_pill: crate::ui::animation::Animated::transition(
            0.0,
            crate::ui::animation::ease_in_out_quad(crate::ui::animation::PILL_MS),
        ),
        hud_anim: crate::ui::animation::Animated::transition(
            0.0,
            crate::ui::animation::ease_out_cubic(crate::ui::animation::HUD_ANIM_MS),
        ),
        add_dialog_anim: Default::default(),
        about_dialog_anim: Default::default(),
        details_anim: Default::default(),
        confirm_anim: Default::default(),
        update_dialog_anim: Default::default(),
        shutdown: crate::shutdown::ShutdownControl {
            timer_minutes: 30,
            ..Default::default()
        },
        port_status: std::collections::HashMap::new(),
    };

    state.window.hidden_to_tray =
        crate::autostart::is_autostart_launch() && state.settings.start_hidden_on_autostart;

    let ext_on_dialog: Option<
        Box<dyn Fn(crate::extension_api::ExternalDownload) + Send + Sync + 'static>,
    > = Some(Box::new(move |download| {
        let _ = ext_msg_tx.send(Message::Extension(ExtensionMsg::ShowAddDialog(download)));
    }));
    crate::extension_api::spawn_server(
        state.handle.cmd_tx.clone(),
        state.stat_cache.clone(),
        ext_on_dialog,
        None,
    );

    refresh_tray(&mut state);

    if db_open_failed {
        state.toasts.spawn(
            ToastGroup::General,
            ToastKind::Warning,
            state.fluent.get(crate::i18n::Tr::DatabaseError),
            Some(Duration::from_secs(6)),
            true,
        );
    }

    sync_sleep_block(&mut state);

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

pub(crate) fn rebuild_theme(state: &mut Remotrix) {
    let dark = theme::resolve_mode(state.settings.theme_mode, None);
    state.theme = theme::build_iced(settings_accent(&state.settings), dark);
}

pub(crate) fn sync_geometry_to_settings(state: &mut Remotrix) {
    state.settings.window_width = state.window.window_size.width;
    state.settings.window_height = state.window.window_size.height;
    state.settings.window_maximized = state.window.maximized;
    state.applied_settings.window_width = state.window.window_size.width;
    state.applied_settings.window_height = state.window.window_size.height;
    state.applied_settings.window_maximized = state.window.maximized;
}

pub(crate) fn mark_settings_dirty(state: &mut Remotrix) {
    state.settings_dirty = state.settings != state.applied_settings;
}

pub(crate) fn revert_apply_settings(state: &mut Remotrix) {
    state.settings = state.applied_settings.clone();
    if let Err(e) = crate::autostart::set_enabled(state.settings.autostart_enabled) {
        tracing::warn!(error = %e, "autostart sync failed on revert");
    }
    state.settings_dirty = false;
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
    sync_sleep_block(state);
}

pub(crate) fn sync_sleep_block(state: &mut Remotrix) {
    let should = state.applied_settings.prevent_sleep && state.tracking.active_count > 0;
    if should == state.sleep_block_active {
        return;
    }
    crate::power::set_sleep_blocked(should);
    state.sleep_block_active = should;
}

pub(crate) fn apply_settings(state: &mut Remotrix) -> bool {
    config::save(&state.settings);
    if let Err(e) = crate::autostart::set_enabled(state.settings.autostart_enabled) {
        tracing::warn!(error = %e, "autostart sync failed");
        spawn_toast(
            state,
            ToastGroup::General,
            ToastKind::Error,
            state.fluent.get(Tr::LaunchOnStartupFailed),
            Some(Duration::from_secs(5)),
            false,
        );
    }
    crate::logging::set_app_level(&state.settings.log.app_level);
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
    let restart_needed = state
        .settings
        .aria2
        .engine_restart_needed(&state.applied_settings.aria2);
    if restart_needed && state.handle.cmd_tx.send(EngineCmd::RestartEngine).is_err() {
        tracing::warn!("ui: restart engine cmd send failed");
    }
    state.restart.engine_restart_pending = restart_needed
        || state.settings.log.engine_level != state.applied_settings.log.engine_level;
    let ext_changed = state.settings.extension != state.applied_settings.extension;
    if ext_changed {
        let msg = if state.settings.extension.enabled {
            Tr::ExtensionRestarting
        } else {
            Tr::ExtensionStopped
        };
        spawn_toast(
            state,
            ToastGroup::Engine,
            ToastKind::Normal,
            state.fluent.get(msg),
            Some(Duration::from_secs(3)),
            false,
        );
        let on_dialog: Option<
            Box<dyn Fn(crate::extension_api::ExternalDownload) + Send + Sync + 'static>,
        > = Some(Box::new({
            let tx = state.extension_msg_tx.clone();
            move |download| {
                let _ = tx.send(Message::Extension(ExtensionMsg::ShowAddDialog(download)));
            }
        }));
        let tx = state.extension_msg_tx.clone();
        let on_ready: Option<Box<dyn Fn(bool) + Send + Sync + 'static>> =
            Some(Box::new(move |ok| {
                let _ = tx.send(Message::Extension(ExtensionMsg::ServerRestarted { ok }));
            }));
        crate::extension_api::spawn_server(
            state.handle.cmd_tx.clone(),
            state.stat_cache.clone(),
            on_dialog,
            on_ready,
        );
    }
    state.applied_settings = state.settings.clone();
    state.settings_dirty = false;
    sync_sleep_block(state);
    restart_needed
}

fn count_task_stats(state: &Remotrix) -> (usize, usize, usize) {
    let mut active = 0;
    let mut waiting = 0;
    let mut stopped = 0;
    for t in state.tasks.values() {
        match t.status {
            TaskStatus::Active => active += 1,
            TaskStatus::Waiting => waiting += 1,
            TaskStatus::Paused
            | TaskStatus::Completed
            | TaskStatus::Error
            | TaskStatus::Removed => stopped += 1,
        }
    }
    (active, waiting, stopped)
}

pub(crate) fn sync_global_stat_cache(state: &Remotrix) {
    let (active, waiting, stopped) = count_task_stats(state);
    if let Ok(mut cache) = state.stat_cache.lock() {
        cache.num_active = active;
        cache.num_waiting = waiting;
        cache.num_stopped = stopped;
        cache.num_stopped_total = stopped;
    }
}

pub(crate) fn clear_all_local(state: &mut Remotrix) {
    let gids: Vec<String> = state.tasks.keys().cloned().collect();
    for gid in gids {
        begin_task_exit(state, &gid, false);
    }
    state.tracking.dirty.clear();
    state.tracking.active_count = 0;
    state.tracking.paused_gids.clear();
    if let Some(ref db) = state.db {
        db.delete_all();
    }
    sync_sleep_block(state);
}

pub(crate) fn begin_task_exit(state: &mut Remotrix, gid: &str, delete_db: bool) {
    if !state.tasks.contains_key(gid) || state.pending_removals.contains(gid) {
        return;
    }
    state.pending_removals.insert(gid.to_string());
    state
        .tracking
        .removed_gids
        .insert(gid.to_string(), Instant::now());
    match state.card_anim.get_mut(gid) {
        Some(a) => a.set_target(0.0),
        None => {
            state.card_anim.insert(
                gid.to_string(),
                crate::ui::animation::Animated::transition(
                    1.0,
                    crate::ui::animation::ease_out_quad(crate::ui::animation::CARD_EXIT_MS),
                )
                .to(0.0),
            );
        }
    }
    if delete_db {
        if let Some(ref db) = state.db {
            db.delete(gid);
        }
    }
    state.tracking.dirty.remove(gid);
}

pub(crate) fn finalize_task_removal(state: &mut Remotrix, gid: &str) {
    if let Some(t) = state.tasks.remove(gid) {
        if t.status == TaskStatus::Active {
            state.tracking.active_count = state.tracking.active_count.saturating_sub(1);
        }
    }
    sync_sleep_block(state);
    let _ = state.tracking.torrent_files.remove(gid);
    state.tracking.torrent_followed.remove(gid);
    state.tracking.completion_toasted.remove(gid);
    state.tracking.error_notified.remove(gid);
    state.tracking.paused_gids.remove(gid);
    state.progress_anim.remove(gid);
    state.task_order.retain(|g| g != gid);
    state.tracking.dirty.remove(gid);
    state.pending_removals.remove(gid);
    state.card_anim.remove(gid);
}

const REMOVED_GID_GRACE: Duration = Duration::from_secs(60);

#[cfg(target_os = "windows")]
pub(crate) const RESIZE_TICK_MS: u64 = 33;
#[cfg(target_os = "windows")]
pub(crate) const RESIZE_QUIET_MS: u64 = 150;

pub(crate) fn gid_recently_removed(state: &mut Remotrix, gid: &str) -> bool {
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

pub(crate) fn apply_task_name(
    db: &Option<Db>,
    gid: &str,
    t: &mut DownloadTask,
    incoming: String,
) -> Task<Message> {
    if !incoming.starts_with("[METADATA]") {
        if t.name != incoming {
            t.name = incoming;
            if let Some(ref db) = db {
                db.update_name(gid, &t.name, t.metadata_only);
            }
        }
        return Task::none();
    }
    t.metadata_only = true;
    let placeholder = t.name.is_empty() || t.name.starts_with("[METADATA]") || t.name == "magnet:";
    if !placeholder {
        return Task::none();
    }
    let infohash = incoming
        .strip_prefix("[METADATA]")
        .or(t.info_hash.as_deref())
        .unwrap_or(&incoming);
    let path = t.save_dir.join(format!("{infohash}.torrent"));
    let prev_size = t.metadata_probe_size;
    let gid = gid.to_string();
    Task::perform(
        async move {
            let size = std::fs::metadata(&path).ok().map(|m| m.len());
            let name = match size {
                Some(s) if Some(s) != prev_size => resolve_metadata_name(&path)
                    .map(|real| {
                        std::path::Path::new(&real)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or_default()
                            .to_string()
                    })
                    .filter(|n| !n.is_empty()),
                _ => None,
            };
            (gid, incoming, size, name)
        },
        |(gid, incoming, size, name)| {
            Message::Task(crate::message::TaskMsg::MetadataProbeResult {
                gid,
                incoming,
                size,
                name,
            })
        },
    )
}

pub(crate) fn clear_completed_local(state: &mut Remotrix, gids: &[String]) {
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
        begin_task_exit(state, gid, false);
    }
}

pub(crate) fn flush_dirty(state: &mut Remotrix) {
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

pub(crate) fn begin_close(state: &mut Remotrix) -> Task<Message> {
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
    crate::power::release();
    state.sleep_block_active = false;
    state.tray.quit();
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

pub(crate) fn shutdown_timeout_task() -> Task<Message> {
    Task::perform(
        async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
        },
        |_| Message::Window(WindowMsg::ShutdownTimeout),
    )
}

pub(crate) fn engine_restart_safety_timeout_task() -> Task<Message> {
    Task::perform(
        async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
        },
        |_| Message::Engine(EngineMsg::EngineRestartSafetyTimeout),
    )
}

pub(crate) fn engine_restart_cooldown_task() -> Task<Message> {
    Task::perform(
        async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
        },
        |_| Message::Engine(EngineMsg::EngineRestartCooldownFinished),
    )
}

pub(crate) fn finalize_close(state: &mut Remotrix) -> Task<Message> {
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

pub(crate) fn spawn_restart_if_pending(state: &mut Remotrix) {
    if state.restart_pending {
        state.restart_pending = false;
        crate::app_updater::relaunch_after_update();
    }
}

pub(crate) fn read_clipboard(state: &Remotrix) -> Task<Message> {
    if !state.settings.detect_clipboard_on_start {
        return Task::none();
    }
    iced::clipboard::read().map(|content| Message::Window(WindowMsg::ClipboardRead(content)))
}

fn init_ctx_mirrors() -> CtxMirrors {
    let mut mirrors: CtxMirrors = HashMap::new();
    for target in [
        CtxTarget::Search,
        CtxTarget::AddOut,
        CtxTarget::SettingsCustomTracker,
    ] {
        mirrors.insert(target, Rc::new(RefCell::new(CtxCursor::default())));
    }
    for field in [
        AddField::Out,
        AddField::UserAgent,
        AddField::HttpUser,
        AddField::HttpPasswd,
        AddField::Referer,
        AddField::Cookie,
        AddField::ProxyServer,
        AddField::ProxyUsername,
        AddField::ProxyPassword,
    ] {
        mirrors.insert(
            CtxTarget::AddAdvanced(field),
            Rc::new(RefCell::new(CtxCursor::default())),
        );
        mirrors.insert(
            CtxTarget::DetailsAdvanced(field),
            Rc::new(RefCell::new(CtxCursor::default())),
        );
    }
    mirrors
}

pub(crate) fn ctx_value(state: &Remotrix, target: CtxTarget) -> &str {
    match target {
        CtxTarget::Search => &state.search_query,
        CtxTarget::AddOut => &state.add_dialog.out,
        CtxTarget::AddAdvanced(field) => match field {
            AddField::Out => &state.add_dialog.out,
            AddField::UserAgent => &state.add_dialog.user_agent,
            AddField::HttpUser => &state.add_dialog.http_user,
            AddField::HttpPasswd => &state.add_dialog.http_passwd,
            AddField::Referer => &state.add_dialog.referer,
            AddField::Cookie => &state.add_dialog.cookie,
            AddField::ProxyServer => &state.add_dialog.proxy_server,
            AddField::ProxyUsername => &state.add_dialog.proxy_username,
            AddField::ProxyPassword => &state.add_dialog.proxy_password,
        },
        CtxTarget::DetailsAdvanced(field) => match field {
            AddField::UserAgent => &state.details.user_agent,
            AddField::HttpUser => &state.details.http_user,
            AddField::HttpPasswd => &state.details.http_passwd,
            AddField::Referer => &state.details.referer,
            AddField::Cookie => &state.details.cookie,
            AddField::ProxyServer => &state.details.proxy_server,
            AddField::ProxyUsername => &state.details.proxy_username,
            AddField::ProxyPassword => &state.details.proxy_password,
            AddField::Out => "",
        },
        CtxTarget::SettingsCustomTracker => &state.settings_ui.custom_tracker_input,
        CtxTarget::AddUrl | CtxTarget::SettingsUa | CtxTarget::SettingsBtTracker => "",
    }
}

pub(crate) fn ctx_cur(state: &Remotrix, target: CtxTarget) -> Option<CtxCursor> {
    state.input_cursors.get(&target).map(|c| *c.borrow())
}

pub(crate) fn ctx_paste_message(state: &Remotrix, target: CtxTarget, text: String) -> Message {
    match target {
        CtxTarget::AddUrl => Message::Add(AddMsg::UrlEditor(text_editor::Action::Edit(
            text_editor::Edit::Paste(Arc::new(text)),
        ))),
        CtxTarget::SettingsUa => Message::Settings(SettingsMsg::UaEditor(
            text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new(text))),
        )),
        CtxTarget::SettingsBtTracker => Message::Settings(SettingsMsg::BtTrackerEditor(
            text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new(text))),
        )),
        single => {
            let old = ctx_value(state, single);
            let (merged, new_caret) = match ctx_cur(state, single) {
                Some(cur) => ctx_menu::merge_paste(old, &cur, &text),
                None => (format!("{old}{text}"), old.len() + text.len()),
            };
            if let Some(c) = state.input_cursors.get(&single) {
                c.borrow_mut().pending_caret = Some((new_caret, merged.len()));
            }
            match single {
                CtxTarget::Search => Message::Sort(SortMsg::SearchChanged(merged)),
                CtxTarget::AddOut => Message::Add(AddMsg::AddFieldChanged(AddField::Out, merged)),
                CtxTarget::AddAdvanced(field) => {
                    Message::Add(AddMsg::AddFieldChanged(field, merged))
                }
                CtxTarget::DetailsAdvanced(field) => {
                    Message::Task(TaskMsg::DetailsAdvancedFieldChanged(field, merged))
                }
                CtxTarget::SettingsCustomTracker => {
                    Message::Settings(SettingsMsg::TrackerCustomInputChanged(merged))
                }
                CtxTarget::AddUrl | CtxTarget::SettingsUa | CtxTarget::SettingsBtTracker => {
                    unreachable!()
                }
            }
        }
    }
}

pub(crate) fn pill_to_index(state: &mut Remotrix, index: usize) {
    state
        .filter_pill
        .set_target(index as f32 * crate::ui::dims::FILTER_STEP);
}

pub(crate) fn set_page(state: &mut Remotrix, page: Page) {
    if state.page != page {
        state.page = page;
        let index = match page {
            Page::Tasks => crate::ui::category_bar::task_filter_index(state.task_filter),
            Page::Settings => crate::ui::category_bar::settings_cat_index(state.settings_cat),
        };
        state
            .filter_pill
            .settle_at(index as f32 * crate::ui::dims::FILTER_STEP);
    }
}

pub fn update(state: &mut Remotrix, message: Message) -> Task<Message> {
    crate::update::dispatch(state, message)
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
    #[cfg(target_os = "windows")]
    if state.window.resizing {
        return resize_placeholder_view(state);
    }
    let titlebar = crate::ui::title_bar::view(t, state.window.maximized);
    let left_col = crate::ui::sidebar::view(&state.fluent, t, state.page);

    let mid_col = crate::ui::category_bar::view(
        &state.fluent,
        t,
        state.page,
        state.task_filter,
        state.settings_cat,
        &counts,
        &state.filter_pill,
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
                !state.tasks.is_empty(),
                state.sort_field,
                state.sort_order,
                state.sort_menu_open,
                &state.search_query,
                &state.progress_anim,
                &state.card_anim,
                &state.input_cursors,
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
                settings_dirty: state.settings_dirty,
                engine_restart_pending: state.restart.engine_restart_pending,
                engine_restart_in_progress: state.restart.engine_restart_in_progress,
                aria2_version: state.engine_ui.aria2_version.as_deref(),
                aria2_status: state
                    .engine_ui
                    .aria2_status
                    .as_ref()
                    .map(|(s, m)| (s.as_str(), m.as_str())),
                aria2_fetch_error: state.engine_ui.aria2_fetch_error.as_deref(),
                update_check_in_flight: state.engine_ui.update_check_in_flight,
                aria2_download_version: state.engine_ui.aria2_downloading_version.as_deref(),
                aria2_download_progress: state.engine_ui.aria2_download_progress,
                ua_editor: &state.ua_editor,
                bt_tracker_editor: &state.bt_tracker_editor,
                path_history: &state.settings.path_history,
                font_restart_required: state.settings.font_family != state.applied_font_family,
                ctx_mirrors: &state.input_cursors,
                port_status: &state.port_status,
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

    let framed: iced::Element<'_, Message> = if state.window.maximized {
        let base = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::style::base_background);
        #[allow(clippy::useless_conversion)]
        {
            iced::widget::opaque(base).into()
        }
    } else {
        let base = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::style::base_background);
        let border = container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::style::window_border);
        stack![
            iced::widget::opaque(base),
            crate::ui::resize_frame::view(),
            border,
        ]
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
        dl,
        up,
        &state.hud_anim,
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
        let content = crate::ui::add_dialog::view(
            &state.fluent,
            t,
            &state.add_dialog,
            &state.settings.path_history,
            state.add_dialog_anim.value(),
            &state.input_cursors,
        );
        crate::ui::components::dialog::overlay(
            crate::ui::animation::animation(state.add_dialog_anim.anim(), content)
                .on_update(Message::AddDialogAnim),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let about_layer: iced::Element<'_, Message> = if state.about_dialog_visible {
        let content = crate::ui::about_dialog::view(
            &state.fluent,
            t,
            state.engine_ui.aria2_version.as_deref(),
            state.about_dialog_anim.value(),
        );
        crate::ui::components::dialog::overlay(
            crate::ui::animation::animation(state.about_dialog_anim.anim(), content)
                .on_update(Message::AboutDialogAnim),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let close_layer: iced::Element<'_, Message> = if state.window.show_close_dialog {
        if let Some(anim) = &state.window.close_dialog_anim {
            let content = crate::ui::close_dialog::view(
                &state.fluent,
                t,
                state.tray.enabled(),
                state.settings.close_to_tray,
                *anim.value(),
            );
            crate::ui::components::dialog::overlay(
                crate::ui::animation::animation(anim, content).on_update(Message::CloseDialogAnim),
            )
        } else {
            iced::widget::Space::new().into()
        }
    } else {
        iced::widget::Space::new().into()
    };

    let details_layer: iced::Element<'_, Message> = if state.details.is_visible() {
        let task = state
            .details
            .gid
            .as_deref()
            .and_then(|g| state.tasks.get(g));
        let content = crate::ui::details_dialog::view(
            &state.fluent,
            t,
            task,
            &state.details,
            state.details_anim.value(),
            &state.input_cursors,
            &state.progress_anim,
        );
        crate::ui::components::dialog::overlay(
            crate::ui::animation::animation(state.details_anim.anim(), content)
                .on_update(Message::DetailsAnim),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let confirm_layer: iced::Element<'_, Message> = if let Some(action) = &state.confirm {
        let content =
            crate::ui::confirm_dialog::view(&state.fluent, t, action, state.confirm_anim.value());
        crate::ui::components::dialog::overlay(
            crate::ui::animation::animation(state.confirm_anim.anim(), content)
                .on_update(Message::ConfirmAnim),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let update_layer: iced::Element<'_, Message> = if let Some(dialog) = &state.update_dialog {
        let content = crate::ui::update_dialog::view(
            &state.fluent,
            t,
            &dialog.offers,
            &dialog.changelogs,
            dialog.active_tab,
            state.update_dialog_anim.value(),
        );
        crate::ui::components::dialog::overlay(
            crate::ui::animation::animation(state.update_dialog_anim.anim(), content)
                .on_update(Message::UpdateDialogAnim),
        )
    } else {
        iced::widget::Space::new().into()
    };

    let drop_overlay_layer: iced::Element<'_, Message> = if state.drop_hover
        && !(state.window.show_close_dialog
            || state.about_dialog_visible
            || state.confirm.is_some()
            || state.update_dialog.is_some())
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

    let ctx_layer: iced::Element<'_, Message> = if let Some(menu) = &state.ctx_menu {
        let selected: Option<String> = match menu.target {
            CtxTarget::AddUrl => state.add_dialog.url_editor.selection(),
            CtxTarget::SettingsUa => state.ua_editor.selection(),
            CtxTarget::SettingsBtTracker => state.bt_tracker_editor.selection(),
            t => state.input_cursors.get(&t).and_then(|c| {
                c.borrow().selection.map(|(a, b)| {
                    iced::widget::text_input::Value::new(ctx_value(state, t))
                        .select(a, b)
                        .to_string()
                })
            }),
        };
        let selected = if ctx_menu::is_secure_target(menu.target) {
            None
        } else {
            selected
        };
        let position = menu.position;
        let menu_el = ctx_menu::menu(&state.fluent, selected, menu.clipboard.clone(), menu.target);
        stack![
            mouse_area(
                iced::widget::Space::new()
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::CtxClose)
            .on_right_press(Message::CtxClose),
            float::Float::new(menu_el).translate(move |bounds, viewport| {
                let px = position
                    .x
                    .clamp(0.0, (viewport.width - bounds.width).max(0.0));
                let py = position
                    .y
                    .clamp(0.0, (viewport.height - bounds.height).max(0.0));
                Vector::new(px - bounds.x, py - bounds.y)
            }),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        iced::widget::Space::new().into()
    };

    let shutdown_layer: iced::Element<'_, Message> = if state.shutdown.card_open {
        let card = crate::ui::shutdown_card::view(&state.fluent, t, &state.shutdown);
        stack![
            mouse_area(
                iced::widget::Space::new()
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::Shutdown(crate::message::ShutdownMsg::CloseCard)),
            float::Float::new(card).translate(move |bounds, viewport| {
                let x = SIDEBAR_W + 8.0;
                let y = (viewport.height - SHUTDOWN_CARD_ANCHOR)
                    .clamp(0.0, (viewport.height - bounds.height).max(0.0));
                Vector::new(x - bounds.x, y - bounds.y)
            }),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
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
        update_layer,
        drop_overlay_layer,
        toast_layer,
        ctx_layer,
        shutdown_layer,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

    container(stacked)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(target_os = "windows")]
fn resize_placeholder_view(state: &Remotrix) -> Element<'static, Message> {
    let base = container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::ui::theme::style::base_background);
    if state.window.maximized {
        #[allow(clippy::useless_conversion)]
        {
            iced::widget::opaque(base).into()
        }
    } else {
        stack![iced::widget::opaque(base), crate::ui::resize_frame::view(),]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
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

struct WakeSlot(Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Message>>>>);

impl Hash for WakeSlot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl PartialEq for WakeSlot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for WakeSlot {}

impl Clone for WakeSlot {
    fn clone(&self) -> Self {
        WakeSlot(self.0.clone())
    }
}

fn build_wake_stream(slot: &WakeSlot) -> impl iced::futures::Stream<Item = Message> {
    let rx = {
        let mut guard = slot.0.lock().expect("wake rx slot poisoned");
        guard.take()
    };
    iced::stream::channel(
        4,
        move |mut sender: iced::futures::channel::mpsc::Sender<Message>| async move {
            if let Some(mut rx) = rx {
                while let Some(msg) = rx.recv().await {
                    let _ = sender.send(msg).await;
                }
            }
        },
    )
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
                    let _ = sender
                        .send(Message::Engine(EngineMsg::Event(Box::new(ev))))
                        .await;
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

    let wake = Subscription::run_with(WakeSlot(state.wake_rx_slot.clone()), build_wake_stream);

    let extension = Subscription::run_with(
        ExtensionSlot(state.ext_msg_rx_slot.clone()),
        build_extension_stream,
    );

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let notify = Subscription::run_with(
        crate::notify::NotifySlot(state.notify_rx_slot.clone()),
        crate::notify::build_notify_stream,
    );

    let tray = Subscription::run_with(
        crate::tray::TraySlot(state.tray_rx_slot.clone()),
        crate::tray::build_tray_stream,
    );

    let open = iced::window::open_events().map(|id| Message::Window(WindowMsg::WindowOpened(id)));
    let close =
        iced::window::close_requests().map(|_id| Message::Window(WindowMsg::CloseRequested));
    let focus = iced::event::listen_with(|event, _status, window| match event {
        iced::event::Event::Window(iced::window::Event::Focused) => {
            Some(Message::Window(WindowMsg::WindowFocused(window)))
        }
        _ => None,
    });

    let ctx_cursor = iced::event::listen_with(|event, _status, _window| match event {
        iced::event::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::CursorMoved(position))
        }
        iced::event::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        }) => Some(Message::CtxClose),
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

    #[cfg(target_os = "windows")]
    let resize_tick = if state.window.resizing {
        iced::time::every(Duration::from_millis(RESIZE_TICK_MS))
            .map(|_| Message::Window(WindowMsg::ResizeTick))
    } else {
        Subscription::none()
    };

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

    let auto_update = if state.settings.update.enabled {
        iced::time::every(Duration::from_secs(3600))
            .map(|_| Message::Settings(SettingsMsg::CheckAutoUpdate { startup: false }))
    } else {
        Subscription::none()
    };

    let ed2k_bootstrap_auto_sync = if state.settings.aria2.ed2k_bootstrap_auto_sync {
        let hours = state
            .settings
            .aria2
            .ed2k_bootstrap_sync_interval_hours
            .max(1);
        iced::time::every(Duration::from_secs(hours as u64 * 3600))
            .map(|_| Message::Settings(SettingsMsg::Ed2kBootstrapSyncNow))
    } else {
        Subscription::none()
    };

    let shutdown_tick = if state.shutdown.timer_enabled
        || matches!(state.confirm, Some(ConfirmAction::Shutdown { .. }))
    {
        iced::time::every(Duration::from_secs(1))
            .map(|_| Message::Shutdown(crate::message::ShutdownMsg::ShutdownTick))
    } else {
        Subscription::none()
    };

    Subscription::batch(vec![
        engine,
        wake,
        extension,
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        notify,
        tray,
        open,
        close,
        focus,
        ctx_cursor,
        files,
        flush,
        resizes,
        #[cfg(target_os = "windows")]
        resize_tick,
        persist_periodic,
        refresh,
        toast_tick,
        signals,
        tracker_auto_sync,
        auto_update,
        ed2k_bootstrap_auto_sync,
        shutdown_tick,
    ])
}

#[derive(Clone)]
struct ExtensionSlot(Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Message>>>>);

impl std::hash::Hash for ExtensionSlot {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl PartialEq for ExtensionSlot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ExtensionSlot {}

fn build_extension_stream(slot: &ExtensionSlot) -> impl iced::futures::Stream<Item = Message> {
    let rx = {
        let mut guard = slot.0.lock().expect("extension slot poisoned");
        guard.take()
    };
    iced::stream::channel(
        1,
        move |mut sender: iced::futures::channel::mpsc::Sender<Message>| async move {
            if let Some(mut rx) = rx {
                while let Some(msg) = rx.recv().await {
                    let _ = sender.send(msg).await;
                }
            }
        },
    )
}

pub(crate) fn picker_mut(
    state: &mut Remotrix,
    id: PathPickerId,
) -> &mut crate::ui::components::path_picker::PathPicker {
    match id {
        PathPickerId::DownloadDir => &mut state.settings_ui.download_picker,
        PathPickerId::SaveDir => &mut state.add_dialog.save_picker,
        PathPickerId::Torrent => unreachable!("torrent upload is not a PathPicker"),
        PathPickerId::Metalink => unreachable!("metalink upload is not a PathPicker"),
        PathPickerId::Ed2kServerList => &mut state.settings_ui.ed2k_server_list_picker,
        PathPickerId::Ed2kNodeList => &mut state.settings_ui.ed2k_node_list_picker,
    }
}

pub(crate) fn apply_path(state: &mut Remotrix, id: PathPickerId, p: PathBuf) {
    let s = p.to_string_lossy().to_string();
    match id {
        PathPickerId::DownloadDir => {
            state.settings.record_path(id.history_key(), &s);
            state.applied_settings.record_path(id.history_key(), &s);
            state.settings.download_dir = p.clone();
            state
                .settings_ui
                .download_picker
                .set_value(p.to_string_lossy());
            state.settings_ui.download_picker.close_history();
            mark_settings_dirty(state);
        }
        PathPickerId::SaveDir => {
            state.settings.record_path(id.history_key(), &s);
            state.applied_settings.record_path(id.history_key(), &s);
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
        PathPickerId::Metalink => {
            if crate::engine::is_metalink_file(&p) {
                state
                    .add_dialog
                    .set_metalink_path(p.to_string_lossy().to_string());
            } else {
                let toast = Toast::new(ToastKind::Warning, state.fluent.get(Tr::InvalidMetalink))
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
            mark_settings_dirty(state);
        }
        PathPickerId::Ed2kNodeList => {
            state.settings.aria2.ed2k_node_list = p.to_string_lossy().into_owned();
            state
                .settings_ui
                .ed2k_node_list_picker
                .set_value(p.to_string_lossy());
            state.settings_ui.ed2k_node_list_picker.close_history();
            mark_settings_dirty(state);
        }
    }
}

pub(crate) fn details_files_tree(
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

pub(crate) fn selected_details_indices(state: &Remotrix) -> Vec<u64> {
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

pub(crate) fn schedule_details_select_flush(state: &mut Remotrix) -> Task<Message> {
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

pub(crate) fn spawn_toast(
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

pub(crate) fn copy_to_clipboard(state: &mut Remotrix, content: String) -> Task<Message> {
    if content.is_empty() {
        return Task::none();
    }
    spawn_toast(
        state,
        ToastGroup::General,
        ToastKind::Success,
        state.fluent.get(Tr::Copied),
        Some(Duration::from_secs(2)),
        false,
    );
    iced::clipboard::write::<Message>(content)
}

pub(crate) fn dismiss_toast(state: &mut Remotrix, id: u64) {
    state.toasts.dismiss(id);
}

const SHUTDOWN_CONFIRM_SECS: u32 = 30;
const SHUTDOWN_CARD_ANCHOR: f32 = 112.0;

pub(crate) fn reset_shutdown_card(state: &mut Remotrix) {
    let minutes = state.shutdown.timer_minutes.max(1);
    state.shutdown = crate::shutdown::ShutdownControl {
        timer_minutes: minutes,
        ..Default::default()
    };
}

pub(crate) fn trigger_shutdown_confirm(state: &mut Remotrix) {
    if matches!(state.confirm, Some(ConfirmAction::Shutdown { .. })) {
        return;
    }
    state.confirm = Some(ConfirmAction::Shutdown {
        seconds_left: SHUTDOWN_CONFIRM_SECS,
    });
    state.confirm_anim.open();
}

pub(crate) fn handle_shutdown(
    state: &mut Remotrix,
    msg: crate::message::ShutdownMsg,
) -> Task<Message> {
    use crate::message::ShutdownMsg;
    match msg {
        ShutdownMsg::ToggleCard => {
            state.shutdown.card_open = !state.shutdown.card_open;
        }
        ShutdownMsg::CloseCard => {
            state.shutdown.card_open = false;
        }
        ShutdownMsg::SetAfterComplete(v) => {
            state.shutdown.after_complete = v;
        }
        ShutdownMsg::SetTimerEnabled(v) => {
            state.shutdown.timer_enabled = v;
            if v {
                state.shutdown.timer_deadline = Some(
                    Instant::now() + Duration::from_secs(state.shutdown.timer_minutes as u64 * 60),
                );
            } else {
                state.shutdown.timer_deadline = None;
            }
        }
        ShutdownMsg::SetTimerMinutes(n) => {
            state.shutdown.timer_minutes = n.max(1);
            if state.shutdown.timer_enabled {
                state.shutdown.timer_deadline = Some(
                    Instant::now() + Duration::from_secs(state.shutdown.timer_minutes as u64 * 60),
                );
            }
        }
        ShutdownMsg::ShutdownTick => {
            if state.confirm_anim.is_dismissing() {
                return Task::none();
            }
            if let Some(ConfirmAction::Shutdown { seconds_left }) = state.confirm {
                if seconds_left <= 1 {
                    state.confirm = None;
                    state.confirm_anim.begin_exit();
                    reset_shutdown_card(state);
                    return Task::done(Message::Shutdown(ShutdownMsg::ShutdownNow));
                } else {
                    state.confirm = Some(ConfirmAction::Shutdown {
                        seconds_left: seconds_left - 1,
                    });
                }
            } else if state.shutdown.timer_enabled && state.confirm.is_none() {
                let expired = state
                    .shutdown
                    .timer_deadline
                    .map(|d| Instant::now() >= d)
                    .unwrap_or(false);
                if expired {
                    reset_shutdown_card(state);
                    trigger_shutdown_confirm(state);
                }
            }
        }
        ShutdownMsg::ShutdownNow => {
            return Task::perform(
                async move { crate::shutdown::shutdown_system_blocking() },
                |res| {
                    Message::Shutdown(ShutdownMsg::ShutdownExecuted {
                        ok: res.is_ok(),
                        error: res.err(),
                    })
                },
            );
        }
        ShutdownMsg::ShutdownExecuted { ok, error } => {
            state.confirm = None;
            if ok {
                tracing::info!("system shutdown requested");
            } else if let Some(err) = error {
                let mut args = std::collections::HashMap::new();
                args.insert(
                    std::borrow::Cow::from("error"),
                    std::borrow::Cow::from(err).into(),
                );
                spawn_toast(
                    state,
                    ToastGroup::General,
                    ToastKind::Error,
                    state.fluent.get_args(Tr::ShutdownFailed, &args),
                    Some(Duration::from_secs(6)),
                    true,
                );
            }
        }
    }
    Task::none()
}

pub(crate) fn send_system_notification(
    state: &mut Remotrix,
    title: String,
    body: String,
    buttons: Vec<(String, crate::notify::NotifyAction)>,
    default_action: crate::notify::NotifyAction,
) {
    #[cfg(target_os = "linux")]
    {
        if let Some((handle, actions)) =
            crate::notify::show(&state.notifiers, &title, &body, &buttons)
        {
            let _ = state.notify_tx.send(crate::notify::NotifyEvent {
                handle,
                actions,
                default_action,
            });
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(event) =
            crate::notify::show(&state.notifiers, &title, &body, &buttons, default_action)
        {
            let _ = state.notify_tx.send(event);
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = default_action;
        crate::notify::show(&state.notifiers, &title, &body, &buttons);
    }
}

pub(crate) fn check_updates(state: &mut Remotrix, startup: bool, manual: bool) -> Task<Message> {
    if state.engine_ui.update_check_in_flight {
        return Task::none();
    }
    if state.engine_ui.aria2_version.is_none() {
        return Task::none();
    }
    if state.settings.update != state.applied_settings.update {
        return Task::none();
    }
    let now_ms = chrono::Local::now().timestamp_millis();
    if !manual {
        if !state.settings.update.enabled {
            return Task::none();
        }
        if !state.settings.update.check_due(startup, now_ms) {
            return Task::none();
        }
    }
    state.engine_ui.update_check_in_flight = true;
    state.settings.update.last_check_time = Some(now_ms);
    state.applied_settings.update.last_check_time = Some(now_ms);
    config::save(&state.applied_settings);

    let scope = state.settings.update.scope;
    let silent = state.settings.update.aria2_silent_update;
    let beta = state.settings.update.beta_channel;
    let aria2_downloading = state.engine_ui.aria2_downloading;
    let proxy = state.settings.aria2.all_proxy_value();
    let app_current = crate::app_updater::current_app_version().to_string();
    let engine_current = crate::aria2_fetcher::installed_version().unwrap_or_default();
    let slug = crate::updater::platform_slug();
    let app_kind = crate::app_updater::detect_install_kind();
    let timeout_msg = state.fluent.get(Tr::UpdateCheckTimeout);

    Task::perform(
        async move {
            let fetched = tokio::time::timeout(Duration::from_secs(30), async {
                let mut offers = Vec::new();
                let mut silent_applied = Vec::new();
                let mut errors = Vec::new();

                if !aria2_downloading && scope.covers("aria2-next") {
                    match crate::updater::fetch_latest_release(
                        "AnInsomniacy/aria2-next",
                        "aria2-next",
                        slug,
                        false,
                        proxy.clone(),
                        false,
                    )
                    .await
                    {
                        Ok(latest) => {
                            if crate::updater::version_gt(&latest.version, &engine_current) {
                                let settings = crate::config::load();
                                if !settings.update.is_skipped("aria2-next", &latest.version) {
                                    let offer = crate::ui::update_dialog::UpdateOffer {
                                        component: crate::ui::update_dialog::UpdateComponent::Aria2,
                                        current: engine_current.clone(),
                                        latest: latest.version.clone(),
                                        changelog: String::new(),
                                        download_url: latest.download_url.clone(),
                                        sha256: None,
                                        asset_name: latest.asset_name.clone(),
                                    };
                                    if silent {
                                        silent_applied.push(offer);
                                    } else {
                                        offers.push(offer);
                                    }
                                }
                            }
                        }
                        Err(e) => errors.push(format!("aria2-next: {e}")),
                    }
                }

                if scope.covers("remotrix") {
                    let kind = app_kind;
                    match crate::updater::fetch_latest_asset(
                        crate::updater::APP_REPO,
                        move |name| kind.asset_matches(name),
                        false,
                        proxy.clone(),
                        beta,
                    )
                    .await
                    {
                        Ok(latest) => {
                            if crate::updater::version_gt(&latest.version, &app_current) {
                                offers.push(crate::ui::update_dialog::UpdateOffer {
                                    component: crate::ui::update_dialog::UpdateComponent::App,
                                    current: app_current.clone(),
                                    latest: latest.version.clone(),
                                    changelog: String::new(),
                                    download_url: latest.download_url.clone(),
                                    sha256: None,
                                    asset_name: latest.asset_name.clone(),
                                });
                            }
                        }
                        Err(e) => errors.push(format!("remotrix: {e}")),
                    }
                }

                (offers, silent_applied, errors)
            })
            .await;

            match fetched {
                Ok((offers, silent_applied, errors)) => {
                    Message::Settings(SettingsMsg::UpdateResult {
                        offers,
                        silent_applied,
                        errors,
                    })
                }
                Err(_) => Message::Settings(SettingsMsg::UpdateResult {
                    offers: Vec::new(),
                    silent_applied: Vec::new(),
                    errors: vec![timeout_msg],
                }),
            }
        },
        std::convert::identity,
    )
}

pub(crate) fn changelog_fetch_task(state: &Remotrix, tab: usize) -> Task<Message> {
    let Some(dialog) = &state.update_dialog else {
        return Task::none();
    };
    let Some(offer) = dialog.offers.get(tab) else {
        return Task::none();
    };
    let current = offer.current.clone();
    let proxy = state.settings.aria2.all_proxy_value();
    let beta = state.settings.update.beta_channel;
    let task: std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Vec<crate::updater::ReleaseInfo>, String>>
                + Send,
        >,
    > = match offer.component {
        crate::ui::update_dialog::UpdateComponent::App => {
            let kind = crate::app_updater::detect_install_kind();
            Box::pin(crate::updater::fetch_changelog(
                crate::updater::APP_REPO,
                current,
                move |name, _| kind.asset_matches(name),
                proxy,
                beta,
            ))
        }
        crate::ui::update_dialog::UpdateComponent::Aria2 => {
            let slug = crate::updater::platform_slug();
            Box::pin(crate::updater::fetch_changelog(
                "AnInsomniacy/aria2-next",
                current,
                move |name, version| name == format!("aria2-next-{version}-{slug}"),
                proxy,
                false,
            ))
        }
    };
    Task::perform(task, move |r| {
        Message::Settings(SettingsMsg::UpdateChangelogLoaded { tab, releases: r })
    })
}

pub(crate) fn concat_changelog(rels: &[crate::updater::ReleaseInfo]) -> String {
    let mut parts = Vec::new();
    for rel in rels {
        let mut block = format!("v{}", rel.version);
        if !rel.notes.trim().is_empty() {
            block.push_str("\n\n");
            block.push_str(rel.notes.trim_end());
        }
        parts.push(block);
    }
    parts.join("\n\n---\n\n")
}

pub(crate) fn send_download_aria2_update(
    state: &mut Remotrix,
    offer: &crate::ui::update_dialog::UpdateOffer,
    silent: bool,
) {
    state.engine_ui.aria2_download_silent = silent;
    state.engine_ui.aria2_downloading = true;
    state.engine_ui.aria2_downloading_version = Some(offer.latest.clone());
    state.engine_ui.aria2_download_progress = None;
    if !silent {
        spawn_toast(
            state,
            ToastGroup::Engine,
            ToastKind::Normal,
            state.fluent.get(Tr::UpdateDownloading),
            Some(Duration::from_secs(4)),
            false,
        );
        send_system_notification(
            state,
            state.fluent.get(Tr::Aria2UpdateStartingTitle),
            state.fluent.get(Tr::Aria2UpdateStartingBody),
            vec![],
            crate::notify::NotifyAction::ActivateWindow,
        );
    }
    let _ = state.handle.cmd_tx.send(EngineCmd::DownloadAria2Update {
        version: offer.latest.clone(),
        asset_name: offer.asset_name.clone(),
        download_url: offer.download_url.clone(),
        sha256: offer.sha256.clone(),
    });
}

pub(crate) fn start_tracker_fetch(state: &mut Remotrix, urls: Vec<String>) -> Task<Message> {
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
    let proxy = state.settings.aria2.all_proxy_value();
    let fetch = Task::perform(
        async move { crate::trackers::fetch_sources(&urls, proxy).await },
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

pub(crate) fn pick_path(id: PathPickerId) -> Task<Message> {
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
            PathPickerId::Metalink => {
                dialog
                    .set_title("Select metalink file")
                    .add_filter("Metalink", &["metalink", "meta4"])
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

pub(crate) fn open_path_in_manager(p: PathBuf) -> Task<Message> {
    if p.as_os_str().is_empty() {
        return Task::none();
    }
    let target = if p.is_dir() {
        p
    } else {
        p.parent()
            .filter(|q| !q.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or(p)
    };
    Task::perform(
        async move {
            let _ = open::that(&target);
        },
        |_| Message::Noop,
    )
}

pub(crate) fn continue_close_flow(state: &mut Remotrix) -> Task<Message> {
    if state.settings.close_to_tray && state.tray.enabled() {
        return hide_to_tray(state);
    }
    state.window.show_close_dialog = true;
    open_close_dialog(state);
    Task::none()
}

pub(crate) fn open_close_dialog(state: &mut Remotrix) {
    state.window.close_dialog_anim = Some(
        crate::ui::animation::Animated::transition(
            0.0,
            crate::ui::animation::ease_out_cubic(crate::ui::animation::CARD_ENTER_MS),
        )
        .to(1.0),
    );
    state.window.close_dialog_dismissing = false;
}

pub(crate) fn hide_to_tray(state: &mut Remotrix) -> Task<Message> {
    state.window.show_close_dialog = false;
    state.window.close_dialog_anim = None;
    state.window.close_dialog_dismissing = false;
    state.window.hidden_to_tray = true;
    refresh_tray(state);
    tracing::info!(
        wayland = state.window.wayland,
        window_id = state.window.window_id.is_some(),
        "hide to tray"
    );
    let Some(id) = state.window.window_id else {
        return Task::none();
    };
    if state.window.wayland {
        iced::window::minimize::<Message>(id, true)
    } else {
        iced::window::set_mode::<Message>(id, iced::window::Mode::Hidden)
    }
}

pub(crate) fn restore_window_from_tray_wayland(state: &mut Remotrix) -> Task<Message> {
    let task = restore_window_from_tray(state);
    if state.window.wayland {
        let title = state.fluent.get(Tr::TrayWaylandFocusTitle);
        let body = state.fluent.get(Tr::TrayWaylandFocusBody);
        send_system_notification(
            state,
            title,
            body,
            vec![],
            crate::notify::NotifyAction::ActivateWindow,
        );
    }
    task
}

pub(crate) fn restore_window_from_tray(state: &mut Remotrix) -> Task<Message> {
    if state.window.hidden_to_tray {
        state.window.hidden_to_tray = false;
        refresh_tray(state);
    }
    let mut task = Task::none();
    if let Some(id) = state.window.window_id {
        task = iced::window::set_mode::<Message>(id, iced::window::Mode::Windowed)
            .chain(iced::window::gain_focus(id));
    }
    task
}

pub(crate) fn open_add_dialog(state: &mut Remotrix) {
    state.add_dialog.save_picker.close_history();
    state
        .add_dialog
        .open(state.settings.download_dir.clone(), state.settings.split);
    state.add_dialog_anim.open();
}

pub(crate) fn refresh_tray(state: &mut Remotrix) {
    if !state.tray.enabled() {
        return;
    }
    let summary = tray_summary(state);
    state.tray.refresh(&summary);
}

fn tray_summary(state: &Remotrix) -> crate::tray::TraySummary {
    let active = state.tracking.active_count;
    let paused = state.tracking.paused_gids.len();
    let total = state.tasks.len();
    let (down_speed, up_speed) = if active > 0 {
        state.global_speed.unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    let mut args = std::collections::HashMap::new();
    args.insert(
        std::borrow::Cow::from("down"),
        crate::task::format_speed(down_speed).into(),
    );
    args.insert(
        std::borrow::Cow::from("up"),
        crate::task::format_speed(up_speed).into(),
    );
    args.insert(std::borrow::Cow::from("active"), (active as i64).into());
    let labels = crate::tray::TrayLabels {
        show: state.fluent.get(Tr::TrayShow),
        hide: state.fluent.get(Tr::TrayHide),
        new: state.fluent.get(Tr::TrayNewDownload),
        pause_all: state.fluent.get(Tr::TrayPauseAll),
        start_all: state.fluent.get(Tr::TrayStartAll),
        open_dir: state.fluent.get(Tr::TrayOpenFolder),
        settings: state.fluent.get(Tr::TraySettings),
        quit: state.fluent.get(Tr::TrayQuit),
        tooltip: state.fluent.get_args(Tr::TrayTooltip, &args),
    };
    crate::tray::TraySummary {
        active,
        paused,
        total,
        download_dir: state.settings.download_dir.clone(),
        engine_degraded: state.engine_ui.aria2_fetch_error.is_some(),
        hidden: state.window.hidden_to_tray,
        labels,
    }
}
