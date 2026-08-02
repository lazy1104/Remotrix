use std::collections::HashMap;
use std::sync::Mutex;

use iced::widget::{
    button, checkbox, column, container, pick_list, row, text, text_editor, text_input, toggler,
};
use iced::{Alignment, Element, Length};

use crate::config::Settings;
use crate::i18n::{Fluent, Locale, Tr};
use crate::message::{Message, PathPickerId, SettingKey, SettingsCategory, SpeedUnit};
use iced::Color;

use crate::ui::components::number_stepper::number_stepper;
use crate::ui::components::path_picker::{PathPicker, PathPickerEvent};
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::components::time_picker::time_picker;
use crate::ui::components::tooltip;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

#[derive(Debug, Clone)]
pub struct SettingsUiState {
    pub download_picker: PathPicker,
    pub ed2k_server_list_picker: PathPicker,
    pub ed2k_node_list_picker: PathPicker,
    pub speed_units: HashMap<SettingKey, SpeedUnit>,
    pub schedule_start_picker_open: bool,
    pub schedule_end_picker_open: bool,
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
            ed2k_server_list_picker: PathPicker::file(settings.aria2.ed2k_server_list.clone()),
            ed2k_node_list_picker: PathPicker::file(settings.aria2.ed2k_node_list.clone()),
            speed_units,
            schedule_start_picker_open: false,
            schedule_end_picker_open: false,
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
    font_restart_required: bool,
) -> Element<'a, Message> {
    let accent = theme::accent(theme);
    let dirty = !settings.apply_fields_equal(applied_settings);
    let content = match category {
        SettingsCategory::General => general_view(fluent, theme, settings, font_restart_required),
        SettingsCategory::Download => {
            download_view(fluent, theme, settings, settings_ui, path_history)
        }
        SettingsCategory::BitTorrent => bittorrent_view(fluent, settings, accent),
        SettingsCategory::Ed2k => ed2k_view(fluent, theme, settings, settings_ui),
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
        .push(text(settings_title(fluent, category)).size(FONT_PAGE_TITLE))
        .push(iced::widget::Space::new().height(Length::Fixed(20.0)))
        .push(slim_scrollable(content).height(Length::Fill));

    let mut actions = row![].spacing(SPACE_2XL).width(Length::Fill);
    actions = actions.push(
        button(text(fluent.get(Tr::Apply)).size(FONT_BODY))
            .on_press_maybe(if dirty {
                Some(Message::ApplySettings)
            } else {
                None
            })
            .padding(PADDING_BUTTON_XL)
            .style(theme::style::button::primary()),
    );
    if dirty {
        actions = actions.push(
            button(text(fluent.get(Tr::Reset)).size(FONT_BODY))
                .on_press(Message::ResetSettings)
                .padding(PADDING_BUTTON_XL)
                .style(theme::style::button::secondary()),
        );
    }
    body = body.push(actions);

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(PADDING_PAGE)
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
    font_restart_required: bool,
) -> Element<'a, Message> {
    let accent = theme::accent(theme);
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
        .spacing(SPACE_SM)
        .push(group_title(fluent, Tr::Appearance, accent))
        .push(theme_color_swatches(fluent, settings))
        .push(labeled_pick(
            fluent,
            fluent.get(Tr::ColorMode),
            mode_opts,
            Some(settings.theme_mode),
            |opt| Message::ThemeModeChanged(opt.value),
        ))
        .push(font_family_row(fluent, settings, font_restart_required))
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
            |opt| Message::LocaleChanged(opt.value),
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .into()
}

