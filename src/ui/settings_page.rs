use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use iced::widget::{
    button, checkbox, column, container, pick_list, row, text, text_editor, text_input, toggler,
};
use iced::{Alignment, Element, Length};

use crate::config::Settings;
use crate::i18n::{Fluent, Locale, Tr};
use crate::message::{
    AddMsg, EngineMsg, Message, PathPickerId, SettingKey, SettingValue, SettingsCategory,
    SettingsMsg, SpeedUnit, TaskMsg,
};
use chrono::TimeZone;
use iced::Color;

use crate::ui::components::number_stepper::number_stepper;
use crate::ui::components::path_picker::{PathPicker, PathPickerEvent};
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::components::tag_picker::tag_picker;
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
    pub schedule_days_menu_open: bool,
    pub custom_tracker_input: String,
    pub syncing_trackers: bool,
    pub tracker_sync_toast_id: Option<u64>,
    pub readonly_hovered: HashSet<String>,
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
            schedule_days_menu_open: false,
            custom_tracker_input: String::new(),
            syncing_trackers: false,
            tracker_sync_toast_id: None,
            readonly_hovered: HashSet::new(),
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

pub struct SettingsPageContext<'a> {
    pub fluent: &'a Fluent,
    pub theme: &'a iced::Theme,
    pub settings: &'a Settings,
    pub settings_ui: &'a SettingsUiState,
    pub category: SettingsCategory,
    pub applied_settings: &'a Settings,
    pub engine_restart_pending: bool,
    pub engine_restart_in_progress: bool,
    pub aria2_version: Option<&'a str>,
    pub aria2_check_msg: Option<&'a str>,
    pub aria2_status: Option<(&'a str, &'a str)>,
    pub aria2_fetch_error: Option<&'a str>,
    pub update_pending: Option<&'a str>,
    pub ua_editor: &'a text_editor::Content,
    pub bt_tracker_editor: &'a text_editor::Content,
    pub path_history: &'a HashMap<String, Vec<String>>,
    pub font_restart_required: bool,
}

