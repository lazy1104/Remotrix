use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ldtray::{Event, Icon, Menu, MenuItem, Notification, Tray, TrayConfig, TrayHandle};

use iced::futures::SinkExt;

use crate::message::{AddMsg, Message, NavMsg, Page, TaskMsg, TrayMsg, WindowMsg};

pub struct TraySummary {
    pub active: usize,
    pub paused: usize,
    pub total: usize,
    pub download_dir: PathBuf,
    pub engine_degraded: bool,
    pub hidden: bool,
    pub labels: TrayLabels,
}

pub struct TrayLabels {
    pub show: String,
    pub hide: String,
    pub new: String,
    pub pause_all: String,
    pub start_all: String,
    pub open_dir: String,
    pub settings: String,
    pub quit: String,
    pub tooltip: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayMenuId {
    Show,
    New,
    PauseAll,
    StartAll,
    OpenDir,
    Settings,
    Quit,
}

impl TrayMenuId {
    fn value(self) -> u32 {
        match self {
            Self::Show => 1,
            Self::New => 2,
            Self::PauseAll => 3,
            Self::StartAll => 4,
            Self::OpenDir => 5,
            Self::Settings => 6,
            Self::Quit => 7,
        }
    }

    fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::Show),
            2 => Some(Self::New),
            3 => Some(Self::PauseAll),
            4 => Some(Self::StartAll),
            5 => Some(Self::OpenDir),
            6 => Some(Self::Settings),
            7 => Some(Self::Quit),
            _ => None,
        }
    }
}

struct TrayInner {
    handle: TrayHandle,
    dir: Arc<Mutex<PathBuf>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SummaryState {
    active: usize,
    paused: usize,
    total: usize,
    download_dir: PathBuf,
    engine_degraded: bool,
    hidden: bool,
}

pub struct TrayManager {
    inner: Option<TrayInner>,
    last: Option<SummaryState>,
    last_tooltip: Option<Instant>,
}

impl TrayManager {
    pub fn new(tx: tokio::sync::mpsc::UnboundedSender<Message>, enabled: bool) -> Self {
        Self {
            inner: if enabled { init_tray(tx) } else { None },
            last: None,
            last_tooltip: None,
        }
    }

    pub fn enabled(&self) -> bool {
        self.inner.is_some()
    }

    pub fn refresh(&mut self, s: &TraySummary) {
        const TOOLTIP_THROTTLE: Duration = Duration::from_secs(1);
        let structural = SummaryState {
            active: s.active,
            paused: s.paused,
            total: s.total,
            download_dir: s.download_dir.clone(),
            engine_degraded: s.engine_degraded,
            hidden: s.hidden,
        };
        let structural_changed = self.last.as_ref() != Some(&structural);
        let tooltip_due = self
            .last_tooltip
            .map(|t| t.elapsed() >= TOOLTIP_THROTTLE)
            .unwrap_or(true);
        if !structural_changed && !(tooltip_due && (s.active > 0 || s.hidden)) {
            return;
        }
        let Some(inner) = &self.inner else {
            return;
        };
        if structural_changed {
            self.last = Some(structural);
            *inner.dir.lock().expect("tray dir poisoned") = s.download_dir.clone();
            if let Err(e) = inner.handle.set_menu(build_menu(s)) {
                tracing::debug!(error = %e, "tray: set_menu failed");
            }
            if let Err(e) = inner.handle.set_tooltip(s.labels.tooltip.clone()) {
                tracing::debug!(error = %e, "tray: set_tooltip failed");
            }
        } else {
            if let Err(e) = inner.handle.set_tooltip(s.labels.tooltip.clone()) {
                tracing::debug!(error = %e, "tray: set_tooltip failed");
            }
        }
        self.last_tooltip = Some(Instant::now());
    }

    pub fn quit(&mut self) {
        if let Some(inner) = &self.inner {
            if let Err(e) = inner.handle.quit() {
                tracing::debug!(error = %e, "tray: quit failed");
            }
        }
        self.inner = None;
        self.last = None;
        self.last_tooltip = None;
    }

