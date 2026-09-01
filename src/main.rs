#![windows_subsystem = "windows"]

use remotrix::app;
use remotrix::autostart;
use remotrix::config;
use remotrix::logging;
use remotrix::message;
use remotrix::ui;
use remotrix::APP_ID;

fn main() -> iced::Result {
    let mut cfg = config::load();

    if let Err(e) = config::migrate_paths(&mut cfg) {
        eprintln!("remotrix: path migration failed: {e}");
    }
    config::save(&cfg);

    let _log_guard = logging::init();

    config::install_desktop_file();
    #[cfg(target_os = "windows")]
    win_toast::init();

    if std::env::var_os("REMOTRIX_RESTART").is_none()
        && app_single_instance::notify_if_running(APP_ID)
    {
        tracing::info!("another instance is running; exiting");
        std::process::exit(0);
    }
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        app_log_level = %cfg.log.app_level,
        "remotrix starting"
    );

    if let Err(e) = autostart::set_enabled(cfg.autostart_enabled) {
        tracing::warn!(error = %e, "autostart sync failed");
    }
    let hidden_start = autostart::is_autostart_launch() && cfg.start_hidden_on_autostart;

    let w = cfg.window_width.max(800.0);
    let h = cfg.window_height.max(560.0);

    iced::application(app::init, app::update, app::view)
        .title(app::app_title as fn(&app::Remotrix) -> String)
        .theme(app::theme as fn(&app::Remotrix) -> iced::Theme)
        .subscription(
            app::subscription as fn(&app::Remotrix) -> iced::Subscription<message::Message>,
        )
        .font(ui::icon::FONT as &[_])
        .font(include_bytes!("../fonts/HarmonyOS_Sans_SC_Regular.ttf") as &[_])
        .font(iced_aw::ICED_AW_FONT_BYTES)
        .default_font(ui::theme::font_from_family(&cfg.font_family))
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
        application_id: APP_ID.to_string(),
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