pub fn view<'a>(ctx: &SettingsPageContext<'a>) -> Element<'a, Message> {
    let SettingsPageContext {
        fluent,
        theme,
        settings,
        settings_ui,
        category,
        applied_settings,
        engine_restart_pending,
        engine_restart_in_progress,
        aria2_version,
        aria2_check_msg,
        aria2_status,
        aria2_fetch_error,
        update_pending,
        ua_editor,
        bt_tracker_editor,
        path_history,
        font_restart_required,
    } = ctx;
    let accent = theme::accent(theme);
    let dirty = settings != applied_settings;
    let content = match category {
        SettingsCategory::General => general_view(fluent, theme, settings, *font_restart_required),
        SettingsCategory::Download => {
            download_view(fluent, theme, settings, settings_ui, path_history)
        }
        SettingsCategory::BitTorrent => bittorrent_view(
            fluent,
            settings,
            applied_settings,
            settings_ui,
            bt_tracker_editor,
            settings_ui.syncing_trackers,
            accent,
        ),
        SettingsCategory::Ed2k => ed2k_view(fluent, theme, settings, settings_ui),
        SettingsCategory::Network => network_view(fluent, settings, ua_editor, accent),
        SettingsCategory::Advanced => advanced_view(
            fluent,
            theme,
            settings,
            applied_settings,
            settings_ui,
            *engine_restart_pending,
            *aria2_version,
            *aria2_check_msg,
            *aria2_status,
            *aria2_fetch_error,
            *update_pending,
        ),
    };

    let mut body = column![]
        .push(text(settings_title(fluent, *category)).size(FONT_PAGE_TITLE))
        .push(iced::widget::Space::new().height(Length::Fixed(20.0)))
        .push(slim_scrollable(content).height(Length::Fill));

    let mut actions = row![].spacing(SPACE_2XL).width(Length::Fill);
    actions = actions.push(
        button(text(fluent.get(Tr::Apply)).size(FONT_BODY))
            .on_press_maybe(if dirty {
                Some(Message::Settings(SettingsMsg::ApplySettings))
            } else {
                None
            })
            .padding(PADDING_BUTTON_XL)
            .style(theme::style::button::primary()),
    );
    actions = actions.push(
        button(text(fluent.get(Tr::Reset)).size(FONT_BODY))
            .on_press_maybe(if dirty {
                Some(Message::Settings(SettingsMsg::ResetSettings))
            } else {
                None
            })
            .padding(PADDING_BUTTON_XL)
            .style(theme::style::button::secondary()),
    );
    actions = actions.push(
        button(
            row![
                icon::refresh().size(FONT_ICON),
                text(fluent.get(Tr::RestartEngine)).size(FONT_BODY),
            ]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center),
        )
        .on_press_maybe(if *engine_restart_in_progress {
            None
        } else {
            Some(Message::Engine(EngineMsg::RestartEngine))
        })
        .padding(PADDING_BUTTON_XL)
        .style(theme::style::button::secondary()),
    );
    actions = actions.push(iced::widget::Space::new().width(Length::Fill));
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
            |opt| Message::Settings(SettingsMsg::ThemeModeChanged(opt.value)),
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
            |opt| Message::Settings(SettingsMsg::LocaleChanged(opt.value)),
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
    let pick: Element<'a, Message> = pick_list(options, selected, |o| {
        Message::Settings(SettingsMsg::FontFamilyChanged(o.value))
    })
    .placeholder(&placeholder)
    .text_size(FONT_MEDIUM)
    .padding(theme::INPUT_PADDING)
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
                            Message::Add(AddMsg::PathPicker(PathPickerId::DownloadDir, e))
                        }),
                )
                .height(Length::Fixed(36.0))
                .align_y(Alignment::Center),
        )
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::ConnectionSegment, accent))
        .push(labeled_number(
            fluent.get(Tr::MaxConcurrent),
            settings.max_concurrent,
            1..=u32::MAX,
            1,
            SettingKey::MaxConcurrent,
        ))
        .push(labeled_number(
            fluent.get(Tr::Split),
            settings.split,
            1..=128u16,
            1,
            SettingKey::Split,
        ))
        .push(labeled_number(
            fluent.get(Tr::MaxConnectionPerServer),
            settings.aria2.max_connection_per_server,
            1..=16u32,
            1,
            SettingKey::MaxConnectionPerServer,
        ))
        .push(setting_row(
            fluent.get(Tr::MinSplitSize),
            row![]
                .spacing(SPACE_LG)
                .push(number_stepper(
                    settings.aria2.min_split_size_mb,
                    1..=1024u64,
                    1,
                    move |v| {
                        Message::Settings(SettingsMsg::SettingChanged(
                            SettingKey::MinSplitSize,
                            SettingValue::Num(v),
                        ))
                    },
                    Length::Fixed(160.0),
                ))
                .align_y(Alignment::Center)
                .into(),
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::ResumeRetry, accent))
        .push(labeled_number(
            fluent.get(Tr::MaxTries),
            settings.aria2.max_tries,
            0..=u32::MAX,
            1,
            SettingKey::MaxTries,
        ))
        .push(labeled_number(
            fluent.get(Tr::RetryWait),
            settings.aria2.retry_wait,
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
                settings.download_limit_kb,
                unit,
                move |v| {
                    Message::Settings(SettingsMsg::SettingChanged(
                        SettingKey::DownloadLimit,
                        SettingValue::Num(v),
                    ))
                },
                move |u| {
                    Message::Settings(SettingsMsg::SpeedUnitChanged(SettingKey::DownloadLimit, u))
                },
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
                settings.upload_limit_kb,
                unit,
                move |v| {
                    Message::Settings(SettingsMsg::SettingChanged(
                        SettingKey::UploadLimit,
                        SettingValue::Num(v),
                    ))
                },
                move |u| {
                    Message::Settings(SettingsMsg::SpeedUnitChanged(SettingKey::UploadLimit, u))
                },
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
                settings.aria2.max_download_limit_kb,
                unit,
                move |v| {
                    let kb = if unit == SpeedUnit::Kbps { v } else { v * 1024 };
                    Message::Settings(SettingsMsg::SettingChanged(
                        SettingKey::MaxDownloadLimit,
                        SettingValue::Num(kb),
                    ))
                },
                move |u| {
                    Message::Settings(SettingsMsg::SpeedUnitChanged(
                        SettingKey::MaxDownloadLimit,
                        u,
                    ))
                },
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
                settings.aria2.max_upload_limit_kb,
                unit,
                move |v| {
                    let kb = if unit == SpeedUnit::Kbps { v } else { v * 1024 };
                    Message::Settings(SettingsMsg::SettingChanged(
                        SettingKey::MaxUploadLimit,
                        SettingValue::Num(kb),
                    ))
                },
                move |u| {
                    Message::Settings(SettingsMsg::SpeedUnitChanged(SettingKey::MaxUploadLimit, u))
                },
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
                settings.aria2.lowest_speed_limit_kb,
                unit,
                move |v| {
                    let kb = if unit == SpeedUnit::Kbps { v } else { v * 1024 };
                    Message::Settings(SettingsMsg::SettingChanged(
                        SettingKey::LowestSpeedLimit,
                        SettingValue::Num(kb),
                    ))
                },
                move |u| {
                    Message::Settings(SettingsMsg::SpeedUnitChanged(
                        SettingKey::LowestSpeedLimit,
                        u,
                    ))
                },
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
                    labeled_pick(
                        fluent,
                        fluent.get(Tr::ScheduleStartTime),
                        time_pick_options(),
                        Some(settings.speed_limit_schedule.start.clone()),
                        move |opt| Message::Settings(SettingsMsg::SettingChanged(
                            SettingKey::ScheduleStart,
                            SettingValue::Text(opt.value)
                        )),
                    ),
                    labeled_pick(
                        fluent,
                        fluent.get(Tr::ScheduleEndTime),
                        time_pick_options(),
                        Some(settings.speed_limit_schedule.end.clone()),
                        move |opt| Message::Settings(SettingsMsg::SettingChanged(
                            SettingKey::ScheduleEnd,
                            SettingValue::Text(opt.value)
                        )),
                    ),
                    {
                        let day_labels = [
                            fluent.get(Tr::WeekdayMon),
                            fluent.get(Tr::WeekdayTue),
                            fluent.get(Tr::WeekdayWed),
                            fluent.get(Tr::WeekdayThu),
                            fluent.get(Tr::WeekdayFri),
                            fluent.get(Tr::WeekdaySat),
                            fluent.get(Tr::WeekdaySun),
                        ];
                        let options = day_labels
                            .iter()
                            .enumerate()
                            .map(|(i, label)| ((i + 1) as u8, label.clone()))
                            .collect::<Vec<_>>();
                        setting_row_auto(
                            fluent.get(Tr::ScheduleDays),
                            tag_picker(
                                options,
                                &settings.speed_limit_schedule.weekdays,
                                fluent.get(Tr::ScheduleDays),
                                settings_ui.schedule_days_menu_open,
                                move |day, enabled| {
                                    Message::Settings(SettingsMsg::ScheduleDayToggled {
                                        day,
                                        enabled,
                                    })
                                },
                                Message::Settings(SettingsMsg::ToggleScheduleDaysMenu),
                                Length::Fixed(360.0),
                            ),
                        )
                    },
                    setting_row_auto(
                        String::new(),
                        text(fluent.get(Tr::ScheduleHint))
                            .size(FONT_SMALL)
                            .style(theme::style::text::secondary)
                            .into(),
                    ),
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
    applied_settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    bt_tracker_editor: &'a text_editor::Content,
    syncing_trackers: bool,
    accent: Color,
) -> Element<'a, Message> {
    let tracker_count = crate::trackers::count(&settings.aria2.bt_tracker);
    let last_sync = match settings.tracker.last_sync_time {
        Some(ms) => match chrono::Local.timestamp_millis_opt(ms) {
            chrono::LocalResult::Single(t) => t.format("%Y-%m-%d %H:%M").to_string(),
            _ => "—".to_string(),
        },
        None => "—".to_string(),
    };
    let count_str = fluent.get_args(Tr::BtTrackerCount, &{
        let mut a = std::collections::HashMap::new();
        a.insert(
            std::borrow::Cow::from("count"),
            (tracker_count as i64).into(),
        );
        a
    });
    let last_sync_str = fluent.get_args(Tr::LastSyncTime, &{
        let mut a = std::collections::HashMap::new();
        a.insert(std::borrow::Cow::from("time"), last_sync.into());
        a
    });

    let mut tracker_rows: Vec<Element<'a, Message>> = Vec::new();
    tracker_rows.push(
        text(fluent.get(Tr::BtTrackerSourcePreset))
            .size(FONT_SMALL)
            .style(theme::style::text::secondary)
            .into(),
    );
    for (owner, repo, url) in crate::config::TRACKER_SOURCE_OPTIONS {
        let url_str = url.to_string();
        let checked = settings.tracker.sources.contains(&url_str);
        tracker_rows.push(setting_row(
            format!("{owner}/{repo}"),
            checkbox(checked)
                .on_toggle(move |enabled| {
                    Message::Settings(SettingsMsg::TrackerSourceToggled {
                        source: url_str.clone(),
                        enabled,
                    })
                })
                .into(),
        ));
    }
    let custom_placeholder = fluent.get(Tr::BtTrackerSourceCustomPlaceholder);
    tracker_rows.push(setting_row_auto(
        fluent.get(Tr::BtTrackerSourceCustom),
        row![
            text_input(&custom_placeholder, &settings_ui.custom_tracker_input)
                .on_input(|s| Message::Settings(SettingsMsg::TrackerCustomInputChanged(s)))
                .on_submit(Message::Settings(SettingsMsg::TrackerCustomAdd))
                .width(Length::Fill)
                .style(theme::style::input::standard),
            button(icon::plus().size(FONT_BODY))
                .on_press(Message::Settings(SettingsMsg::TrackerCustomAdd))
                .padding(PADDING_BUTTON_SM)
                .style(theme::style::button::secondary()),
        ]
        .spacing(SPACE_SM)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into(),
    ));
    for url in &settings.tracker.custom_urls {
        tracker_rows.push(setting_row_auto(
            String::new(),
            container(
                row![
                    text(url.clone())
                        .size(FONT_SMALL)
                        .width(Length::Fill)
                        .wrapping(text::Wrapping::Glyph),
                    button(icon::x().size(FONT_BODY))
                        .on_press(Message::Settings(SettingsMsg::TrackerCustomRemove(
                            url.clone()
                        )))
                        .padding(PADDING_BUTTON_SM)
                        .style(theme::style::button::text()),
                ]
                .spacing(SPACE_SM)
                .align_y(Alignment::Center)
                .width(Length::Fill),
            )
            .padding([6, 10])
            .width(Length::Fill)
            .style(theme::style::card)
            .into(),
        ));
    }
    tracker_rows.push(setting_row_auto(
        fluent.get(Tr::BtTrackerSync),
        button(
            row![
                icon::refresh().size(FONT_ICON),
                text(fluent.get(Tr::BtTrackerSync)).size(FONT_BODY),
            ]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center),
        )
        .on_press_maybe(
            if syncing_trackers || settings.tracker.sources != applied_settings.tracker.sources {
                None
            } else {
                Some(Message::Settings(SettingsMsg::SyncTrackers))
            },
        )
        .padding(PADDING_BUTTON_SM)
        .style(theme::style::button::secondary())
        .into(),
    ));
    tracker_rows.push(setting_row_auto(
        String::new(),
        text(format!("{count_str} · {last_sync_str}"))
            .size(FONT_SMALL)
            .style(theme::style::text::secondary)
            .into(),
    ));
    tracker_rows.push(labeled_editor(
        fluent.get(Tr::BtTracker),
        bt_tracker_editor,
        |a| Message::Settings(SettingsMsg::BtTrackerEditor(a)),
        fluent.get(Tr::BtTrackerInputTips),
        140.0,
    ));
    tracker_rows.push(labeled_toggle(
        fluent.get(Tr::AutoSync),
        settings.tracker.auto_sync,
        SettingKey::TrackerAutoSync,
    ));
    if settings.tracker.auto_sync {
        let freq_opts = vec![
            Labeled {
                value: 0,
                label: fluent.get(Tr::IntervalEveryStartup),
            },
            Labeled {
                value: 6,
                label: fluent.get(Tr::Interval6Hours),
            },
            Labeled {
                value: 12,
                label: fluent.get(Tr::Interval12Hours),
            },
            Labeled {
                value: 24,
                label: fluent.get(Tr::IntervalDaily),
            },
            Labeled {
                value: 168,
                label: fluent.get(Tr::IntervalWeekly),
            },
        ];
        tracker_rows.push(labeled_pick(
            fluent,
            fluent.get(Tr::SyncFrequency),
            freq_opts,
            Some(settings.tracker.sync_interval_hours),
            |opt| {
                Message::Settings(SettingsMsg::SettingChanged(
                    SettingKey::TrackerSyncInterval,
                    SettingValue::Num(opt.value as u64),
                ))
            },
        ));
    }

    let mut bt_col = column![]
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
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::BtTrackers, accent))
        .push(iced::widget::Space::new().height(Length::Fixed(8.0)));
    for row in tracker_rows {
        bt_col = bt_col.push(row);
    }
    let bt_col = bt_col
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
            settings.aria2.seed_ratio,
            0.0..=100.0f64,
            0.1,
            SettingKey::SeedRatio,
        ))
        .push(labeled_number(
            fluent.get(Tr::SeedTime),
            settings.aria2.seed_time,
            0..=u32::MAX,
            1,
            SettingKey::SeedTime,
        ));
    bt_col.into()
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
                            Message::Add(AddMsg::PathPicker(PathPickerId::Ed2kServerList, e))
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
                            Message::Add(AddMsg::PathPicker(PathPickerId::Ed2kNodeList, e))
                        }),
                )
                .height(Length::Fixed(36.0))
                .align_y(Alignment::Center),
        )
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Network, accent))
        .push(labeled_number(
            fluent.get(Tr::Ed2kListenPort),
            settings.aria2.ed2k_listen_port,
            0..=65535u16,
            1,
            SettingKey::Ed2kListenPort,
        ))
        .push(labeled_number(
            fluent.get(Tr::Ed2kUdpListenPort),
            settings.aria2.ed2k_udp_listen_port,
            0..=65535u16,
            1,
            SettingKey::Ed2kUdpListenPort,
        ))
        .push(labeled_number(
            fluent.get(Tr::Ed2kUploadSlots),
            settings.aria2.ed2k_upload_slots,
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
                |a| Message::Settings(SettingsMsg::UaEditor(a)),
                placeholder,
                80.0,
            )
        })
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::ConnectTimeout, accent))
        .push(labeled_number(
            fluent.get(Tr::ConnectTimeout),
            settings.aria2.connect_timeout,
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
    applied_settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    engine_restart_pending: bool,
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
            .on_toggle(|v| Message::Settings(SettingsMsg::SetAutoCheck(v)))
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
        let dir_str = dir.to_string_lossy().into_owned();
        engine_rows.push(labeled_readonly(
            fluent,
            theme,
            fluent.get(Tr::EngineDataDir),
            &dir_str,
            settings_ui.readonly_hovered.contains(&dir_str),
        ));
    }
    if let Some(path) = crate::config::session_dir() {
        let sf = path.join("session.txt");
        let sf_str = sf.to_string_lossy().into_owned();
        engine_rows.push(labeled_readonly(
            fluent,
            theme,
            fluent.get(Tr::EngineSessionFile),
            &sf_str,
            settings_ui.readonly_hovered.contains(&sf_str),
        ));
    }
    if let Some(path) = crate::config::config_file_path() {
        let p_str = path.to_string_lossy().into_owned();
        engine_rows.push(labeled_readonly(
            fluent,
            theme,
            fluent.get(Tr::ConfigFile),
            &p_str,
            settings_ui.readonly_hovered.contains(&p_str),
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
                .on_press(Message::Engine(EngineMsg::RestartEngine))
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
                .on_press(Message::Engine(EngineMsg::RetryAria2Fetch))
                .padding(PADDING_BUTTON_SM)
                .style(theme::style::button::secondary()),
        );
    } else {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::CheckUpdate)).size(FONT_SMALL))
                .on_press(Message::Engine(EngineMsg::CheckAria2Update))
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
                |opt| {
                    Message::Settings(SettingsMsg::SettingChanged(
                        SettingKey::FileAllocation,
                        SettingValue::Text(opt.value),
                    ))
                },
            )
        })
        .push(labeled_number(
            fluent.get(Tr::DiskCache),
            settings.aria2.disk_cache_mb,
            0..=u64::MAX,
            1,
            SettingKey::DiskCache,
        ))
        .push(group_title(fluent, Tr::Logging, accent))
        .push(logging_view(
            fluent,
            theme,
            settings,
            applied_settings,
            settings_ui,
            engine_restart_pending,
        ))
        .push(group_title(fluent, Tr::Engine, accent))
        .push(engine_col)
        .into()
}

