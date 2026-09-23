use chrono::TimeZone;
use iced::widget::{button, checkbox, column, container, mouse_area, row, text};
use iced::{Alignment, Element, Length};

use super::{
    group_title, labeled_editor, labeled_hint, labeled_number, labeled_pick, labeled_toggle,
    setting_row, sub_items, CtxMirrors, Fluent, Labeled, Message, SettingKey, SettingValue,
    Settings, SettingsMsg, SettingsUiState, Tr, FONT_BODY, FONT_ICON, FONT_MEDIUM, FONT_SMALL,
    PADDING_BUTTON_SM, SPACE_SM,
};
use crate::message::CtxTarget;
use crate::ui::components::ctx_input;
use crate::ui::icon;
use crate::ui::theme;

#[allow(clippy::too_many_arguments)]
pub(super) fn bittorrent_view<'a>(
    fluent: &'a Fluent,
    settings: &'a Settings,
    applied_settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    bt_tracker_editor: &'a iced::widget::text_editor::Content,
    syncing_trackers: bool,
    accent: iced::Color,
    ctx_mirrors: &CtxMirrors,
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
    tracker_rows.push(setting_row(
        fluent.get(Tr::BtTrackerSourceCustom),
        row![
            mouse_area(
                ctx_input::CtxInput::new(
                    &custom_placeholder,
                    &settings_ui.custom_tracker_input,
                    ctx_mirrors
                        .get(&CtxTarget::SettingsCustomTracker)
                        .cloned()
                        .unwrap_or_default(),
                )
                .on_input(|s| Message::Settings(SettingsMsg::TrackerCustomInputChanged(s)))
                .on_submit(Message::Settings(SettingsMsg::TrackerCustomAdd))
                .padding(theme::INPUT_PADDING)
                .size(FONT_MEDIUM)
                .width(Length::Fill)
                .style(theme::style::input::standard),
            )
            .on_right_press(Message::CtxOpen(CtxTarget::SettingsCustomTracker)),
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
        tracker_rows.push(setting_row(
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
    tracker_rows.push(setting_row(
        fluent.get(Tr::BtTrackerSync),
        button(
            row![
                if syncing_trackers {
                    crate::ui::components::spinner::Spinner::refresh(accent, FONT_ICON as f32)
                        .view()
                } else {
                    icon::circle_fading_arrow_up().size(FONT_ICON).into()
                },
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
        .height(Length::Fixed(crate::ui::components::CONTROL_HEIGHT))
        .style(theme::style::button::secondary())
        .into(),
    ));
    tracker_rows.push(labeled_hint(format!("{count_str} · {last_sync_str}")));
    tracker_rows.push(labeled_editor(
        fluent.get(Tr::BtTracker),
        bt_tracker_editor,
        |a| Message::Settings(SettingsMsg::BtTrackerEditor(a)),
        fluent.get(Tr::BtTrackerInputTips),
        140.0,
        CtxTarget::SettingsBtTracker,
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
        tracker_rows.push(sub_items([labeled_pick(
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
        )]));
    }

    let mut bt_col = column![]
        .spacing(SPACE_SM)
        .push(crate::ui::components::scroll_top_gap::view())
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
