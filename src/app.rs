use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use iced::futures::SinkExt;
use iced::widget::{column, combo_box, container, row, stack};
use iced::window::Id;
use iced::{Element, Length, Subscription, Task};

use crate::config::{self, Settings};
use crate::engine::{EngineCmd, EngineEvent, EngineHandle, EventRx};
use crate::i18n::{Fluent, Locale};
use crate::message::{
    CloseDialogChoice, FileKind, Message, Page, SettingKey, SettingsCategory, SortField, SortOrder,
    TaskFilter, WindowCmd,
};
use crate::task::{DownloadTask, TaskStatus};
use crate::ui::add_dialog::AddDialogState;
use crate::ui::category_bar::Counts;
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
    pending_speed_apply: bool,
    fluent: Fluent,
    theme: iced::Theme,
    maximized: bool,
    show_close_dialog: bool,
    window_id: Option<Id>,
    sort_combo_state: combo_box::State<SortField>,
    sort_field: SortField,
    sort_order: SortOrder,
    aria2_version: Option<String>,
    aria2_check_msg: Option<String>,
    update_pending: Option<String>,
    aria2_status: Option<(String, String)>,
    aria2_fetch_error: Option<String>,
    logo_handle: iced::widget::image::Handle,
}

pub fn init() -> (Remotrix, Task<Message>) {
    config::announce();
    let settings = config::load();

    let (handle, event_rx) = crate::engine::spawn_engine();

    let add_dialog = AddDialogState::new(settings.download_dir.clone());
    let fluent = Fluent::new(settings.locale);

    let theme = theme::build_iced(theme::resolve_mode(settings.theme_mode, None));
    let logo_handle =
        iced::widget::image::Handle::from_bytes(&include_bytes!("../assets/icon.png")[..]);

    let sort_options = vec![
        SortField::AddedTime,
        SortField::Name,
        SortField::Size,
        SortField::Progress,
        SortField::Status,
    ];
    let state = Remotrix {
        sort_combo_state: combo_box::State::new(sort_options),
        page: Page::Tasks,
        task_filter: TaskFilter::All,
        settings_cat: SettingsCategory::General,
        tasks: HashMap::new(),
        task_order: Vec::new(),
        handle,
        event_rx_slot: Arc::new(Mutex::new(Some(event_rx))),
        add_dialog,
        about_dialog_visible: false,
        settings,
        pending_speed_apply: false,
        fluent,
        theme,
        maximized: false,
        show_close_dialog: false,
        window_id: None,
        sort_field: SortField::AddedTime,
        sort_order: SortOrder::Desc,
        aria2_version: None,
        aria2_check_msg: None,
        update_pending: None,
        aria2_status: None,
        aria2_fetch_error: None,
        logo_handle,
    };

    (state, Task::none())
}

pub fn app_title(_state: &Remotrix) -> String {
    "Remotrix".to_string()
}

pub fn theme(state: &Remotrix) -> iced::Theme {
    state.theme.clone()
}

fn rebuild_theme(state: &mut Remotrix) {
    state.theme = theme::build_iced(theme::resolve_mode(state.settings.theme_mode, None));
}

