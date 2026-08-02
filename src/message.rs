use std::path::PathBuf;

use crate::clipboard_watch::ClipboardPayload;
use crate::engine::EngineEvent;
use crate::i18n::Locale;
use crate::task::TaskStatus;
use crate::ui::components::path_picker::PathPickerEvent;
use crate::ui::components::toast::Toast;
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
    LeaveSettings { target: Page },
}

#[derive(Debug, Clone)]
pub enum Message {
    NavigatePage(Page),
    SetTaskFilter(TaskFilter),
    SetSettingsCategory(SettingsCategory),
    PathPicker(PathPickerId, PathPickerEvent),
    PathPicked(PathPickerId, Option<PathBuf>),
    SelectAddTab(AddTab),
    TorrentUpload(crate::ui::components::torrent_upload::TorrentUploadEvent),
    TorrentTreeExpand(String),
    TorrentTreeToggle(String),
    TorrentFilesSelectAll,
    TorrentFilesSelectNone,
    DetailsTreeExpand(String),
    DetailsTreeToggle(String),
    DetailsFilesSelectAll,
    DetailsFilesSelectNone,
    TorrentFilesScroll(f32),
    TorrentFilesTogglePanel,
    DetailsFilesScroll(f32),
    DetailsFilesFlush(u64),
    FileHovered(PathBuf),
    FileDropped(PathBuf),
    FilesHoveredLeft,
    CopyPath(String),
    SplitChanged(String),
    AddDownload,
    AddFieldChanged(AddField, String),
    ToggleAdvanced(bool),
    CancelAdd,
    OpenAddDialog,
    PauseTask(String),
    ResumeTask(String),
    RemoveTask(String),
    DeleteTask(String),
    StartAll,
    PauseAll,
    DeleteAll,
    RemoveAllRecords,
    ClearCompleted,
    Refresh,
    SortSelected(SortField),
    ToggleSortMenu,
    CloseSortMenu,
    ToggleSortOrder,
    SearchChanged(String),
    SettingChanged(SettingKey, String),
    ApplySettings,
    ResetSettings,
    OpenAbout,
    CloseAbout,
    Engine(EngineEvent),

    WindowOpened(iced::window::Id),
    WindowFocused(iced::window::Id),
    ClipboardRead(Option<String>),
    ClipboardParsed(Option<ClipboardPayload>, String),
    DragWindow,
    ResizeWindow(iced::window::Direction),
    WindowAction(WindowCmd),
    CloseRequested,
    CloseDialog(CloseDialogChoice),
    ShutdownRequested,
    ShutdownTimeout,

    ThemeModeChanged(ThemeMode),
    ThemeColorChanged(iced::Color),
    LocaleChanged(Locale),

    CheckAria2Update,
    RetryAria2Fetch,
    RestartEngine,
    SetAutoCheck(bool),
    CheckMissingFiles,

    OpenTaskDetails(String),
    CloseTaskDetails,
    RefreshTaskDetails,
    FlushDirty,
    WindowResized(iced::Size),
    WindowMaximized(bool),
    PersistWindowGeometry,
    SelectDetailsTab(DetailsTab),
    OpenTaskFolder(String),
    OpenTaskFile(String),
    CopyTaskLink(String),
    Noop,

    UrlEditor(iced::widget::text_editor::Action),
    UaEditor(iced::widget::text_editor::Action),

    RequestConfirm(ConfirmAction),
    ConfirmCancel,
    ApplyAndLeaveSettings,
    DiscardAndLeaveSettings,
    SpeedUnitChanged(SettingKey, SpeedUnit),
    ShowToast(Toast),
    DismissToast(u64),
    ToastHovered(u64),
    ToastUnhovered(u64),
    ToastTick,
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
    MinimizeToTray,
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
    ThemeMode,
    Locale,
    MaxConnectionPerServer,
    MinSplitSize,
    AutoFileRenaming,
    AllowOverwrite,
    Continue,
    CheckIntegrity,
    MaxDownloadLimit,
    MaxUploadLimit,
    LowestSpeedLimit,
    UserAgent,
    ProxyServer,
    ProxyUsername,
    ProxyPassword,
    MaxTries,
    RetryWait,
    ConnectTimeout,
    BtTracker,
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
    DeleteTorrentAfterComplete,
    CleanupCompletedOnClose,
    RemoveTaskIfFilesMissing,
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
}

impl TaskStatus {
    pub fn from_engine(status: &str) -> Self {
        match status {
            "waiting" => TaskStatus::Waiting,
            "active" => TaskStatus::Active,
            "paused" => TaskStatus::Paused,
            "complete" => TaskStatus::Completed,
            "error" => TaskStatus::Error,
            "removed" => TaskStatus::Removed,
            _ => TaskStatus::Waiting,
        }
    }
}