    #[allow(dead_code)]
    pub fn notify(&self, title: &str, body: &str) {
        if let Some(inner) = &self.inner {
            if let Err(e) = inner.handle.notify(Notification::new(title, body)) {
                tracing::debug!(error = %e, "tray: notify failed");
            }
        }
    }
}

fn init_tray(tx: tokio::sync::mpsc::UnboundedSender<Message>) -> Option<TrayInner> {
    let icon = load_app_icon()?;
    let config = TrayConfig::new(icon).tooltip("Remotrix");
    let tray = match Tray::new(config) {
        Ok(tray) => tray,
        Err(e) => {
            tracing::debug!(error = %e, "tray: unavailable, running headless");
            return None;
        }
    };
    let dir = Arc::new(Mutex::new(PathBuf::new()));
    let handle = match tray.spawn(event_callback(tx, dir.clone())) {
        Ok(handle) => handle,
        Err(e) => {
            tracing::debug!(error = %e, "tray: could not spawn event loop");
            return None;
        }
    };
    Some(TrayInner { handle, dir })
}

fn load_app_icon() -> Option<Icon> {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    Icon::from_rgba(w, h, img.into_raw()).ok()
}

fn event_callback(
    tx: tokio::sync::mpsc::UnboundedSender<Message>,
    dir: Arc<Mutex<PathBuf>>,
) -> impl FnMut(Event) + Send + 'static {
    move |event| {
        let msg = match event {
            Event::LeftClick | Event::DoubleClick => Message::Tray(TrayMsg::ClickShow),
            Event::Menu(id) => match TrayMenuId::from_u32(id.0) {
                Some(TrayMenuId::Show) => Message::Tray(TrayMsg::ToggleWindow),
                Some(TrayMenuId::New) => Message::Add(AddMsg::OpenAddDialog),
                Some(TrayMenuId::PauseAll) => Message::Task(TaskMsg::PauseAll),
                Some(TrayMenuId::StartAll) => Message::Task(TaskMsg::StartAll),
                Some(TrayMenuId::OpenDir) => {
                    let path = dir.lock().expect("tray dir poisoned").clone();
                    Message::Task(TaskMsg::OpenFolder(path))
                }
                Some(TrayMenuId::Settings) => Message::Nav(NavMsg::NavigatePage(Page::Settings)),
                Some(TrayMenuId::Quit) => Message::Window(WindowMsg::ShutdownRequested),
                None => Message::Noop,
            },
            Event::RightClick | Event::MiddleClick | Event::NotificationAction(_) => Message::Noop,
            _ => Message::Noop,
        };
        let _ = tx.send(msg);
    }
}

fn build_menu(s: &TraySummary) -> Menu {
    let show_label = if s.hidden {
        &s.labels.show
    } else {
        &s.labels.hide
    };
    Menu::new()
        .item(MenuItem::button(TrayMenuId::Show.value(), show_label))
        .item(MenuItem::button(TrayMenuId::New.value(), &s.labels.new).enabled(!s.engine_degraded))
        .item(MenuItem::separator())
        .item(
            MenuItem::button(TrayMenuId::PauseAll.value(), &s.labels.pause_all)
                .enabled(s.active > 0),
        )
        .item(
            MenuItem::button(TrayMenuId::StartAll.value(), &s.labels.start_all)
                .enabled(s.paused > 0),
        )
        .item(MenuItem::separator())
        .item(MenuItem::button(
            TrayMenuId::OpenDir.value(),
            &s.labels.open_dir,
        ))
        .item(MenuItem::button(
            TrayMenuId::Settings.value(),
            &s.labels.settings,
        ))
        .item(MenuItem::separator())
        .item(MenuItem::button(TrayMenuId::Quit.value(), &s.labels.quit))
}

pub struct TraySlot(pub Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Message>>>>);

impl std::hash::Hash for TraySlot {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl PartialEq for TraySlot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for TraySlot {}

impl Clone for TraySlot {
    fn clone(&self) -> Self {
        TraySlot(self.0.clone())
    }
}

pub fn build_tray_stream(slot: &TraySlot) -> impl iced::futures::Stream<Item = Message> {
    let rx = {
        let mut guard = slot.0.lock().expect("tray slot poisoned");
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