fn theme_color_swatches<'a>(fluent: &'a Fluent, settings: &'a Settings) -> Element<'a, Message> {
    let current = theme::accent_color(&settings.theme_color);
    let mut swatch_row = row![].spacing(SPACE_XL).align_y(Alignment::Center);
    for (color, name) in theme::candidate_colors() {
        let selected = *color == current;
        let swatch = button(if selected {
            icon::circle_check().size(FONT_ICON)
        } else {
            text("").size(FONT_ICON)
        })
        .on_press(Message::ThemeColorChanged(*color))
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
    setting_row_auto(
        fluent.get(Tr::ThemeColor),
        swatch_row
            .width(Length::Fill)
            .wrap()
            .vertical_spacing(SPACE_LG)
            .into(),
    )
}

fn font_family_row<'a>(
    fluent: &'a Fluent,
    settings: &'a Settings,
    restart_required: bool,
) -> Element<'a, Message> {
    let options = font_family_options(fluent);
    let placeholder = fluent.get(Tr::SelectPlaceholder);
    let selected = options
        .iter()
        .find(|o| o.value == settings.font_family)
        .cloned();
    let pick: Element<'a, Message> =
        pick_list(options, selected, |o| Message::FontFamilyChanged(o.value))
            .placeholder(&placeholder)
            .width(Length::Fixed(240.0))
            .style(theme::style::pick_list::standard)
            .menu_style(theme::style::pick_list::menu)
            .into();

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
                .on_press(Message::RestartApp)
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

type FontOptions = &'static [Labeled<String>];

static FONT_OPTIONS: Mutex<Option<(Locale, FontOptions)>> = Mutex::new(None);

fn font_family_options(fluent: &Fluent) -> FontOptions {
    let mut cache = FONT_OPTIONS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((locale, options)) = cache.as_ref() {
        if *locale == fluent.locale {
            return options;
        }
    }
    let mut options = Vec::new();
    options.push(Labeled {
        value: String::new(),
        label: fluent.get(Tr::SystemDefault),
    });
    options.push(Labeled {
        value: theme::BUNDLED_FONT_NAME.to_string(),
        label: theme::BUNDLED_FONT_NAME.to_string(),
    });
    for family in theme::system_font_families() {
        let family = family.clone();
        if family.eq_ignore_ascii_case(theme::BUNDLED_FONT_NAME) {
            continue;
        }
        options.push(Labeled {
            value: family.clone(),
            label: family,
        });
    }
    let leaked: FontOptions = Box::leak(options.into_boxed_slice());
    *cache = Some((fluent.locale, leaked));
    leaked
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
        .spacing(SPACE_SM)
        .push(group_title(fluent, Tr::DownloadFolder, accent))
        .push(
            row![]
                .push(
                    text(fluent.get(Tr::DownloadFolder))
                        .size(FONT_MEDIUM)
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
                .spacing(SPACE_LG)
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
        .push(labeled_toggle(
            fluent.get(Tr::EnableScheduledSpeedLimit),
            settings.speed_limit_schedule.enabled,
            SettingKey::SpeedLimitScheduleEnabled,
        ))
        .push({
            let el: Element<'_, Message> = if settings.speed_limit_schedule.enabled {
                column![
                    setting_row(
                        fluent.get(Tr::ScheduleStartTime),
                        time_picker(
                            &settings.speed_limit_schedule.start,
                            settings_ui.schedule_start_picker_open,
                            Message::ToggleScheduleStartPicker,
                            move |s| Message::SettingChanged(SettingKey::ScheduleStart, s),
                            Length::Fixed(160.0),
                        ),
                    ),
                    setting_row(
                        fluent.get(Tr::ScheduleEndTime),
                        time_picker(
                            &settings.speed_limit_schedule.end,
                            settings_ui.schedule_end_picker_open,
                            Message::ToggleScheduleEndPicker,
                            move |s| Message::SettingChanged(SettingKey::ScheduleEnd, s),
                            Length::Fixed(160.0),
                        ),
                    ),
                    text(fluent.get(Tr::ScheduleHint))
                        .size(FONT_SMALL)
                        .style(theme::style::text::secondary),
                ]
                .spacing(SPACE_SM)
                .into()
            } else {
                iced::widget::Space::new().height(Length::Fixed(0.0)).into()
            };
            el
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
        .push(labeled_toggle(
            fluent.get(Tr::CleanupCompletedOnClose),
            settings.cleanup_completed_on_close,
            SettingKey::CleanupCompletedOnClose,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::RemoveTaskIfFilesMissing),
            settings.remove_task_if_files_missing,
            SettingKey::RemoveTaskIfFilesMissing,
        ))
        .into()
}

fn bittorrent_view<'a>(
    fluent: &'a Fluent,
    settings: &'a Settings,
    accent: Color,
) -> Element<'a, Message> {
    column![]
        .spacing(SPACE_SM)
        .push(group_title(fluent, Tr::BtSettings, accent))
        .push(labeled_toggle(
            fluent.get(Tr::BtAutoDownload),
            settings.aria2.bt_auto_download,
            SettingKey::BtAutoDownload,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::BtRequireCrypto),
            settings.aria2.bt_require_crypto,
            SettingKey::BtRequireCrypto,
        ))
        .push({
            let placeholder = fluent.get(Tr::BtTrackerPlaceholder);
            labeled_text_input(
                fluent.get(Tr::BtTracker),
                &settings.aria2.bt_tracker,
                SettingKey::BtTracker,
                false,
                &placeholder,
            )
        })
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::NodeExchange, accent))
        .push(labeled_toggle(
            fluent.get(Tr::EnableDht),
            settings.aria2.enable_dht,
            SettingKey::EnableDht,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::BtEnableLpd),
            settings.aria2.bt_enable_lpd,
            SettingKey::BtEnableLpd,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::EnablePeerExchange),
            settings.aria2.enable_peer_exchange,
            SettingKey::EnablePeerExchange,
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Seeding, accent))
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

