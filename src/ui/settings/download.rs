use std::collections::HashMap;

use iced::widget::{column, row, text};
use iced::{Alignment, Element, Length};

use super::{
    group_title, labeled_hint, labeled_number, labeled_pick, labeled_toggle, setting_row,
    setting_row_auto, speed_labeled_input, sub_items, time_pick_options, Fluent, Message,
    SettingKey, SettingValue, Settings, SettingsMsg, SettingsUiState, SpeedUnit, Tr, FONT_MEDIUM,
    SPACE_LG, SPACE_SM,
};
use crate::ui::components::number_stepper::number_stepper;
use crate::ui::components::tag_picker::tag_picker;

pub(super) fn download_view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    path_history: &'a HashMap<String, Vec<String>>,
) -> Element<'a, Message> {
    use crate::message::{AddMsg, PathPickerId};
    let accent = super::theme::accent(theme);
    let download_hist: &[String] = path_history
        .get("download_dir")
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    column![]
        .spacing(SPACE_SM)
        .push(crate::ui::components::scroll_top_gap::view())
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
            1..=crate::config::MAX_CONCURRENT_DOWNLOADS,
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
                sub_items([
                    labeled_pick(
                        fluent,
                        fluent.get(Tr::ScheduleStartTime),
                        time_pick_options(),
                        Some(settings.speed_limit_schedule.start.clone()),
                        move |opt| {
                            Message::Settings(SettingsMsg::SettingChanged(
                                SettingKey::ScheduleStart,
                                SettingValue::Text(opt.value),
                            ))
                        },
                    ),
                    labeled_pick(
                        fluent,
                        fluent.get(Tr::ScheduleEndTime),
                        time_pick_options(),
                        Some(settings.speed_limit_schedule.end.clone()),
                        move |opt| {
                            Message::Settings(SettingsMsg::SettingChanged(
                                SettingKey::ScheduleEnd,
                                SettingValue::Text(opt.value),
                            ))
                        },
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
                    labeled_hint(fluent.get(Tr::ScheduleHint)),
                ])
            } else {
                iced::widget::Space::new().height(Length::Fixed(0.0)).into()
            };
            el
        })
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
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::NotificationsConfirmations, accent))
        .push(labeled_toggle(
            fluent.get(Tr::NotifyDownloadComplete),
            settings.notifications.download_complete,
            SettingKey::NotificationDownloadComplete,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::NotifyDownloadError),
            settings.notifications.download_error,
            SettingKey::NotificationDownloadError,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::NotifyEngineDegraded),
            settings.notifications.engine_degraded,
            SettingKey::NotificationEngineDegraded,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::NotifyDownloadAdded),
            settings.notifications.download_added,
            SettingKey::NotificationDownloadAdded,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::PreventSleep),
            settings.prevent_sleep,
            SettingKey::PreventSleep,
        ))
        .push(labeled_toggle(
            fluent.get(Tr::NavToTasksAfterAdd),
            settings.nav_to_tasks_after_add,
            SettingKey::NavToTasksAfterAdd,
        ))
        .into()
}