pub fn update(state: &mut Remotrix, message: Message) -> Task<Message> {
    match message {
        Message::NavigatePage(page) => {
            state.page = page;
        }
        Message::SetTaskFilter(filter) => {
            state.task_filter = filter;
        }
        Message::SetSettingsCategory(cat) => {
            state.settings_cat = cat;
        }
        Message::OpenAddDialog => {
            state.add_dialog.open(state.settings.download_dir.clone());
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
                        if state.add_dialog.url.trim().is_empty() {
                            state.add_dialog.url = format!("file://{}", p.display());
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
        }
        Message::ClearCompleted => {
            state
                .tasks
                .retain(|_k, t| !matches!(t.status, TaskStatus::Completed | TaskStatus::Removed));
            state.task_order.retain(|gid| state.tasks.contains_key(gid));
        }
        Message::Refresh => {
            if state.handle.cmd_tx.send(EngineCmd::Snapshot).is_err() {
                tracing::warn!("ui: snapshot cmd send failed");
            }
        }
        Message::SortSelected(field) => {
            state.sort_field = field;
        }
        Message::OpenAbout => {
            state.about_dialog_visible = true;
        }
        Message::CloseAbout => {
            state.about_dialog_visible = false;
        }
        Message::SettingChanged(key, value) => match key {
            SettingKey::DownloadDir => {
                return pick_folder(FileKind::SaveDir);
            }
            SettingKey::MaxConcurrent => {
                if let Ok(n) = value.parse::<u32>() {
                    state.settings.max_concurrent = n.max(1);
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
        },
        Message::ApplySettings => {
            config::save(&state.settings);
            let dl = if state.settings.download_limit_kb > 0 {
                Some(state.settings.download_limit_kb * 1024)
            } else {
                None
            };
            let ul = if state.settings.upload_limit_kb > 0 {
                Some(state.settings.upload_limit_kb * 1024)
            } else {
                None
            };
            tracing::info!(
                max_concurrent = state.settings.max_concurrent,
                ?dl,
                ?ul,
                "ui: apply settings"
            );
            if state
                .handle
                .cmd_tx
                .send(EngineCmd::SetSpeedLimit {
                    download: dl,
                    upload: ul,
                })
                .is_err()
            {
                tracing::warn!("ui: set speed limit cmd send failed");
            }
        }
        Message::Engine(event) => match event {
            EngineEvent::EngineReady => {
                tracing::info!("engine ready");
                state.aria2_fetch_error = None;
            }
            EngineEvent::EngineStopped => {
                tracing::info!("engine stopped");
            }
            EngineEvent::Added { gid, name } => {
                tracing::info!(?gid, ?name, "ui: task added");
                let task = DownloadTask {
                    gid: gid.clone(),
                    name,
                    url: String::new(),
                    save_dir: PathBuf::new(),
                    downloaded: 0,
                    total: 0,
                    speed: 0,
                    status: TaskStatus::Waiting,
                };
                state.tasks.insert(gid.clone(), task);
                state.task_order.insert(0, gid);
            }
            EngineEvent::Progress {
                gid,
                downloaded,
                total,
                speed,
                status,
            } => {
                if let Some(t) = state.tasks.get_mut(&gid) {
                    t.downloaded = downloaded;
                    t.total = total;
                    t.speed = speed;
                    t.status = TaskStatus::from_engine(&status);
                }
            }
            EngineEvent::Removed(gid) => {
                tracing::info!(?gid, "ui: task removed");
                state.tasks.remove(&gid);
                state.task_order.retain(|g| g != &gid);
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
        Message::ThemeModeChanged(mode) => {
            state.settings.theme_mode = mode;
            rebuild_theme(state);
            config::save(&state.settings);
        }
        Message::LocaleChanged(locale) => {
            state.settings.locale = locale;
            state.fluent = Fluent::new(locale);
            config::save(&state.settings);
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
        Message::RestoreAutoCheck => {
            state.settings.update.set_ignored("aria2-next", false);
            config::save(&state.settings);
        }
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
            crate::ui::task_list::view(&state.fluent, t, &sorted, &state.sort_combo_state)
        }
        Page::Settings => crate::ui::settings_page::view(
            &state.fluent,
            t,
            &state.settings,
            state.aria2_version.as_deref(),
            state.aria2_check_msg.as_deref(),
            !state.settings.update.should_auto_check("aria2-next"),
            state
                .aria2_status
                .as_ref()
                .map(|(s, m)| (s.as_str(), m.as_str())),
            state.aria2_fetch_error.as_deref(),
            state.update_pending.as_deref(),
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

    let mut stacked = iced::widget::opaque(base);

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

    Subscription::batch(vec![engine, open, close])
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