fn logging_view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    settings: &'a Settings,
    applied_settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    engine_restart_pending: bool,
) -> Element<'a, Message> {
    let placeholder = fluent.get(Tr::SelectPlaceholder);

    let app_opts: Vec<Labeled<String>> = crate::logging::app_level_options()
        .iter()
        .map(|level| Labeled {
            value: level.to_string(),
            label: level_label(fluent, level),
        })
        .collect();
    let engine_opts: Vec<Labeled<String>> = crate::logging::engine_level_options()
        .iter()
        .map(|level| Labeled {
            value: level.to_string(),
            label: level_label(fluent, level),
        })
        .collect();

    let sel_app = app_opts
        .iter()
        .find(|o| o.value == settings.log.app_level)
        .cloned();
    let sel_engine = engine_opts
        .iter()
        .find(|o| o.value == settings.log.engine_level)
        .cloned();

    let mut col = column![].spacing(SPACE_SM);

    if let Some(dir) = crate::config::log_dir() {
        let dir_str = dir.to_string_lossy().into_owned();
        col = col.push(labeled_readonly(
            fluent,
            theme,
            fluent.get(Tr::LogLocation),
            &dir_str,
            settings_ui.readonly_hovered.contains(&dir_str),
        ));
    }

    col = col.push(setting_row(
        fluent.get(Tr::LogLevelApp),
        pick_list(app_opts, sel_app, |opt| {
            Message::Settings(SettingsMsg::SettingChanged(
                SettingKey::AppLogLevel,
                SettingValue::Text(opt.value),
            ))
        })
        .placeholder(&placeholder)
        .text_size(FONT_MEDIUM)
        .padding(theme::INPUT_PADDING)
        .width(Length::Fixed(140.0))
        .style(theme::style::pick_list::standard)
        .menu_style(theme::style::pick_list::menu)
        .into(),
    ));
    col = col.push(setting_row(
        fluent.get(Tr::LogLevelEngine),
        pick_list(engine_opts, sel_engine, |opt| {
            Message::Settings(SettingsMsg::SettingChanged(
                SettingKey::EngineLogLevel,
                SettingValue::Text(opt.value),
            ))
        })
        .placeholder(&placeholder)
        .text_size(FONT_MEDIUM)
        .padding(theme::INPUT_PADDING)
        .width(Length::Fixed(140.0))
        .style(theme::style::pick_list::standard)
        .menu_style(theme::style::pick_list::menu)
        .into(),
    ));

    if engine_restart_pending || settings.log.engine_level != applied_settings.log.engine_level {
        col = col.push(
            text(fluent.get(Tr::LogLevelEngineHint))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        );
    }

    col = col.push(setting_row(
        String::new(),
        button(text(fluent.get(Tr::ClearLogs)).size(FONT_BODY))
            .on_press(Message::Settings(SettingsMsg::ClearLogs))
            .padding(PADDING_BUTTON_SM)
            .style(theme::style::button::secondary())
            .into(),
    ));

    col.into()
}

