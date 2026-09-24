use chrono::TimeZone;
use iced::widget::{button, column, container, row, text, toggler};
use iced::{Alignment, Element, Length};

use super::{
    group_title, labeled_pick, labeled_toggle, setting_row, sub_items, Fluent, Labeled, Locale,
    Message, SettingKey, SettingValue, Settings, SettingsMsg, SettingsUiState, Tr, FONT_BODY,
    FONT_ICON, FONT_MEDIUM, FONT_SMALL, PADDING_BUTTON_SM, SPACE_LG, SPACE_SM, SPACE_XL,
    SWATCH_SIZE,
};
use crate::ui::components::copyable_text::copyable_text;
use crate::ui::components::tooltip;
use crate::ui::theme;

#[allow(clippy::too_many_arguments)]
pub(super) fn general_view<'a>(
    fluent: &'a Fluent,
    theme_: &'a iced::Theme,
    settings: &'a Settings,
    applied_settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    font_restart_required: bool,
    aria2_version: Option<&'a str>,
    update_check_in_flight: bool,
    aria2_download_version: Option<&'a str>,
    aria2_download_progress: Option<(u64, u64)>,
) -> Element<'a, Message> {
    let accent = theme::accent(theme_);
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

    let mut col = column![]
        .spacing(SPACE_SM)
        .push(crate::ui::components::scroll_top_gap::view())
        .push(group_title(fluent, Tr::SystemInfo, accent))
        .push(setting_row(
            fluent.get(Tr::SystemPlatform),
            copyable_text(
                crate::updater::platform_display(),
                Message::CopyText(crate::updater::platform_display()),
            )
            .into(),
        ))
        .push(setting_row(
            fluent.get(Tr::AppVersion),
            copyable_text(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                Message::CopyText(format!("v{}", env!("CARGO_PKG_VERSION"))),
            )
            .into(),
        ))
        .push(setting_row(
            fluent.get(Tr::Aria2Version),
            copyable_text(
                aria2_version.map_or("--".into(), |v| format!("v{v}")),
                Message::CopyText(aria2_version.map_or("--".into(), |v| format!("v{v}"))),
            )
            .into(),
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Appearance, accent));
    if theme::system_accent_supported() {
        col = col.push(setting_row(
            fluent.get(Tr::FollowSystemAccent),
            toggler(settings.follow_system_accent)
                .on_toggle(|v| Message::Settings(SettingsMsg::FollowSystemAccentToggled(v)))
                .width(Length::Fixed(50.0))
                .into(),
        ));
    }
    if !settings.follow_system_accent {
        col = col.push(theme_color_swatches(fluent, theme_, settings));
    }
    col = col
        .push(labeled_pick(
            fluent,
            fluent.get(Tr::ColorMode),
            mode_opts,
            Some(settings.theme_mode),
            |opt| Message::Settings(SettingsMsg::ThemeModeChanged(opt.value)),
        ))
        .push(font_family_row(
            fluent,
            theme_,
            settings,
            settings_ui,
            font_restart_required,
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Locale, accent))
        .push(labeled_pick(
            fluent,
            fluent.get(Tr::Locale),
            vec![
                Labeled {
                    value: Locale::System,
                    label: fluent.get(Tr::LocaleSystem),
                },
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
            |opt| Message::Settings(SettingsMsg::LocaleChanged(opt.value)),
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Tray, accent))
        .push(labeled_toggle(
            fluent.get(Tr::CloseToTray),
            settings.close_to_tray,
            SettingKey::CloseToTray,
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Startup, accent))
        .push(labeled_toggle(
            fluent.get(Tr::LaunchOnStartup),
            settings.autostart_enabled,
            SettingKey::AutoStart,
        ))
        .push(if settings.autostart_enabled {
            sub_items([labeled_toggle(
                fluent.get(Tr::LaunchHiddenOnAutostart),
                settings.start_hidden_on_autostart,
                SettingKey::StartHiddenOnAutostart,
            )])
        } else {
            iced::widget::Space::new().height(Length::Fixed(0.0)).into()
        })
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::AutoUpdate, accent))
        .push(labeled_toggle(
            fluent.get(Tr::AutoUpdate),
            settings.update.enabled,
            SettingKey::AutoUpdateEnabled,
        ))
        .push(if settings.update.enabled {
            let freq_opts = vec![
                Labeled {
                    value: 0,
                    label: fluent.get(Tr::IntervalEveryStartup),
                },
                Labeled {
                    value: 24,
                    label: fluent.get(Tr::IntervalDaily),
                },
                Labeled {
                    value: 168,
                    label: fluent.get(Tr::IntervalWeekly),
                },
                Labeled {
                    value: 720,
                    label: fluent.get(Tr::IntervalMonthly),
                },
            ];
            sub_items([
                labeled_pick(
                    fluent,
                    fluent.get(Tr::UpdateFrequency),
                    freq_opts,
                    Some(settings.update.interval_hours),
                    |opt| {
                        Message::Settings(SettingsMsg::SettingChanged(
                            SettingKey::UpdateCheckInterval,
                            SettingValue::Num(opt.value as u64),
                        ))
                    },
                ),
                labeled_pick(
                    fluent,
                    fluent.get(Tr::UpdateScope),
                    vec![
                        Labeled {
                            value: crate::config::UpdateScope::App,
                            label: fluent.get(Tr::ScopeApp),
                        },
                        Labeled {
                            value: crate::config::UpdateScope::Engine,
                            label: fluent.get(Tr::ScopeEngine),
                        },
                        Labeled {
                            value: crate::config::UpdateScope::Both,
                            label: fluent.get(Tr::ScopeBoth),
                        },
                    ],
                    Some(settings.update.scope),
                    |opt| {
                        Message::Settings(SettingsMsg::SettingChanged(
                            SettingKey::UpdateScope,
                            SettingValue::Text(opt.value.as_str().into()),
                        ))
                    },
                ),
                labeled_pick(
                    fluent,
                    fluent.get(Tr::SilentUpdateScope),
                    vec![
                        Labeled {
                            value: crate::config::SilentUpdateScope::Off,
                            label: fluent.get(Tr::ScopeOff),
                        },
                        Labeled {
                            value: crate::config::SilentUpdateScope::Engine,
                            label: fluent.get(Tr::ScopeEngine),
                        },
                        Labeled {
                            value: crate::config::SilentUpdateScope::App,
                            label: fluent.get(Tr::ScopeApp),
                        },
                        Labeled {
                            value: crate::config::SilentUpdateScope::Both,
                            label: fluent.get(Tr::ScopeBoth),
                        },
                    ],
                    Some(settings.update.silent_update_scope),
                    |opt| {
                        Message::Settings(SettingsMsg::SettingChanged(
                            SettingKey::SilentUpdateScope,
                            SettingValue::Text(opt.value.as_str().into()),
                        ))
                    },
                ),
                labeled_toggle(
                    fluent.get(Tr::UpdateBetaChannel),
                    settings.update.beta_channel,
                    SettingKey::BetaChannel,
                ),
            ])
        } else {
            iced::widget::Space::new().height(Length::Fixed(0.0)).into()
        })
        .push(last_check_row(
            fluent,
            theme_,
            settings,
            applied_settings,
            update_check_in_flight,
        ));
    if let Some(row) = aria2_download_progress_row(aria2_download_version, aria2_download_progress)
    {
        col = col.push(row);
    }
    col.into()
}