fn ed2k_view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
) -> Element<'a, Message> {
    let accent = theme::accent(theme);
    column![]
        .spacing(SPACE_SM)
        .push(group_title(fluent, Tr::Ed2kSettings, accent))
        .push({
            let placeholder = fluent.get(Tr::Ed2kServerPlaceholder);
            labeled_text_input(
                fluent.get(Tr::Ed2kServer),
                &settings.aria2.ed2k_server,
                SettingKey::Ed2kServer,
                false,
                &placeholder,
            )
        })
        .push(
            row![]
                .push(
                    text(fluent.get(Tr::Ed2kServerList))
                        .size(FONT_MEDIUM)
                        .width(Length::Fixed(200.0)),
                )
                .push(
                    settings_ui
                        .ed2k_server_list_picker
                        .view(fluent, theme, &[], |e| {
                            Message::PathPicker(PathPickerId::Ed2kServerList, e)
                        }),
                )
                .height(Length::Fixed(36.0))
                .align_y(Alignment::Center),
        )
        .push(
            row![]
                .push(
                    text(fluent.get(Tr::Ed2kNodeList))
                        .size(FONT_MEDIUM)
                        .width(Length::Fixed(200.0)),
                )
                .push(
                    settings_ui
                        .ed2k_node_list_picker
                        .view(fluent, theme, &[], |e| {
                            Message::PathPicker(PathPickerId::Ed2kNodeList, e)
                        }),
                )
                .height(Length::Fixed(36.0))
                .align_y(Alignment::Center),
        )
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Network, accent))
        .push(labeled_number(
            fluent.get(Tr::Ed2kListenPort),
            &settings.aria2.ed2k_listen_port,
            0..=65535u16,
            1,
            SettingKey::Ed2kListenPort,
        ))
        .push(labeled_number(
            fluent.get(Tr::Ed2kUdpListenPort),
            &settings.aria2.ed2k_udp_listen_port,
            0..=65535u16,
            1,
            SettingKey::Ed2kUdpListenPort,
        ))
        .push(labeled_number(
            fluent.get(Tr::Ed2kUploadSlots),
            &settings.aria2.ed2k_upload_slots,
            1..=u16::MAX,
            1,
            SettingKey::Ed2kUploadSlots,
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(8.0)))
        .push(
            text(fluent.get(Tr::Ed2kRestartHint))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .into()
}

