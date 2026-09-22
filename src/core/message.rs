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
    Metalink,
    Ed2kServerList,
    Ed2kNodeList,
    CustomAria2Dir,
    CustomAppDataDir,
    CustomLogDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddTab {
    Url,
    Torrent,
    Metalink,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CtxTarget {
    Search,
    AddUrl,
    AddOut,
    AddAdvanced(AddField),
    DetailsAdvanced(AddField),
    SettingsUa,
    SettingsBtTracker,
    SettingsCustomTracker,
}

impl PathPickerId {
    pub fn history_key(self) -> &'static str {
        match self {
            Self::DownloadDir => "download_dir",
            Self::SaveDir => "save_dir",
            Self::Torrent => "torrent",
            Self::Metalink => "metalink",
            Self::Ed2kServerList => "ed2k_server_list",
            Self::Ed2kNodeList => "ed2k_node_list",
            Self::CustomAria2Dir => "custom_aria2_dir",
            Self::CustomAppDataDir => "custom_app_data_dir",
            Self::CustomLogDir => "custom_log_dir",
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
    UnsavedOnClose,
    RestartEngine { has_active: bool },
    Shutdown { seconds_left: u32 },
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
    CtxOpen(CtxTarget),
    CtxClipboardRead(Option<String>),
    CtxCopy(String),
    CtxPaste(CtxTarget, String),
    CtxClose,
    CursorMoved(iced::Point),
    OpenLink(String),
    #[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
    OpenFile(PathBuf),
    #[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
    RevealDir(PathBuf),
    ShowRequested,
    #[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
    ActivateWindow,
    Tray(TrayMsg),
    ProgressAnim(String, crate::ui::animation::Event<f32>),
    CardAnim(String, crate::ui::animation::Event<f32>),
    HudAnim(crate::ui::animation::Event<f32>),
    BorderAnim(crate::ui::animation::Event<f32>),
    PillAnim(crate::ui::animation::Event<f32>),
    AddDialogAnim(crate::ui::animation::Event<f32>),
    AboutDialogAnim(crate::ui::animation::Event<f32>),
    DetailsAnim(crate::ui::animation::Event<f32>),
    ConfirmAnim(crate::ui::animation::Event<f32>),
    UpdateDialogAnim(crate::ui::animation::Event<f32>),
    CloseDialogAnim(crate::ui::animation::Event<f32>),
    Extension(ExtensionMsg),
    Shutdown(ShutdownMsg),
    ScrollAnimTick,
    ScrollableScrolled(iced::widget::Id),
    SpeedLimitDebounceTick,
    BorderAnimTick,
    Noop,
}

#[derive(Debug, Clone)]
pub enum ShutdownMsg {
    ToggleCard,
    CloseCard,
    SetAfterComplete(bool),
    SetTimerEnabled(bool),
    SetTimerMinutes(u32),
    ShutdownTick,
    ShutdownNow,
    ShutdownExecuted { ok: bool, error: Option<String> },
}

#[derive(Debug, Clone)]
pub enum ExtensionMsg {
    ShowAddDialog(crate::extension_api::ExternalDownload),
    GenerateSecret,
    ServerRestarted { ok: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMsg {
    ClickShow,
    ToggleWindow,
    OpenAddDialog,
    OpenSettings,
    #[allow(dead_code)]
    WatchdogReaddRequested,
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
    MetalinkUpload(crate::ui::components::torrent_upload::TorrentUploadEvent),
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
    AddFromEd2kResult(Vec<String>),
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
    DetailsAdvancedFieldChanged(AddField, String),
    DetailsAdvancedSave,
    MetadataProbeResult {
        gid: String,
        incoming: String,
        size: Option<u64>,
        name: Option<String>,
    },
    OpenFolder(PathBuf),
}

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    SettingChanged(SettingKey, SettingValue),
    ApplySettings,
    ResetSettings,
    ApplyAndLeaveSettings,
    DiscardAndLeaveSettings,
    ApplyAndClose,
    DiscardAndClose,
    ThemeModeChanged(ThemeMode),
    ThemeColorChanged(iced::Color),
    CustomColorPickerToggle,
    CustomColorHsvChanged(crate::ui::components::color_picker::HsvColor),
    CustomColorHexChanged(String),
    CustomColorApply,
    CustomColorCancel,
    CustomColorHistorySelect(String),
    LocaleChanged(Locale),
    FontFamilyChanged(String),
    FontPickerToggle,
    FontPickerClose,
    FontPickerQueryChanged(String),
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
    CheckPendingUpdates,
    UpdateDialogTab(usize),
    UpdateDialogCancel,
    UpdateDialogApply,
    RetryChangelog(usize),
    AppUpdateProgress {
        downloaded: u64,
        total: u64,
    },
    AppUpdateReady {
        outcome: crate::app_updater::AppUpdateOutcome,
    },
    AppUpdateFailed {
        error: String,
    },
    ApplyAppUpdate {
        outcome: crate::app_updater::AppUpdateOutcome,
    },
    UpdateResult {
        offers: Vec<crate::ui::update_dialog::UpdateOffer>,
        silent_applied: Vec<crate::ui::update_dialog::UpdateOffer>,
        errors: Vec<String>,
        pending_restart_engine: bool,
        pending_app_update: Option<crate::app_updater::AppUpdateOutcome>,
    },
    UpdateChangelogLoaded {
        tab: usize,
        releases: Result<Vec<crate::updater::ReleaseInfo>, String>,
    },
    SpeedUnitChanged(SettingKey, SpeedUnit),
    Ed2kSearchSubmit,
    Ed2kSearchCancel,
    Ed2kBootstrapSyncNow,
    ToggleScheduleDaysMenu,
    ScheduleDayToggled {
        day: u8,
        enabled: bool,
    },
    ClearLogs,
    RestoreDefaultPath(PathPickerId),
}

#[derive(Debug, Clone)]
pub enum EngineMsg {
    Event(Box<EngineEvent>),
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
    CloseDialogTrayPrefChanged(bool),
    HideToTray,
    ShutdownRequested,
    ShutdownTimeout,
    PersistWindowGeometry,
    FlushDirty,
    #[allow(dead_code)]
    ResizeTick,
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
    OpenSpeedLimitPopover,
    CloseSpeedLimitPopover,
    SpeedLimitChanged(SettingKey, u64),
    SpeedLimitUnitChanged(SettingKey, SpeedUnit),
}

#[derive(Debug, Clone)]
pub enum ToastMsg {
    DismissToast(u64),
    ToastHovered(u64),
    ToastUnhovered(u64),
    ToastActionPressed(u64),
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

impl Page {
    pub fn index(&self) -> usize {
        match self {
            Page::Tasks => 0,
            Page::Settings => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskFilter {
    All,
    Downloading,
    Completed,
    Failed,
}

impl TaskFilter {
    pub fn index(&self) -> usize {
        match self {
            TaskFilter::All => 0,
            TaskFilter::Downloading => 1,
            TaskFilter::Completed => 2,
            TaskFilter::Failed => 3,
        }
    }
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

impl SettingsCategory {
    pub fn index(&self) -> usize {
        match self {
            SettingsCategory::General => 0,
            SettingsCategory::Download => 1,
            SettingsCategory::BitTorrent => 2,
            SettingsCategory::Ed2k => 3,
            SettingsCategory::Network => 4,
            SettingsCategory::Advanced => 5,
        }
    }
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
    Advanced,
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
    AutoStart,
    StartHiddenOnAutostart,
    CloseToTray,
    DeleteTorrentAfterComplete,
    CleanupCompletedOnClose,
    RemoveTaskIfFilesMissing,
    NotificationDownloadComplete,
    NotificationDownloadError,
    NotificationEngineDegraded,
    NotificationDownloadAdded,
    PreventSleep,
    ExtensionApiEnabled,
    ExtensionApiPort,
    ExtensionApiSecret,
    ExtensionAutoSubmit,
    DetectClipboardOnStart,
    ClipboardHttp,
    ClipboardFtp,
    ClipboardMagnet,
    ClipboardEd2k,
    ClipboardThunder,
    ClipboardBtInfohash,
    ClipboardWebpageFilter,
    Ed2kServer,
    Ed2kListenPort,
    Ed2kUdpListenPort,
    Ed2kUploadSlots,
    RpcListenPort,
    SpeedLimitScheduleEnabled,
    ScheduleStart,
    ScheduleEnd,
    FollowMetalink,
    Ed2kServerMetUrl,
    Ed2kNodesDatUrl,
    Ed2kBootstrapAutoSync,
    Ed2kBootstrapSyncInterval,
    Ed2kSearchKeyword,
    Ed2kSearchFileType,
    Ed2kSearchMinSources,
    Ed2kSearchTimeout,
    AutoUpdateEnabled,
    UpdateCheckInterval,
    UpdateScope,
    SilentUpdateScope,
    BetaChannel,
    AppLogLevel,
    EngineLogLevel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_index_bijective() {
        assert_eq!(Page::Tasks.index(), 0);
        assert_eq!(Page::Settings.index(), 1);
    }

    #[test]
    fn task_filter_index_covers_all_variants() {
        let all = vec![
            TaskFilter::All.index(),
            TaskFilter::Downloading.index(),
            TaskFilter::Completed.index(),
            TaskFilter::Failed.index(),
        ];
        let mut deduped = all.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(all.len(), 4);
        assert_eq!(deduped.len(), 4, "duplicate index for some variant");
    }

    #[test]
    fn task_filter_index_bijective() {
        assert_eq!(TaskFilter::All.index(), 0);
        assert_eq!(TaskFilter::Downloading.index(), 1);
        assert_eq!(TaskFilter::Completed.index(), 2);
        assert_eq!(TaskFilter::Failed.index(), 3);
    }

    #[test]
    fn settings_cat_index_covers_all_variants() {
        let all = vec![
            SettingsCategory::General.index(),
            SettingsCategory::Download.index(),
            SettingsCategory::BitTorrent.index(),
            SettingsCategory::Ed2k.index(),
            SettingsCategory::Network.index(),
            SettingsCategory::Advanced.index(),
        ];
        let mut deduped = all.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(all.len(), 6);
        assert_eq!(deduped.len(), 6, "duplicate index for some category");
    }

    #[test]
    fn settings_cat_index_bijective() {
        assert_eq!(SettingsCategory::General.index(), 0);
        assert_eq!(SettingsCategory::Download.index(), 1);
        assert_eq!(SettingsCategory::BitTorrent.index(), 2);
        assert_eq!(SettingsCategory::Ed2k.index(), 3);
        assert_eq!(SettingsCategory::Network.index(), 4);
        assert_eq!(SettingsCategory::Advanced.index(), 5);
    }
}
