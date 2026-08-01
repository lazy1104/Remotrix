use std::collections::HashMap;

use iced::widget::{
    button, column, container, pick_list, row, text, text_editor, text_input, toggler,
};
use iced::{Alignment, Element, Length};

use crate::config::Settings;
use crate::i18n::{Fluent, Locale, Tr};
use crate::message::{Message, PathPickerId, SettingKey, SettingsCategory, SpeedUnit};
use iced::Color;

use crate::ui::components::number_stepper::number_stepper;
use crate::ui::components::path_picker::{PathPicker, PathPickerEvent};
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::theme;

#[derive(Debug, Clone)]
pub struct SettingsUiState {
    pub download_picker: PathPicker,
    pub speed_units: HashMap<SettingKey, SpeedUnit>,
}

impl SettingsUiState {
    pub fn new(settings: &Settings) -> Self {
        let mut speed_units = HashMap::new();
        for key in &[
            SettingKey::DownloadLimit,
            SettingKey::UploadLimit,
            SettingKey::MaxDownloadLimit,
            SettingKey::MaxUploadLimit,
            SettingKey::LowestSpeedLimit,
        ] {
            speed_units.insert(*key, SpeedUnit::Kbps);
        }
        Self {
            download_picker: PathPicker::folder(
                settings.download_dir.to_string_lossy().into_owned(),
                true,
            ),
            speed_units,
        }
    }
}

#[derive(Debug, Clone)]
struct Labeled<T> {
    value: T,
    label: String,
}

impl<T> std::fmt::Display for Labeled<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

impl<T> PartialEq for Labeled<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    category: SettingsCategory,
    applied_settings: &'a Settings,
    aria2_version: Option<&'a str>,
    aria2_check_msg: Option<&'a str>,
    aria2_status: Option<(&'a str, &'a str)>,
    aria2_fetch_error: Option<&'a str>,
    update_pending: Option<&'a str>,
    ua_editor: &'a text_editor::Content,
    path_history: &'a HashMap<String, Vec<String>>,
) -> Element<'a, Message> {
    let accent = theme::accent(theme);
    let dirty = !settings.apply_fields_equal(applied_settings);
    let content = match category {
        SettingsCategory::General => general_view(fluent, theme, settings),
        SettingsCategory::Download => {
            download_view(fluent, theme, settings, settings_ui, path_history)
        }
        SettingsCategory::BitTorrent => bittorrent_view(fluent, settings, accent),
        SettingsCategory::Ed2k => ed2k_view(fluent),
        SettingsCategory::Network => network_view(fluent, settings, ua_editor, accent),
        SettingsCategory::Advanced => advanced_view(
            fluent,
            theme,
            settings,
            aria2_version,
            aria2_check_msg,
            aria2_status,
            aria2_fetch_error,
            update_pending,
        ),
    };

    let mut body = column![]
        .push(text(settings_title(fluent, category)).size(22))
        .push(iced::widget::Space::new().height(Length::Fixed(20.0)))
        .push(slim_scrollable(content).height(Length::Fill));

    let mut actions = row![].spacing(12).width(Length::Fill);
    actions = actions.push(
        button(text(fluent.get(Tr::Apply)).size(14))
            .on_press_maybe(if dirty {
                Some(Message::ApplySettings)
            } else {
                None
            })
            .padding([10, 24])
            .style(theme::style::button::primary()),
    );
    if dirty {
        actions = actions.push(
            button(text(fluent.get(Tr::Reset)).size(14))
                .on_press(Message::ResetSettings)
                .padding([10, 24])
                .style(theme::style::button::secondary()),
        );
    }
    body = body.push(actions);

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([24, 28])
        .into()
}

fn settings_title(fluent: &Fluent, category: SettingsCategory) -> String {
    let key = match category {
        SettingsCategory::General => Tr::General,
        SettingsCategory::Download => Tr::DownloadCategory,
        SettingsCategory::BitTorrent => Tr::BitTorrent,
        SettingsCategory::Ed2k => Tr::Ed2k,
        SettingsCategory::Network => Tr::Network,
        SettingsCategory::Advanced => Tr::Advanced,
    };
    fluent.get(key)
}

