use std::path::PathBuf;

use crate::engine::EngineEvent;
use crate::i18n::Locale;
use crate::task::TaskStatus;
use crate::ui::theme::ThemeMode;

#[derive(Debug, Clone)]
pub enum Message {
    NavigatePage(Page),
    SetTaskFilter(TaskFilter),
    SetSettingsCategory(SettingsCategory),
    AddUrlChanged(String),
    SaveDirChanged(String),
    BrowseSaveDir,
    BrowseTorrent,
    FilePicked(FileKind, Option<PathBuf>),
    SplitChanged(String),
    AddDownload,
    CancelAdd,
    OpenAddDialog,
    PauseTask(String),
    ResumeTask(String),
    RemoveTask(String),
    StartAll,
    PauseAll,
    DeleteAll,
    ClearCompleted,
    Refresh,
    SortSelected(SortField),
    ToggleSortMenu,
    CloseSortMenu,
    ToggleSortOrder,
    SettingChanged(SettingKey, String),
    ApplySettings,
    OpenAbout,
    CloseAbout,
    Engine(EngineEvent),

    WindowOpened(iced::window::Id),
    DragWindow,
    WindowAction(WindowCmd),
    CloseRequested,
    CloseDialog(CloseDialogChoice),

    ThemeModeChanged(ThemeMode),
    LocaleChanged(Locale),

    CheckAria2Update,
    RetryAria2Fetch,
    RestartEngine,
    SetAutoCheck(bool),

    UaEditor(iced::widget::text_editor::Action),
    HeadersEditor(iced::widget::text_editor::Action),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    SaveDir,
    Torrent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    DownloadDir,
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
    Headers,
    AllProxy,
    MaxTries,
    RetryWait,
    ConnectTimeout,
    BtTracker,
    SeedRatio,
    SeedTime,
    EnableDht,
    BtRequireCrypto,
    EnableProxy,
    NavToTasksAfterAdd,
    DeleteTorrentAfterComplete,
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
