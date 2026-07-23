use fluent_templates::{langid, static_loader, LanguageIdentifier, Loader};

static_loader! {
    static LOCALES = {
        locales: "./i18n/locales",
        fallback_language: "en",
    };
}

use serde::{Deserialize, Serialize};

static EN: LanguageIdentifier = langid!("en");
static ZH_CN: LanguageIdentifier = langid!("zh-CN");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[serde(rename = "zh-CN")]
    ZhCN,
    #[serde(rename = "en")]
    EnUS,
}

impl Default for Locale {
    fn default() -> Self {
        detect_locale()
    }
}

impl Locale {
    pub fn label(self) -> &'static str {
        match self {
            Locale::ZhCN => "中文",
            Locale::EnUS => "English",
        }
    }

    pub fn langid(&self) -> &'static LanguageIdentifier {
        match self {
            Locale::ZhCN => &ZH_CN,
            Locale::EnUS => &EN,
        }
    }
}

pub fn detect_locale() -> Locale {
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default();
    let lower = lang.to_lowercase();
    if lower.starts_with("zh") {
        Locale::ZhCN
    } else {
        Locale::EnUS
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Tr {
    AppName,
    All,
    Downloading,
    Completed,
    Settings,
    Tasks,
    New,
    NoTasks,
    NoTasksHint,
    Pause,
    Resume,
    Remove,
    Waiting,
    Active,
    Paused,
    Error,
    Removed,
    NewDownload,
    UrlPlaceholder,
    OrTorrent,
    Browse,
    SaveTo,
    SplitConnections,
    Cancel,
    Download,
    SettingsTitle,
    General,
    DownloadFolder,
    MaxConcurrent,
    SpeedLimits,
    DownloadLimit,
    UploadLimit,
    About,
    Apply,
    Theme,
    ThemeDark,
    ThemeLight,
    ThemeSystem,
    Locale,
    LocaleZh,
    LocaleEn,
    ConfirmCloseTitle,
    ConfirmCloseBody,
    CloseAction,
    TrayAction,
    TrayComingSoon,
    TasksList,
    Preferences,
    StartAll,
    PauseAll,
    DeleteAll,
    ClearList,
    Refresh,
    Sort,
    SortByAdded,
    SortByName,
    SortBySize,
    SortByProgress,
    SortByStatus,
    AboutTitle,
    CloseAbout,
}

impl Tr {
    fn key(self) -> &'static str {
        match self {
            Tr::AppName => "app-name",
            Tr::All => "all",
            Tr::Downloading => "downloading",
            Tr::Completed => "completed",
            Tr::Settings => "settings",
            Tr::Tasks => "tasks",
            Tr::New => "new",
            Tr::NoTasks => "no-tasks",
            Tr::NoTasksHint => "no-tasks-hint",
            Tr::Pause => "pause",
            Tr::Resume => "resume",
            Tr::Remove => "remove",
            Tr::Waiting => "waiting",
            Tr::Active => "active",
            Tr::Paused => "paused",
            Tr::Error => "error",
            Tr::Removed => "removed",
            Tr::NewDownload => "new-download",
            Tr::UrlPlaceholder => "url-placeholder",
            Tr::OrTorrent => "or-torrent",
            Tr::Browse => "browse",
            Tr::SaveTo => "save-to",
            Tr::SplitConnections => "split-connections",
            Tr::Cancel => "cancel",
            Tr::Download => "download",
            Tr::SettingsTitle => "settings-title",
            Tr::General => "general",
            Tr::DownloadFolder => "download-folder",
            Tr::MaxConcurrent => "max-concurrent",
            Tr::SpeedLimits => "speed-limits",
            Tr::DownloadLimit => "download-limit",
            Tr::UploadLimit => "upload-limit",
            Tr::About => "about",
            Tr::Apply => "apply",
            Tr::Theme => "theme",
            Tr::ThemeDark => "theme-dark",
            Tr::ThemeLight => "theme-light",
            Tr::ThemeSystem => "theme-system",
            Tr::Locale => "locale",
            Tr::LocaleZh => "locale-zh",
            Tr::LocaleEn => "locale-en",
            Tr::ConfirmCloseTitle => "confirm-close-title",
            Tr::ConfirmCloseBody => "confirm-close-body",
            Tr::CloseAction => "close-action",
            Tr::TrayAction => "tray-action",
            Tr::TrayComingSoon => "tray-coming-soon",
            Tr::TasksList => "tasks-list",
            Tr::Preferences => "preferences",
            Tr::StartAll => "start-all",
            Tr::PauseAll => "pause-all",
            Tr::DeleteAll => "delete-all",
            Tr::ClearList => "clear-list",
            Tr::Refresh => "refresh",
            Tr::Sort => "sort",
            Tr::SortByAdded => "sort-by-added",
            Tr::SortByName => "sort-by-name",
            Tr::SortBySize => "sort-by-size",
            Tr::SortByProgress => "sort-by-progress",
            Tr::SortByStatus => "sort-by-status",
            Tr::AboutTitle => "about-title",
            Tr::CloseAbout => "close-about",
        }
    }
}

pub struct Fluent {
    pub locale: Locale,
}

impl Fluent {
    pub fn new(locale: Locale) -> Self {
        Self { locale }
    }

    pub fn get(&self, key: Tr) -> String {
        LOCALES.lookup(self.locale.langid(), key.key())
    }
}