fn general_view<'a>(
    fluent: &'a Fluent,
    theme: &iced::Theme,
    settings: &'a Settings,
) -> Element<'a, Message> {
    let accent = theme::accent(theme);
    let light_opts: Vec<Labeled<String>> = theme::light_themes()
        .into_iter()
        .map(|(name, display)| Labeled {
            value: name,
            label: display,
        })
        .collect();
    let dark_opts: Vec<Labeled<String>> = theme::dark_themes()
        .into_iter()
        .map(|(name, display)| Labeled {
            value: name,
            label: display,
        })
        .collect();
    let mode_opts = vec![
        Labeled {
            value: crate::ui::theme::ThemeMode::Dark,
            label: fluent.get(Tr::ThemeDark),
        },
        Labeled {
            value: crate::ui::theme::ThemeMode::Light,
            label: fluent.get(Tr::ThemeLight),
        },
        Labeled {
            value: crate::ui::theme::ThemeMode::System,
            label: fluent.get(Tr::ThemeSystem),
        },
    ];

    column![]
        .spacing(4)
        .push(group_title(fluent, Tr::Appearance, accent))
        .push(labeled_pick(
            fluent,
            fluent.get(Tr::LightTheme),
            light_opts,
            Some(settings.light_theme.clone()),
            |opt| Message::LightThemeChanged(opt.value),
        ))
        .push(labeled_pick(
            fluent,
            fluent.get(Tr::DarkTheme),
            dark_opts,
            Some(settings.dark_theme.clone()),
            |opt| Message::DarkThemeChanged(opt.value),
        ))
        .push(labeled_pick(
            fluent,
            fluent.get(Tr::ColorMode),
            mode_opts,
            Some(settings.theme_mode),
            |opt| Message::ThemeModeChanged(opt.value),
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Locale, accent))
        .push(labeled_pick(
            fluent,
            fluent.get(Tr::Locale),
            vec![
                Labeled {
                    value: Locale::ZhCN,
                    label: fluent.get(Tr::LocaleZh),
                },
                Labeled {
                    value: Locale::EnUS,
                    label: fluent.get(Tr::LocaleEn),
                },
            ],
            Some(settings.locale),
            |opt| Message::LocaleChanged(opt.value),
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Clipboard, accent))
        .push(labeled_toggle(
            fluent.get(Tr::DetectClipboardOnStart),
            settings.detect_clipboard_on_start,
            SettingKey::DetectClipboardOnStart,
        ))
        .into()
}

