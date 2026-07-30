#![allow(dead_code)]

mod app;
mod aria2_fetcher;
mod config;
mod db;
mod engine;
mod i18n;
mod message;
mod task;
mod ui;
mod updater;

fn main() -> iced::Result {
    let _log_guard = init_tracing();

    let cfg = crate::config::load();
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

        .default_font(iced::Font::with_name("HarmonyOS Sans SC"))
        .window(iced::window::Settings {
            size: iced::Size::new(w, h),
            maximized: cfg.window_maximized,
            icon: load_icon(),
            decorations: false,
            exit_on_close_request: false,
            min_size: Some(iced::Size::new(800.0, 560.0)),
            ..Default::default()
        })
        .run()
}

fn load_icon() -> Option<iced::window::Icon> {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    iced::window::icon::from_rgba(img.into_raw(), w, h).ok()
}

fn init_tracing() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,remotrix=debug"));

    match crate::config::log_dir() {
        Some(dir) => {
            let file_appender = tracing_appender::rolling::daily(&dir, "remotrix.log");
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            let _ = tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().with_target(false))
                .with(
                    fmt::layer()
                        .with_ansi(false)
                        .with_target(true)
                        .with_writer(non_blocking),
                )
                .try_init();
            tracing::info!("remotrix starting");
            Some(guard)
        }
        None => {
            let _ = tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().with_target(false))
                .try_init();
            tracing::warn!("log file disabled: log_dir unavailable");
            tracing::info!("remotrix starting");
            None
        }
    }
}