fn network_view<'a>(
    fluent: &'a Fluent,
    settings: &'a Settings,
    ua_editor: &'a text_editor::Content,
    accent: Color,
) -> Element<'a, Message> {
    column![]
        .spacing(SPACE_SM)
        .push(group_title(fluent, Tr::Proxy, accent))
        .push(labeled_toggle(
            fluent.get(Tr::EnableProxy),
            settings.aria2.proxy_enabled,
            SettingKey::EnableProxy,
        ))
        .push(proxy_fields(fluent, settings))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::UserAgent, accent))
        .push({
            let placeholder = fluent.get(Tr::UserAgentPlaceholder);
            labeled_editor(
                fluent.get(Tr::UserAgent),
                ua_editor,
                Message::UaEditor,
                placeholder,
            )
        })
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
        let address = fluent.get(Tr::ProxyAddressPlaceholder);
        let username = fluent.get(Tr::ProxyUsernamePlaceholder);
        let password = fluent.get(Tr::ProxyPasswordPlaceholder);
        column![
            labeled_text_input(
                fluent.get(Tr::ProxyAddress),
                &settings.aria2.proxy_server,
                SettingKey::ProxyServer,
                false,
                &address,
            ),
            labeled_text_input(
                fluent.get(Tr::ProxyUsername),
                &settings.aria2.proxy_username,
                SettingKey::ProxyUsername,
                false,
                &username,
            ),
            labeled_text_input(
                fluent.get(Tr::ProxyPassword),
                &settings.aria2.proxy_password,
                SettingKey::ProxyPassword,
                true,
                &password,
            ),
        ]
        .spacing(SPACE_SM)
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
                    .size(FONT_MEDIUM)
                    .width(Length::Fixed(200.0)),
            )
            .push(
                text(version_text)
                    .size(FONT_MEDIUM)
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
        engine_rows.push(text(message).size(FONT_SMALL).color(status_color).into());
    }

    if let Some(err) = aria2_fetch_error {
        engine_rows.push(
            text(err)
                .size(FONT_SMALL)
                .color(theme::danger(theme))
                .into(),
        );
    }

    let mut btn_row = row![].spacing(SPACE_2XL);

    if let Some(pending) = update_pending {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::RestartToUpdate)).size(FONT_SMALL))
                .on_press(Message::RestartEngine)
                .padding(PADDING_BUTTON_SM)
                .style(theme::style::button::primary()),
        );
        btn_row = btn_row.push(
            text(format!(
                "v{pending} - {}",
                fluent.get(Tr::PendingUpdateHint)
            ))
            .size(FONT_SMALL)
            .style(theme::style::text::secondary),
        );
    } else if aria2_fetch_error.is_some() {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::Retry)).size(FONT_SMALL))
                .on_press(Message::RetryAria2Fetch)
                .padding(PADDING_BUTTON_SM)
                .style(theme::style::button::secondary()),
        );
    } else {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::CheckUpdate)).size(FONT_SMALL))
                .on_press(Message::CheckAria2Update)
                .padding(PADDING_BUTTON_SM)
                .style(theme::style::button::secondary()),
        );
    }

    if let Some(msg) = aria2_check_msg {
        btn_row = btn_row.push(
            text(msg)
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        );
    }

    engine_rows.push(btn_row.into());

    let mut engine_col = column![].spacing(SPACE_LG);
    for elem in engine_rows {
        engine_col = engine_col.push(elem);
    }

    let mut clipboard_col = column![].spacing(SPACE_SM);
    clipboard_col = clipboard_col
        .push(group_title(fluent, Tr::Clipboard, accent))
        .push(labeled_toggle(
            fluent.get(Tr::DetectClipboardOnStart),
            settings.detect_clipboard_on_start,
            SettingKey::DetectClipboardOnStart,
        ));
    if settings.detect_clipboard_on_start {
        clipboard_col = clipboard_col
            .push(labeled_checkbox(
                fluent.get(Tr::LinkTypeHttp),
                settings.clipboard_types.http,
                SettingKey::ClipboardHttp,
            ))
            .push(labeled_checkbox(
                fluent.get(Tr::LinkTypeFtp),
                settings.clipboard_types.ftp,
                SettingKey::ClipboardFtp,
            ))
            .push(labeled_checkbox(
                fluent.get(Tr::LinkTypeMagnet),
                settings.clipboard_types.magnet,
                SettingKey::ClipboardMagnet,
            ))
            .push(labeled_checkbox(
                fluent.get(Tr::LinkTypeEd2k),
                settings.clipboard_types.ed2k,
                SettingKey::ClipboardEd2k,
            ))
            .push(labeled_checkbox(
                fluent.get(Tr::LinkTypeThunder),
                settings.clipboard_types.thunder,
                SettingKey::ClipboardThunder,
            ))
            .push(labeled_checkbox(
                fluent.get(Tr::LinkTypeBtInfohash),
                settings.clipboard_types.bt_infohash,
                SettingKey::ClipboardBtInfohash,
            ));
    }

    column![]
        .spacing(SPACE_2XL)
        .push(update_toggle)
        .push(clipboard_col)
        .push(group_title(fluent, Tr::Performance, accent))
        .push({
            let fa_none = fluent.get(Tr::FileAllocationNone);
            let fa_prealloc = fluent.get(Tr::FileAllocationPrealloc);
            let fa_falloc = fluent.get(Tr::FileAllocationFalloc);
            let opts = vec![
                Labeled {
                    value: "none".to_string(),
                    label: fa_none,
                },
                Labeled {
                    value: "prealloc".to_string(),
                    label: fa_prealloc,
                },
                Labeled {
                    value: "falloc".to_string(),
                    label: fa_falloc,
                },
            ];
            labeled_pick(
                fluent,
                fluent.get(Tr::FileAllocation),
                opts,
                Some(settings.aria2.file_allocation.clone()),
                |opt| Message::SettingChanged(SettingKey::FileAllocation, opt.value),
            )
        })
        .push(labeled_number(
            fluent.get(Tr::DiskCache),
            &settings.aria2.disk_cache_mb,
            0..=u64::MAX,
            1,
            SettingKey::DiskCache,
        ))
        .push(group_title(fluent, Tr::Engine, accent))
        .push(engine_col)
        .into()
}