fn download_view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    path_history: &'a HashMap<String, Vec<String>>,
) -> Element<'a, Message> {
    let accent = theme::accent(theme);
    let download_hist: &[String] = path_history
        .get("download_dir")
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    column![]
        .spacing(4)
        .push(group_title(fluent, Tr::DownloadFolder, accent))
        .push(
            row![]
                .push(
                    text(fluent.get(Tr::DownloadFolder))
                        .size(13)
                        .width(Length::Fixed(200.0)),
                )
                .push(
                    settings_ui
                        .download_picker
                        .view(fluent, theme, download_hist, |e| {
                            Message::PathPicker(PathPickerId::DownloadDir, e)
                        }),
                )
                .height(Length::Fixed(36.0))
                .align_y(Alignment::Center),
        )
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::ConnectionSegment, accent))
        .push(labeled_number(
            fluent.get(Tr::MaxConcurrent),
            &settings.max_concurrent,
            1..=u32::MAX,
            1,
            SettingKey::MaxConcurrent,
        ))
        .push(labeled_number(
            fluent.get(Tr::Split),
            &settings.split,
            1..=128u16,
            1,
            SettingKey::Split,
        ))
        .push(labeled_number(
            fluent.get(Tr::MaxConnectionPerServer),
            &settings.aria2.max_connection_per_server,
            1..=16u32,
            1,
            SettingKey::MaxConnectionPerServer,
        ))
        .push(setting_row(
            fluent.get(Tr::MinSplitSize),
            row![]
                .spacing(8)
                .push(number_stepper(
                    &settings.aria2.min_split_size_mb,
                    1..=1024u64,
                    1,
                    move |v| Message::SettingChanged(SettingKey::MinSplitSize, v.to_string()),
                    Length::Fixed(160.0),
                ))
                .align_y(Alignment::Center)
                .into(),
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::ResumeRetry, accent))
        .push(labeled_number(
            fluent.get(Tr::MaxTries),
            &settings.aria2.max_tries,
            0..=u32::MAX,
            1,
            SettingKey::MaxTries,
        ))
        .push(labeled_number(
            fluent.get(Tr::RetryWait),
            &settings.aria2.retry_wait,
            0..=u32::MAX,
            1,
            SettingKey::RetryWait,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::Continue),
            settings.aria2.r#continue,
            SettingKey::Continue,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::CheckIntegrity),
            settings.aria2.check_integrity,
            SettingKey::CheckIntegrity,
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::File, accent))
        .push(labeled_toggle(
            fluent.get(Tr::AutoFileRenaming),
            settings.aria2.auto_file_renaming,
            SettingKey::AutoFileRenaming,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::AllowOverwrite),
            settings.aria2.allow_overwrite,
            SettingKey::AllowOverwrite,
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::SpeedLimits, accent))
        .push({
            let unit = settings_ui
                .speed_units
                .get(&SettingKey::DownloadLimit)
                .copied()
                .unwrap_or(SpeedUnit::Kbps);
            speed_labeled_input(
                fluent.get(Tr::DownloadLimit),
                &settings.download_limit_kb,
                unit,
                move |v| Message::SettingChanged(SettingKey::DownloadLimit, v.to_string()),
                move |u| Message::SpeedUnitChanged(SettingKey::DownloadLimit, u),
            )
        })
        .push({
            let unit = settings_ui
                .speed_units
                .get(&SettingKey::UploadLimit)
                .copied()
                .unwrap_or(SpeedUnit::Kbps);
            speed_labeled_input(
                fluent.get(Tr::UploadLimit),
                &settings.upload_limit_kb,
                unit,
                move |v| Message::SettingChanged(SettingKey::UploadLimit, v.to_string()),
                move |u| Message::SpeedUnitChanged(SettingKey::UploadLimit, u),
            )
        })
        .push({
            let unit = settings_ui
                .speed_units
                .get(&SettingKey::MaxDownloadLimit)
                .copied()
                .unwrap_or(SpeedUnit::Kbps);
            speed_labeled_input(
                fluent.get(Tr::PerTaskDownloadLimit),
                &settings.aria2.max_download_limit_kb,
                unit,
                move |v| {
                    let kb = if unit == SpeedUnit::Kbps { v } else { v * 1024 };
                    Message::SettingChanged(SettingKey::MaxDownloadLimit, kb.to_string())
                },
                move |u| Message::SpeedUnitChanged(SettingKey::MaxDownloadLimit, u),
            )
        })
        .push({
            let unit = settings_ui
                .speed_units
                .get(&SettingKey::MaxUploadLimit)
                .copied()
                .unwrap_or(SpeedUnit::Kbps);
            speed_labeled_input(
                fluent.get(Tr::PerTaskUploadLimit),
                &settings.aria2.max_upload_limit_kb,
                unit,
                move |v| {
                    let kb = if unit == SpeedUnit::Kbps { v } else { v * 1024 };
                    Message::SettingChanged(SettingKey::MaxUploadLimit, kb.to_string())
                },
                move |u| Message::SpeedUnitChanged(SettingKey::MaxUploadLimit, u),
            )
        })
        .push({
            let unit = settings_ui
                .speed_units
                .get(&SettingKey::LowestSpeedLimit)
                .copied()
                .unwrap_or(SpeedUnit::Kbps);
            speed_labeled_input(
                fluent.get(Tr::LowestSpeedLimit),
                &settings.aria2.lowest_speed_limit_kb,
                unit,
                move |v| {
                    let kb = if unit == SpeedUnit::Kbps { v } else { v * 1024 };
                    Message::SettingChanged(SettingKey::LowestSpeedLimit, kb.to_string())
                },
                move |u| Message::SpeedUnitChanged(SettingKey::LowestSpeedLimit, u),
            )
        })
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Confirm, accent))
        .push(labeled_toggle(
            fluent.get(Tr::NavToTasksAfterAdd),
            settings.nav_to_tasks_after_add,
            SettingKey::NavToTasksAfterAdd,
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::AutoCleanup, accent))
        .push(labeled_toggle(
            fluent.get(Tr::DeleteTorrentAfterComplete),
            settings.delete_torrent_after_complete,
            SettingKey::DeleteTorrentAfterComplete,
        ))
        .into()
}