fn level_label(fluent: &Fluent, level: &str) -> String {
    let key = match level {
        "trace" => Tr::LevelTrace,
        "debug" => Tr::LevelDebug,
        "info" => Tr::LevelInfo,
        "notice" => Tr::LevelNotice,
        "warn" => Tr::LevelWarn,
        "error" => Tr::LevelError,
        _ => return level.to_string(),
    };
    fluent.get(key)
}

fn setting_row<'a>(label: String, control: Element<'a, Message>) -> Element<'a, Message> {
    row![]
        .push(text(label).size(FONT_MEDIUM).width(Length::Fixed(200.0)))
        .push(control)
        .height(Length::Fixed(36.0))
        .align_y(Alignment::Center)
        .into()
}

trait ToSettingValue {
    fn to_setting_value(self) -> SettingValue;
}

impl ToSettingValue for u16 {
    fn to_setting_value(self) -> SettingValue {
        SettingValue::Num(self as u64)
    }
}

impl ToSettingValue for u32 {
    fn to_setting_value(self) -> SettingValue {
        SettingValue::Num(self as u64)
    }
}

impl ToSettingValue for u64 {
    fn to_setting_value(self) -> SettingValue {
        SettingValue::Num(self)
    }
}

impl ToSettingValue for f64 {
    fn to_setting_value(self) -> SettingValue {
        SettingValue::NumF(self)
    }
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
    value: T,
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
        + ToSettingValue
        + 'static,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    setting_row(
        label,
        number_stepper(
            value,
            bounds,
            step,
            move |v| Message::Settings(SettingsMsg::SettingChanged(key, v.to_setting_value())),
            Length::Fixed(160.0),
        ),
    )
}

