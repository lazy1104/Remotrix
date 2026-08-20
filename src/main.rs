#![windows_subsystem = "windows"]

mod app;
mod app_updater;
mod aria2_fetcher;
mod autostart;
mod clipboard_watch;
mod config;
mod db;
mod ed2k_bootstrap;
mod engine;
mod extension_api;
mod i18n;
mod logging;
mod message;
mod notify;
mod port_guard;
mod power;
mod scheduler;
mod shutdown;
mod task;
mod torrent_meta;
mod trackers;
mod tray;
mod ui;
mod update;
mod updater;
#[cfg(target_os = "windows")]
mod win_toast;

const APP_ID: &str = "remotrix";

fn main() -> iced::Result {
    let _log_guard = crate::logging::init();

    crate::config::install_desktop_file();
    #[cfg(target_os = "windows")]
    crate::win_toast::init();

    if std::env::var_os("REMOTRIX_RESTART").is_none()
        && app_single_instance::notify_if_running(APP_ID)
    {
        tracing::info!("another instance is running; exiting");
        std::process::exit(0);
    }

    let cfg = crate::config::load();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        app_log_level = %cfg.log.app_level,
        "remotrix starting"
    );

    if let Err(e) = crate::autostart::set_enabled(cfg.autostart_enabled) {
        tracing::warn!(error = %e, "autostart sync failed");
    }
    let hidden_start = crate::autostart::is_autostart_launch() && cfg.start_hidden_on_autostart;

    let w = cfg.window_width.max(800.0);
    let h = cfg.window_height.max(560.0);

    iced::application(app::init, app::update, app::view)
        .title(app::app_title as fn(&app::Remotrix) -> String)
        .theme(app::theme as fn(&app::Remotrix) -> iced::Theme)
        .subscription(
            app::subscription as fn(&app::Remotrix) -> iced::Subscription<crate::message::Message>,
        )
        .font(crate::ui::icon::FONT as &[_])
        .font(include_bytes!("../fonts/HarmonyOS_Sans_SC_Regular.ttf") as &[_])
        .font(iced_aw::ICED_AW_FONT_BYTES)
        .default_font(crate::ui::theme::font_from_family(&cfg.font_family))
        .window(iced::window::Settings {
            size: iced::Size::new(w, h),
            maximized: cfg.window_maximized,
            icon: load_icon(),
            decorations: false,
            exit_on_close_request: false,
            visible: !hidden_start,
            min_size: Some(iced::Size::new(800.0, 560.0)),
            platform_specific: platform_specific_settings(),
            ..Default::default()
        })
        .antialiasing(true)
        .run()
}

#[cfg(target_os = "linux")]
fn platform_specific_settings() -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific {
        application_id: crate::APP_ID.to_string(),
        ..Default::default()
    }
}

#[cfg(not(target_os = "linux"))]
fn platform_specific_settings() -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific::default()
}

fn load_icon() -> Option<iced::window::Icon> {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    iced::window::icon::from_rgba(img.into_raw(), w, h).ok()
}