fn bittorrent_view<'a>(
    fluent: &'a Fluent,
    settings: &'a Settings,
    accent: Color,
) -> Element<'a, Message> {
    column![]
        .spacing(4)
        .push(group_title(fluent, Tr::BtSettings, accent))
        .push(labeled_toggle(
            fluent.get(Tr::BtRequireCrypto),
            settings.aria2.bt_require_crypto,
            SettingKey::BtRequireCrypto,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::EnableDht),
            settings.aria2.enable_dht,
            SettingKey::EnableDht,
        ))
        .push(labeled_text_input(
            fluent.get(Tr::BtTracker),
            &settings.aria2.bt_tracker,
            SettingKey::BtTracker,
            false,
        ))
        .push(labeled_number(
            fluent.get(Tr::SeedRatio),
            &settings.aria2.seed_ratio,
            0.0..=100.0f64,
            0.1,
            SettingKey::SeedRatio,
        ))
        .push(labeled_number(
            fluent.get(Tr::SeedTime),
            &settings.aria2.seed_time,
            0..=u32::MAX,
            1,
            SettingKey::SeedTime,
        ))
        .into()
}

fn ed2k_view<'a>(fluent: &'a Fluent) -> Element<'a, Message> {
    container(
        text(fluent.get(Tr::ComingSoon))
            .size(16)
            .style(theme::style::text::secondary),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .height(Length::Fixed(200.0))
    .into()
}

fn network_view<'a>(
    fluent: &'a Fluent,
    settings: &'a Settings,
    ua_editor: &'a text_editor::Content,
    accent: Color,
) -> Element<'a, Message> {
    column![]
        .spacing(4)
        .push(group_title(fluent, Tr::Proxy, accent))
        .push(labeled_toggle(
            fluent.get(Tr::EnableProxy),
            settings.aria2.proxy_enabled,
            SettingKey::EnableProxy,
        ))
        .push(proxy_fields(fluent, settings))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::UserAgent, accent))
        .push(labeled_editor(
            fluent.get(Tr::UserAgent),
            ua_editor,
            Message::UaEditor,
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::ConnectTimeout, accent))
        .push(labeled_number(
            fluent.get(Tr::ConnectTimeout),
            &settings.aria2.connect_timeout,
            0..=u32::MAX,
            1,
            SettingKey::ConnectTimeout,
        ))
        .into()
}

fn proxy_fields<'a>(fluent: &'a Fluent, settings: &'a Settings) -> Element<'a, Message> {
    if settings.aria2.proxy_enabled {
        column![
            labeled_text_input(
                fluent.get(Tr::ProxyAddress),
                &settings.aria2.proxy_server,
                SettingKey::ProxyServer,
                false,
            ),
            labeled_text_input(
                fluent.get(Tr::ProxyUsername),
                &settings.aria2.proxy_username,
                SettingKey::ProxyUsername,
                false,
            ),
            labeled_text_input(
                fluent.get(Tr::ProxyPassword),
                &settings.aria2.proxy_password,
                SettingKey::ProxyPassword,
                true,
            ),
        ]
        .spacing(4)
        .into()
    } else {
        iced::widget::Space::new().height(Length::Fixed(0.0)).into()
    }
}