pub(super) fn last_check_row<'a>(
    fluent: &'a Fluent,
    theme_: &iced::Theme,
    settings: &'a Settings,
    applied_settings: &'a Settings,
    update_check_in_flight: bool,
) -> Element<'a, Message> {
    let time_str = match settings.update.last_check_time {
        Some(ms) => match chrono::Local.timestamp_millis_opt(ms) {
            chrono::LocalResult::Single(t) => t.format("%Y-%m-%d %H:%M").to_string(),
            _ => fluent.get(Tr::Never),
        },
        None => fluent.get(Tr::Never),
    };
    let last_check_str = fluent.get_args(Tr::LastCheckTime, &{
        let mut a = std::collections::HashMap::new();
        a.insert(std::borrow::Cow::from("time"), time_str.into());
        a
    });
    let check_btn = if update_check_in_flight {
        button(
            row![
                crate::ui::components::spinner::Spinner::hourglass(
                    theme::accent(theme_),
                    FONT_ICON as f32
                )
                .view(),
                text(fluent.get(Tr::CheckingUpdate)).size(FONT_SMALL),
            ]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center),
        )
        .on_press_maybe(None)
        .padding(PADDING_BUTTON_SM)
        .height(Length::Fixed(crate::ui::components::CONTROL_HEIGHT))
        .style(theme::style::button::secondary())
    } else {
        button(
            row![
                crate::ui::icon::circle_fading_arrow_up().size(FONT_ICON),
                text(fluent.get(Tr::CheckNow)).size(FONT_SMALL),
            ]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center),
        )
        .on_press_maybe(if settings.update != applied_settings.update {
            None
        } else {
            Some(Message::Settings(SettingsMsg::CheckUpdatesNow))
        })
        .padding(PADDING_BUTTON_SM)
        .height(Length::Fixed(crate::ui::components::CONTROL_HEIGHT))
        .style(theme::style::button::secondary())
    };
    setting_row(
        fluent.get(Tr::LastCheck),
        row![
            check_btn,
            text(last_check_str)
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        ]
        .spacing(SPACE_LG)
        .align_y(Alignment::Center)
        .into(),
    )
}