fn setting_row<'a>(label: String, control: Element<'a, Message>) -> Element<'a, Message> {
    row![]
        .push(text(label).size(FONT_MEDIUM).width(Length::Fixed(200.0)))
        .push(control)
        .height(Length::Fixed(36.0))
        .align_y(Alignment::Center)
        .into()
}

fn setting_row_auto<'a>(label: String, control: Element<'a, Message>) -> Element<'a, Message> {
    row![]
        .push(
            container(text(label).size(FONT_MEDIUM))
                .width(Length::Fixed(200.0))
                .center_y(Length::Fixed(36.0)),
        )
        .push(control)
        .align_y(Alignment::Start)
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

fn labeled_checkbox<'a>(label: String, value: bool, key: SettingKey) -> Element<'a, Message> {
    setting_row(
        label,
        checkbox(value)
            .on_toggle(move |v| Message::SettingChanged(key, v.to_string()))
            .into(),
    )
}

fn labeled_text_input<'a>(
    label: String,
    value: &'a str,
    key: SettingKey,
    secure: bool,
    placeholder: &str,
) -> Element<'a, Message> {
    let mut input = theme::input_layout(
        text_input(placeholder, value)
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
    placeholder: String,
) -> Element<'a, Message> {
    row![]
        .push(text(label).size(FONT_MEDIUM).width(Length::Fixed(200.0)))
        .push(theme::editor_layout(
            text_editor(content)
                .placeholder(placeholder)
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
        .push(text(label).size(FONT_MEDIUM).width(Length::Fixed(200.0)))
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
            .spacing(SPACE_LG)
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
    text(fluent.get(key)).size(FONT_TITLE).color(accent).into()
}