#[allow(clippy::too_many_arguments)]
fn advanced_view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    settings: &'a Settings,
    aria2_version: Option<&'a str>,
    aria2_check_msg: Option<&'a str>,
    aria2_status: Option<(&'a str, &'a str)>,
    aria2_fetch_error: Option<&'a str>,
    update_pending: Option<&'a str>,
) -> Element<'a, Message> {
    let accent = theme::accent(theme);
    let text_secondary = theme::text_secondary(theme);

    let auto_check_enabled = settings.update.should_auto_check("aria2-next");
    let update_toggle = setting_row(
        fluent.get(Tr::AutoCheckUpdate),
        toggler(auto_check_enabled)
            .on_toggle(Message::SetAutoCheck)
            .width(Length::Fixed(50.0))
            .into(),
    );

    let mut engine_rows: Vec<Element<Message>> = Vec::new();

    let version_text = match aria2_version {
        Some(v) => format!("aria2-next v{v}"),
        None => "aria2-next (--)".to_string(),
    };
    engine_rows.push(
        row![]
            .push(
                text(fluent.get(Tr::Aria2Version))
                    .size(13)
                    .width(Length::Fixed(200.0)),
            )
            .push(
                text(version_text)
                    .size(13)
                    .style(theme::style::text::secondary),
            )
            .height(Length::Fixed(36.0))
            .align_y(Alignment::Center)
            .into(),
    );

    if let Some(dir) = crate::config::aria2_bin_dir() {
        engine_rows.push(labeled_readonly(
            fluent,
            theme,
            fluent.get(Tr::EngineDataDir),
            &dir.to_string_lossy(),
        ));
    }
    if let Some(path) = crate::config::session_dir() {
        let sf = path.join("session.txt");
        engine_rows.push(labeled_readonly(
            fluent,
            theme,
            fluent.get(Tr::EngineSessionFile),
            &sf.to_string_lossy(),
        ));
    }
    if let Some(dir) = crate::config::log_dir() {
        engine_rows.push(labeled_readonly(
            fluent,
            theme,
            fluent.get(Tr::EngineLogFile),
            &dir.to_string_lossy(),
        ));
    }

    if let Some((stage, message)) = aria2_status {
        let status_color = if stage == "update-downloading"
            || stage == "update-verifying"
            || stage == "starting"
        {
            accent
        } else if stage == "ready" {
            theme::success(theme)
        } else {
            text_secondary
        };
        engine_rows.push(text(message).size(12).color(status_color).into());
    }

    if let Some(err) = aria2_fetch_error {
        engine_rows.push(text(err).size(12).color(theme::danger(theme)).into());
    }

    let mut btn_row = row![].spacing(12);

    if let Some(pending) = update_pending {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::RestartToUpdate)).size(12))
                .on_press(Message::RestartEngine)
                .padding([6, 12])
                .style(theme::style::button::primary()),
        );
        btn_row = btn_row.push(
            text(format!(
                "v{pending} - {}",
                fluent.get(Tr::PendingUpdateHint)
            ))
            .size(12)
            .style(theme::style::text::secondary),
        );
    } else if aria2_fetch_error.is_some() {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::Retry)).size(12))
                .on_press(Message::RetryAria2Fetch)
                .padding([6, 12])
                .style(theme::style::button::secondary()),
        );
    } else {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::CheckUpdate)).size(12))
                .on_press(Message::CheckAria2Update)
                .padding([6, 12])
                .style(theme::style::button::secondary()),
        );
    }

    if let Some(msg) = aria2_check_msg {
        btn_row = btn_row.push(text(msg).size(12).style(theme::style::text::secondary));
    }

    engine_rows.push(btn_row.into());

    let mut engine_col = column![].spacing(8);
    for elem in engine_rows {
        engine_col = engine_col.push(elem);
    }

    column![]
        .spacing(12)
        .push(update_toggle)
        .push(group_title(fluent, Tr::Engine, accent))
        .push(engine_col)
        .into()
}

fn setting_row<'a>(label: String, control: Element<'a, Message>) -> Element<'a, Message> {
    row![]
        .push(text(label).size(13).width(Length::Fixed(200.0)))
        .push(control)
        .height(Length::Fixed(36.0))
        .align_y(Alignment::Center)
        .into()
}

fn labeled_number<'a, T>(
    label: String,
    value: &'a T,
    bounds: impl std::ops::RangeBounds<T> + 'a,
    step: T,
    key: SettingKey,
) -> Element<'a, Message>
where
    T: num_traits::Num
        + num_traits::NumAssignOps
        + PartialOrd
        + std::fmt::Display
        + std::str::FromStr
        + Clone
        + Copy
        + num_traits::Bounded
        + 'static,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    setting_row(
        label,
        number_stepper(
            value,
            bounds,
            step,
            move |v| Message::SettingChanged(key, v.to_string()),
            Length::Fixed(160.0),
        ),
    )
}

