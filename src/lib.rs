pub mod app;
pub mod config;
pub mod core;
pub mod engine;
pub mod integration;
pub mod platform;
pub mod storage;
pub mod ui;
pub mod update;
pub mod updaters;

pub const APP_ID: &str = "remotrix";

pub mod db {
    pub use crate::storage::db::*;
}

pub mod ed2k_bootstrap {
    pub use crate::storage::ed2k_bootstrap::*;
}

pub mod torrent_meta {
    pub use crate::storage::torrent_meta::*;
}

pub mod trackers {
    pub use crate::storage::trackers::*;
}

pub mod message {
    pub use crate::core::message::*;
}

pub mod task {
    pub use crate::core::task::*;
}

pub mod i18n {
    pub use crate::core::i18n::*;
}

pub mod logging {
    pub use crate::core::logging::*;
}

pub mod scheduler {
    pub use crate::core::scheduler::*;
}

pub mod notify {
    pub use crate::platform::notify::*;
}

pub mod tray {
    pub use crate::platform::tray::*;
}

#[cfg(target_os = "windows")]
pub mod tray_watchdog {
    pub use crate::platform::tray_watchdog::*;
}

#[cfg(target_os = "windows")]
pub mod win_toast {
    pub use crate::platform::win_toast::*;
}

pub mod autostart {
    pub use crate::platform::autostart::*;
}

pub mod power {
    pub use crate::platform::power::*;
}

pub mod port_guard {
    pub use crate::platform::port_guard::*;
}

pub mod shutdown {
    pub use crate::platform::shutdown::*;
}

pub mod extension_api {
    pub use crate::integration::extension_api::*;
}

pub mod clipboard_watch {
    pub use crate::integration::clipboard_watch::*;
}

pub mod updater {
    pub use crate::updaters::updater::*;
}

pub mod app_updater {
    pub use crate::updaters::app_updater::*;
}

pub mod aria2_fetcher {
    pub use crate::updaters::aria2_fetcher::*;
}

pub mod download {
    pub use crate::updaters::download::*;
}
