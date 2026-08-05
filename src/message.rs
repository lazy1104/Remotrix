use std::path::PathBuf;

use crate::clipboard_watch::ClipboardPayload;
use crate::engine::EngineEvent;
use crate::i18n::Locale;
use crate::ui::components::path_picker::PathPickerEvent;
use crate::ui::theme::ThemeMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPickerId {
    DownloadDir,
    SaveDir,
    Torrent,
    Ed2kServerList,
    Ed2kNodeList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddTab {
    Url,
    Torrent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddField {
    Out,
    UserAgent,
    HttpUser,
    HttpPasswd,
    Referer,
    Cookie,
    ProxyServer,
    ProxyUsername,
    ProxyPassword,
}

impl PathPickerId {
    pub fn history_key(self) -> &'static str {
        match self {
            Self::DownloadDir => "download_dir",
            Self::SaveDir => "save_dir",
            Self::Torrent => "torrent",
            Self::Ed2kServerList => "ed2k_server_list",
            Self::Ed2kNodeList => "ed2k_node_list",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeleteAll,
    ClearCompleted,
    DeleteTask(String),
    RemoveMissingFileTask(String),
    LeaveSettings { target: Page },
    RestartEngine { has_active: bool },
}

#[derive(Debug, Clone)]
pub enum Message {
    Nav(NavMsg),
    Add(AddMsg),
    Task(TaskMsg),
    Settings(SettingsMsg),
    Engine(EngineMsg),
    Window(WindowMsg),
    Sort(SortMsg),
    Dialog(DialogMsg),
    Toast(ToastMsg),
    CopyText(String),
    OpenLink(String),
    OpenFile(PathBuf),
    RevealDir(PathBuf),
    ShowRequested,
    ActivateWindow,
    Tray(TrayMsg),
    Noop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMsg {
    ClickShow,
    ToggleWindow,
    OpenAddDialog,
    OpenSettings,
}

#[derive(Debug, Clone)]
pub enum NavMsg {
    NavigatePage(Page),
    SetTaskFilter(TaskFilter),
    SetSettingsCategory(SettingsCategory),
    SelectDetailsTab(DetailsTab),
}

#[derive(Debug, Clone)]
pub enum AddMsg {
    PathPicker(PathPickerId, PathPickerEvent),
    PathPicked(PathPickerId, Option<PathBuf>),
    SelectAddTab(AddTab),
    TorrentUpload(crate::ui::components::torrent_upload::TorrentUploadEvent),
    TorrentTreeExpand(String),
    TorrentTreeToggle(String),
    TorrentFilesSelectAll,
    TorrentFilesSelectNone,
    TorrentFilesScroll(f32),
    TorrentFilesTogglePanel,
    FileHovered,
    FileDropped(PathBuf),
    FilesHoveredLeft,
    SplitChanged(String),
    AddDownload,
    AddFieldChanged(AddField, String),
    ToggleAdvanced(bool),
    CancelAdd,
    OpenAddDialog,
    UrlEditor(iced::widget::text_editor::Action),
}

#[derive(Debug, Clone)]
pub enum TaskMsg {
    PauseTask(String),
    ResumeTask(String),
    RedownloadTask(String),
    RemoveTask(String),
    DeleteTask(String),
    StartAll,
    PauseAll,
    DeleteAll,
    RemoveAllRecords,
    ClearCompleted,
    Refresh,
    OpenTaskDetails(String),
    CloseTaskDetails,
    RefreshTaskDetails,
    OpenTaskFolder(String),
    OpenTaskFile(String),
    CopyTaskLink(String),
    DetailsTreeExpand(String),
    DetailsTreeToggle(String),
    DetailsFilesSelectAll,
    DetailsFilesSelectNone,
    DetailsFilesScroll(f32),
    DetailsFilesFlush(u64),
    CopyPath(String),
    OpenFolder(PathBuf),
}

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    SettingChanged(SettingKey, SettingValue),
    ApplySettings,
    ResetSettings,
    ApplyAndLeaveSettings,
    DiscardAndLeaveSettings,
    ThemeModeChanged(ThemeMode),
    ThemeColorChanged(iced::Color),
    LocaleChanged(Locale),
    FontFamilyChanged(String),
    RestartApp,
    UaEditor(iced::widget::text_editor::Action),
    BtTrackerEditor(iced::widget::text_editor::Action),
    SyncTrackers,
    TrackersSynced {
        fetched: Vec<String>,
        failures: Vec<(String, String)>,
    },
    TrackerSyncTimedOut,
    TrackerSourceToggled {
        source: String,
        enabled: bool,
    },
    TrackerCustomInputChanged(String),
    TrackerCustomAdd,
    TrackerCustomRemove(String),
    CheckTrackerAutoSync {
        startup: bool,
    },
    CheckUpdatesNow,
    CheckAutoUpdate {
        startup: bool,
    },
    UpdateDialogTab(usize),
    UpdateDialogCancel,
    UpdateDialogApply,
    UpdateDownloadStarted(Result<String, String>),
    UpdateResult {
        offers: Vec<crate::ui::update_dialog::UpdateOffer>,
        silent_applied: Vec<crate::ui::update_dialog::UpdateOffer>,
        errors: Vec<String>,
    },
    UpdateChangelogLoaded {
        tab: usize,
        releases: Result<Vec<crate::updater::ReleaseInfo>, String>,
    },
    SpeedUnitChanged(SettingKey, SpeedUnit),
    ToggleScheduleDaysMenu,
    ScheduleDayToggled {
        day: u8,
        enabled: bool,
    },
    ClearLogs,
    ReadOnlyHover {
        path: String,
        hovered: bool,
    },
}

#[derive(Debug, Clone)]
pub enum EngineMsg {
    Event(EngineEvent),
    RetryAria2Fetch,
    RestartEngine,
    ConfirmRestartEngine,
    EngineRestartCooldownFinished,
    EngineRestartSafetyTimeout,
}

#[derive(Debug, Clone)]
pub enum WindowMsg {
    WindowOpened(iced::window::Id),
    WindowFocused(iced::window::Id),
    WindowResized(iced::Size),
    WindowMaximized(bool),
    ClipboardRead(Option<String>),
    ClipboardParsed(Option<ClipboardPayload>, String),
    DroppedFileParsed(Option<ClipboardPayload>),
    DragWindow,
    ResizeWindow(iced::window::Direction),
    WindowAction(WindowCmd),
    CloseRequested,
    CloseDialog(CloseDialogChoice),
    HideToTray,
    ShutdownRequested,
    ShutdownTimeout,
    PersistWindowGeometry,
    FlushDirty,
}

#[derive(Debug, Clone)]
pub enum SortMsg {
    SortSelected(SortField),
    ToggleSortMenu,
    CloseSortMenu,
    ToggleSortOrder,
    SearchChanged(String),
}

#[derive(Debug, Clone)]
pub enum DialogMsg {
    RequestConfirm(ConfirmAction),
    ConfirmCancel,
    OpenAbout,
    CloseAbout,
}

#[derive(Debug, Clone)]
pub enum ToastMsg {
    DismissToast(u64),
    ToastHovered(u64),
    ToastUnhovered(u64),
    ToastTick,
}

#[derive(Debug, Clone)]
pub enum SettingValue {
    Num(u64),
    NumF(f64),
    Bool(bool),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedUnit {
    Kbps,
    Mbps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCmd {
    Minimize,
    ToggleMaximize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDialogChoice {
    Close,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Tasks,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskFilter {
    All,
    Downloading,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    General,
    Download,
    BitTorrent,
    Ed2k,
    Network,
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    AddedTime,
    Name,
    Size,
    Progress,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailsTab {
    Summary,
    Activity,
    Files,
}

impl std::fmt::Display for SortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SortField::AddedTime => "added_time",
            SortField::Name => "name",
            SortField::Size => "size",
            SortField::Progress => "progress",
            SortField::Status => "status",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingKey {
    MaxConcurrent,
    DownloadLimit,
    UploadLimit,
    Split,
    MaxConnectionPerServer,
    MinSplitSize,
    AutoFileRenaming,
    AllowOverwrite,
    Continue,
    CheckIntegrity,
    MaxDownloadLimit,
    MaxUploadLimit,
    LowestSpeedLimit,
    ProxyServer,
    ProxyUsername,
    ProxyPassword,
    MaxTries,
    RetryWait,
    ConnectTimeout,
    TrackerAutoSync,
    TrackerSyncInterval,
    SeedRatio,
    SeedTime,
    EnableDht,
    BtRequireCrypto,
    BtEnableLpd,
    EnablePeerExchange,
    BtAutoDownload,
    FileAllocation,
    DiskCache,
    EnableProxy,
    NavToTasksAfterAdd,
    TrayEnabled,
    CloseToTray,
    DeleteTorrentAfterComplete,
    CleanupCompletedOnClose,
    RemoveTaskIfFilesMissing,
    NotificationDownloadComplete,
    NotificationDownloadError,
    NotificationEngineDegraded,
    DetectClipboardOnStart,
    ClipboardHttp,
    ClipboardFtp,
    ClipboardMagnet,
    ClipboardEd2k,
    ClipboardThunder,
    ClipboardBtInfohash,
    Ed2kServer,
    Ed2kListenPort,
    Ed2kUdpListenPort,
    Ed2kUploadSlots,
    SpeedLimitScheduleEnabled,
    ScheduleStart,
    ScheduleEnd,
    AutoUpdateEnabled,
    UpdateCheckInterval,
    UpdateScope,
    Aria2SilentUpdate,
    AppLogLevel,
    EngineLogLevel,
}