fn labeled_toggle<'a>(label: String, value: bool, key: SettingKey) -> Element<'a, Message> {
    setting_row(
        label,
        toggler(value)
            .on_toggle(move |v| Message::SettingChanged(key, v.to_string()))
            .width(Length::Fixed(50.0))
            .into(),
    )
}

fn labeled_text_input<'a>(
    label: String,
    value: &'a str,
    key: SettingKey,
    secure: bool,
) -> Element<'a, Message> {
    let mut input = theme::input_layout(
        text_input("", value)
            .on_input(move |s| Message::SettingChanged(key, s))
            .width(Length::Fill)
            .style(theme::style::input::standard),
    );
    if secure {
        input = input.secure(true);
    }
    setting_row(label, input.into())
}

fn labeled_editor<'a>(
    label: String,
    content: &'a text_editor::Content,
    on_edit: fn(text_editor::Action) -> Message,
) -> Element<'a, Message> {
    row![]
        .push(text(label).size(13).width(Length::Fixed(200.0)))
        .push(theme::editor_layout(
            text_editor(content)
                .on_action(on_edit)
                .height(Length::Fixed(80.0))
                .style(theme::style::text_editor::standard),
        ))
        .align_y(Alignment::Start)
        .into()
}

fn labeled_pick<'a, T>(
    fluent: &'a Fluent,
    label: String,
    options: Vec<Labeled<T>>,
    selected: Option<T>,
    on_select: impl Fn(Labeled<T>) -> Message + 'a,
) -> Element<'a, Message>
where
    T: PartialEq + Clone + 'static,
{
    let placeholder = fluent.get(Tr::SelectPlaceholder);
    let sel = selected.and_then(|s| options.iter().find(|o| o.value == s).cloned());
    setting_row(
        label,
        pick_list(options, sel, on_select)
            .placeholder(&placeholder)
            .width(Length::Fixed(180.0))
            .style(theme::style::pick_list::standard)
            .menu_style(theme::style::pick_list::menu)
            .into(),
    )
}

fn labeled_readonly<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    label: String,
    value: &str,
) -> Element<'a, Message> {
    let picker = PathPicker::read_only(value.to_string());
    row![]
        .push(text(label).size(13).width(Length::Fixed(200.0)))
        .push(picker.view(fluent, theme, &[], |e| match e {
            PathPickerEvent::Copy(s) => Message::CopyPath(s),
            _ => Message::Noop,
        }))
        .height(Length::Fixed(36.0))
        .align_y(Alignment::Center)
        .into()
}

#[derive(Debug, Clone, Copy)]
struct UnitOption {
    value: SpeedUnit,
    label: &'static str,
}

impl std::fmt::Display for UnitOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

impl PartialEq for UnitOption {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

fn speed_labeled_input<'a>(
    label: String,
    value_kb: &'a u64,
    unit: SpeedUnit,
    on_value: impl Fn(u64) -> Message + 'a,
    on_unit: impl Fn(SpeedUnit) -> Message + 'a,
) -> Element<'a, Message> {
    let unit_opts = [
        UnitOption {
            value: SpeedUnit::Kbps,
            label: "KB/s",
        },
        UnitOption {
            value: SpeedUnit::Mbps,
            label: "MB/s",
        },
    ];
    let sel = unit_opts.iter().find(|o| o.value == unit).copied();

    let (display_val, step) = match unit {
        SpeedUnit::Kbps => (*value_kb, 100),
        SpeedUnit::Mbps => {
            if *value_kb == 0 {
                (0, 1)
            } else {
                (*value_kb / 1024, 1)
            }
        }
    };
    let display: &'a u64 = &*Box::leak(Box::new(display_val));

    setting_row(
        label,
        row![]
            .spacing(8)
            .push(number_stepper(
                display,
                0..=u64::MAX,
                step,
                on_value,
                Length::Fixed(160.0),
            ))
            .push(
                pick_list(unit_opts, sel, move |o| on_unit(o.value))
                    .width(Length::Fixed(80.0))
                    .style(theme::style::pick_list::standard)
                    .menu_style(theme::style::pick_list::menu),
            )
            .align_y(Alignment::Center)
            .into(),
    )
}

fn group_title<'a>(fluent: &'a Fluent, key: Tr, accent: Color) -> Element<'a, Message> {
    text(fluent.get(key)).size(16).color(accent).into()
}