pub(super) fn aria2_download_progress_row<'a>(
    version: Option<&'a str>,
    progress: Option<(u64, u64)>,
) -> Option<Element<'a, Message>> {
    let (done, total) = progress?;
    let done_mb = done as f64 / 1024.0 / 1024.0;
    let total_mb = total as f64 / 1024.0 / 1024.0;
    let version_str = version.unwrap_or("");
    let label = format!(
        "aria2-next {version_str}（{:.1}MB/{:.1}MB）",
        done_mb, total_mb
    );
    Some(
        row![
            iced::widget::Space::new().width(Length::Fixed(200.0)),
            text(label)
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        ]
        .width(Length::Fill)
        .into(),
    )
}

pub(super) fn theme_color_swatches<'a>(
    fluent: &'a Fluent,
    theme_: &'a iced::Theme,
    settings: &'a Settings,
) -> Element<'a, Message> {
    use crate::ui::components::swatch_icon::{swatch_check, swatch_plus, swatch_text_color};
    let current = theme::accent_color(&settings.theme_color);
    let mut swatch_row = row![].spacing(SPACE_XL).align_y(Alignment::Center);
    for (color, name) in theme::candidate_colors() {
        let selected = *color == current;
        let mark_color = swatch_text_color(*color);
        let swatch = button(
            container(if selected {
                swatch_check(mark_color)
            } else {
                iced::widget::Space::new()
                    .width(Length::Fixed(0.0))
                    .height(Length::Fixed(0.0))
                    .into()
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
        )
        .on_press(Message::Settings(SettingsMsg::ThemeColorChanged(*color)))
        .width(Length::Fixed(SWATCH_SIZE))
        .height(Length::Fixed(SWATCH_SIZE))
        .padding(0)
        .style(theme::style::button::swatch(*color, selected));
        swatch_row = swatch_row.push(tooltip::standard(
            swatch,
            text(*name),
            iced::widget::tooltip::Position::Bottom,
        ));
    }
    let add_swatch = button(
        container(swatch_plus(swatch_text_color(theme::accent(theme_))))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(Message::Settings(SettingsMsg::CustomColorPickerToggle))
    .width(Length::Fixed(SWATCH_SIZE))
    .height(Length::Fixed(SWATCH_SIZE))
    .padding(0)
    .style(theme::style::button::swatch(theme::accent(theme_), false));
    let add_swatch = tooltip::standard(
        add_swatch,
        text(fluent.get(Tr::CustomColor)),
        iced::widget::tooltip::Position::Bottom,
    );
    swatch_row = swatch_row.push(add_swatch);

    setting_row(
        fluent.get(Tr::ThemeColor),
        swatch_row
            .width(Length::Fill)
            .wrap()
            .vertical_spacing(SPACE_LG)
            .into(),
    )
}

pub(super) fn font_family_row<'a>(
    fluent: &'a Fluent,
    theme_: &'a iced::Theme,
    settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    restart_required: bool,
) -> Element<'a, Message> {
    use crate::ui::components::font_picker::{self as font_picker, FontPickerOption};
    let options: &'static [FontPickerOption] =
        font_picker::build_options(fluent, theme::system_font_families());
    let pick: Element<'a, Message> = font_picker::view(
        fluent,
        theme_,
        &settings_ui.font_picker,
        options,
        &settings.font_family,
        Message::Settings(SettingsMsg::FontPickerToggle),
        Message::Settings(SettingsMsg::FontPickerClose),
        |s| Message::Settings(SettingsMsg::FontPickerQueryChanged(s)),
        |id| Message::Settings(SettingsMsg::FontFamilyChanged(id)),
        |_v| Message::ScrollableScrolled(iced::widget::Id::new("font-picker-list")),
    );

    let mut controls = column![
        pick,
        text("AaBb 你好 0123 字体预览")
            .size(FONT_BODY)
            .font(theme::font_from_family(&settings.font_family)),
        text(fluent.get(Tr::FontRestartHint))
            .size(FONT_SMALL)
            .style(theme::style::text::secondary),
    ]
    .spacing(SPACE_SM);
    if restart_required {
        controls = controls.push(
            button(text(fluent.get(Tr::SaveAndRestartApp)).size(FONT_SMALL))
                .on_press(Message::Settings(SettingsMsg::RestartApp))
                .padding(PADDING_BUTTON_SM)
                .style(theme::style::button::primary()),
        );
    }

    row![
        container(text(fluent.get(Tr::FontFamily)).size(FONT_MEDIUM))
            .width(Length::Fixed(200.0))
            .center_y(Length::Fixed(36.0)),
        controls,
    ]
    .align_y(Alignment::Start)
    .into()
}
