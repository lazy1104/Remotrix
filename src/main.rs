#![allow(dead_code)]

mod app;
mod aria2_fetcher;
mod clipboard_watch;
mod config;
mod db;
mod engine;
mod i18n;
mod logging;
mod message;
mod scheduler;
mod task;
mod torrent_meta;
mod trackers;
mod ui;
mod updater;

fn main() -> iced::Result {
    let _log_guard = crate::logging::init();

    let cfg = crate::config::load();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        app_log_level = %cfg.log.app_level,
        "remotrix starting"
    );
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
            min_size: Some(iced::Size::new(800.0, 560.0)),
            ..Default::default()
        })
        .antialiasing(true)
        .run()
}

fn load_icon() -> Option<iced::window::Icon> {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    iced::window::icon::from_rgba(img.into_raw(), w, h).ok()
}