fn labeled_toggle<'a>(label: String, value: bool, key: SettingKey) -> Element<'a, Message> {
    setting_row(
        label,
        toggler(value)
            .on_toggle(move |v| {
                Message::Settings(SettingsMsg::SettingChanged(key, SettingValue::Bool(v)))
            })
            .width(Length::Fixed(50.0))
            .into(),
    )
}

fn labeled_checkbox<'a>(label: String, value: bool, key: SettingKey) -> Element<'a, Message> {
    setting_row(
        label,
        checkbox(value)
            .on_toggle(move |v| {
                Message::Settings(SettingsMsg::SettingChanged(key, SettingValue::Bool(v)))
            })
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
            .on_input(move |s| {
                Message::Settings(SettingsMsg::SettingChanged(key, SettingValue::Text(s)))
            })
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
    height: f32,
) -> Element<'a, Message> {
    row![]
        .push(text(label).size(FONT_MEDIUM).width(Length::Fixed(200.0)))
        .push(theme::editor_layout(
            text_editor(content)
                .placeholder(placeholder)
                .on_action(on_edit)
                .height(Length::Fixed(height))
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
            .text_size(FONT_MEDIUM)
            .padding(theme::INPUT_PADDING)
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
    hovered: bool,
) -> Element<'a, Message> {
    let mut picker = PathPicker::read_only(value.to_string());
    picker.set_hovered(hovered);
    let open_value = value.to_string();
    row![]
        .push(text(label).size(FONT_MEDIUM).width(Length::Fixed(200.0)))
        .push(picker.view(fluent, theme, &[], move |e| match e {
            PathPickerEvent::Copy(s) => Message::Task(TaskMsg::CopyPath(s)),
            PathPickerEvent::Open => {
                Message::Task(TaskMsg::OpenFolder(PathBuf::from(open_value.clone())))
            }
            PathPickerEvent::Entered => Message::Settings(SettingsMsg::ReadOnlyHover {
                path: open_value.clone(),
                hovered: true,
            }),
            PathPickerEvent::Exited => Message::Settings(SettingsMsg::ReadOnlyHover {
                path: open_value.clone(),
                hovered: false,
            }),
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
    value_kb: u64,
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
        SpeedUnit::Kbps => (value_kb, 100),
        SpeedUnit::Mbps => {
            if value_kb == 0 {
                (0, 1)
            } else {
                (value_kb / 1024, 1)
            }
        }
    };

    setting_row(
        label,
        row![]
            .spacing(SPACE_LG)
            .push(number_stepper(
                display_val,
                0..=u64::MAX,
                step,
                on_value,
                Length::Fixed(160.0),
            ))
            .push(
                pick_list(unit_opts, sel, move |o| on_unit(o.value))
                    .text_size(FONT_MEDIUM)
                    .padding(theme::INPUT_PADDING)
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

fn time_pick_options() -> Vec<Labeled<String>> {
    (0..48)
        .map(|i| {
            let minutes = i * 30;
            let value = format!("{:02}:{:02}", minutes / 60, minutes % 60);
            Labeled {
                value: value.clone(),
                label: value,
            }
        })
        .collect()
}
