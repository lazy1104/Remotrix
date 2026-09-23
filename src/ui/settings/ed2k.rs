use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use super::{
    group_title, labeled_hint, labeled_number, labeled_pick, labeled_port, labeled_text_input,
    labeled_toggle, setting_row, Ed2kSearchSession, Fluent, Labeled, Message, SettingKey,
    SettingValue, Settings, SettingsMsg, SettingsUiState, Tr, FONT_BODY, FONT_ICON, FONT_MEDIUM,
    FONT_SMALL, PADDING_BUTTON_SM, SPACE_SM, SPACE_XS,
};
use crate::message::{AddMsg, PathPickerId};
use crate::task::format_size;
use crate::ui::icon;
use crate::ui::theme;

#[allow(clippy::too_many_arguments)]
pub(super) fn ed2k_view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    port_status: &'a std::collections::HashMap<
        crate::port_guard::PortKind,
        (u16, crate::port_guard::PortStatus),
    >,
) -> Element<'a, Message> {
    let accent = theme::accent(theme);
    let syncing_bootstrap = settings_ui.syncing_bootstrap;
    let bootstrap_auto = settings.aria2.ed2k_bootstrap_auto_sync;
    let server_met_placeholder = fluent.get_args(
        Tr::Ed2kBootstrapServerMetUrlPlaceholder,
        &Default::default(),
    );
    let nodes_dat_placeholder =
        fluent.get_args(Tr::Ed2kBootstrapNodesDatUrlPlaceholder, &Default::default());
    let (server_met_path_str, nodes_dat_path_str) = settings_ui
        .ed2k_bootstrap_status
        .0
        .map(|_| ())
        .map(|_| {
            (
                crate::ed2k_bootstrap::server_met_path()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                crate::ed2k_bootstrap::nodes_dat_path()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
        })
        .unwrap_or_default();
    column![]
        .spacing(SPACE_SM)
        .push(crate::ui::components::scroll_top_gap::view())
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
                .push(if bootstrap_auto {
                    text(server_met_path_str)
                        .size(FONT_SMALL)
                        .style(theme::style::text::secondary)
                        .into()
                } else {
                    settings_ui
                        .ed2k_server_list_picker
                        .view(fluent, theme, &[], |e| {
                            Message::Add(AddMsg::PathPicker(PathPickerId::Ed2kServerList, e))
                        })
                })
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
                .push(if bootstrap_auto {
                    text(nodes_dat_path_str)
                        .size(FONT_SMALL)
                        .style(theme::style::text::secondary)
                        .into()
                } else {
                    settings_ui
                        .ed2k_node_list_picker
                        .view(fluent, theme, &[], |e| {
                            Message::Add(AddMsg::PathPicker(PathPickerId::Ed2kNodeList, e))
                        })
                })
                .height(Length::Fixed(36.0))
                .align_y(Alignment::Center),
        )
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Network, accent))
        .push(labeled_port(
            fluent,
            theme,
            fluent.get(Tr::Ed2kListenPort),
            settings.aria2.ed2k_listen_port,
            0..=65535u16,
            SettingKey::Ed2kListenPort,
            crate::port_guard::PortKind::Ed2k,
            port_status,
        ))
        .push(labeled_port(
            fluent,
            theme,
            fluent.get(Tr::Ed2kUdpListenPort),
            settings.aria2.ed2k_udp_listen_port,
            0..=65535u16,
            SettingKey::Ed2kUdpListenPort,
            crate::port_guard::PortKind::Ed2kUdp,
            port_status,
        ))
        .push(labeled_number(
            fluent.get(Tr::Ed2kUploadSlots),
            settings.aria2.ed2k_upload_slots,
            1..=u16::MAX,
            1,
            SettingKey::Ed2kUploadSlots,
        ))
        .push(iced::widget::Space::new().height(Length::Fixed(8.0)))
        .push(labeled_hint(fluent.get(Tr::Ed2kRestartHint)))
        .push(iced::widget::Space::new().height(Length::Fixed(16.0)))
        .push(group_title(fluent, Tr::Ed2kBootstrapSync, accent))
        .push(labeled_toggle(
            fluent.get(Tr::Ed2kBootstrapAutoSync),
            settings.aria2.ed2k_bootstrap_auto_sync,
            SettingKey::Ed2kBootstrapAutoSync,
        ))
        .push(labeled_number(
            fluent.get(Tr::Ed2kBootstrapSyncInterval),
            settings.aria2.ed2k_bootstrap_sync_interval_hours,
            1..=u32::MAX,
            1,
            SettingKey::Ed2kBootstrapSyncInterval,
        ))
        .push(labeled_text_input(
            fluent.get(Tr::Ed2kBootstrapServerMetUrl),
            &settings.aria2.ed2k_server_met_url,
            SettingKey::Ed2kServerMetUrl,
            false,
            &server_met_placeholder,
        ))
        .push(labeled_text_input(
            fluent.get(Tr::Ed2kBootstrapNodesDatUrl),
            &settings.aria2.ed2k_nodes_dat_url,
            SettingKey::Ed2kNodesDatUrl,
            false,
            &nodes_dat_placeholder,
        ))
        .push(setting_row(
            fluent.get(Tr::Ed2kBootstrapSyncNow),
            column![
                button(
                    row![
                        if syncing_bootstrap {
                            crate::ui::components::spinner::Spinner::refresh(
                                accent,
                                FONT_ICON as f32,
                            )
                            .view()
                        } else {
                            icon::circle_fading_arrow_up().size(FONT_ICON).into()
                        },
                        text(fluent.get(Tr::Ed2kBootstrapSyncNow)).size(FONT_BODY),
                    ]
                    .spacing(super::SPACE_SM)
                    .align_y(Alignment::Center),
                )
                .on_press_maybe(if syncing_bootstrap {
                    None
                } else {
                    Some(Message::Settings(SettingsMsg::Ed2kBootstrapSyncNow))
                })
                .padding(PADDING_BUTTON_SM)
                .height(Length::Fixed(crate::ui::components::CONTROL_HEIGHT))
                .style(theme::style::button::secondary()),
                bootstrap_status_column(fluent, settings_ui),
            ]
            .spacing(SPACE_XS)
            .into(),
        ))
        .push(follow_metalink_row(fluent, theme, settings, accent))
        .into()
}

pub(super) fn bootstrap_status_column<'a>(
    fluent: &'a Fluent,
    settings_ui: &'a SettingsUiState,
) -> Element<'a, Message> {
    let (sm_modified, nd_modified) = settings_ui.ed2k_bootstrap_status;
    let fmt = |ms: Option<i64>| {
        ms.and_then(chrono::DateTime::from_timestamp_millis)
            .map(|dt| {
                let local: chrono::DateTime<chrono::Local> = dt.with_timezone(&chrono::Local);
                local.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_else(|| fluent.get(Tr::Never))
    };
    let (sm_path, nd_path) = (
        crate::ed2k_bootstrap::server_met_path(),
        crate::ed2k_bootstrap::nodes_dat_path(),
    );
    let sm_size = sm_path
        .as_deref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| format_size(m.len()));
    let nd_size = nd_path
        .as_deref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| format_size(m.len()));
    column![
        text(format!(
            "{}: {}",
            fluent.get(Tr::Ed2kBootstrapServerMetModified),
            fmt(sm_modified)
        ))
        .size(FONT_SMALL)
        .style(theme::style::text::secondary),
        text(format!(
            "{}: {}",
            fluent.get(Tr::Ed2kBootstrapNodesDatModified),
            fmt(nd_modified)
        ))
        .size(FONT_SMALL)
        .style(theme::style::text::secondary),
        text(fluent.get_args(Tr::Ed2kBootstrapCacheStatus, &{
            let mut a = std::collections::HashMap::new();
            a.insert(
                std::borrow::Cow::from("server-met-size"),
                sm_size.unwrap_or_else(|| "-".into()).into(),
            );
            a.insert(
                std::borrow::Cow::from("nodes-dat-size"),
                nd_size.unwrap_or_else(|| "-".into()).into(),
            );
            a
        }))
        .size(FONT_SMALL)
        .style(theme::style::text::secondary),
        text(fluent.get(Tr::Ed2kBootstrapManagedDefaultHint))
            .size(FONT_SMALL)
            .style(theme::style::text::secondary),
    ]
    .spacing(SPACE_XS)
    .width(Length::Fill)
    .into()
}

pub(super) fn follow_metalink_row<'a>(
    fluent: &'a Fluent,
    _theme: &'a iced::Theme,
    settings: &'a Settings,
    accent: iced::Color,
) -> Element<'a, Message> {
    column![]
        .spacing(SPACE_SM)
        .push(group_title(fluent, Tr::Metalink, accent))
        .push(labeled_toggle(
            fluent.get(Tr::FollowMetalink),
            settings.aria2.follow_metalink,
            SettingKey::FollowMetalink,
        ))
        .push(labeled_hint(fluent.get(Tr::FollowMetalinkHint)))
        .into()
}

#[allow(dead_code)]
pub(super) fn ed2k_search_view<'a>(
    fluent: &'a Fluent,
    _theme: &'a iced::Theme,
    settings_ui: &'a SettingsUiState,
) -> Element<'a, Message> {
    let state = &settings_ui.ed2k_search_state;
    let file_types = ["any", "audio", "video", "doc", "image", "arc"];
    let file_type_options: Vec<Labeled<String>> = file_types
        .iter()
        .map(|s| Labeled {
            value: s.to_string(),
            label: s.to_string(),
        })
        .collect();
    let btn_label = if state.sessions.is_empty() {
        Tr::Ed2kSearchSubmit
    } else {
        Tr::Ed2kSearchCancel
    };
    let btn_msg = if state.sessions.is_empty() {
        Message::Settings(SettingsMsg::Ed2kSearchSubmit)
    } else {
        Message::Settings(SettingsMsg::Ed2kSearchCancel)
    };
    let mut col = column![]
        .spacing(SPACE_SM)
        .push(labeled_text_input(
            fluent.get(Tr::Ed2kSearchKeyword),
            &state.keyword,
            SettingKey::Ed2kSearchKeyword,
            false,
            "",
        ))
        .push(labeled_pick(
            fluent,
            fluent.get(Tr::Ed2kSearchFileType),
            file_type_options,
            Some(state.file_type.clone()),
            |opt| {
                Message::Settings(SettingsMsg::SettingChanged(
                    SettingKey::Ed2kSearchFileType,
                    SettingValue::Text(opt.value),
                ))
            },
        ))
        .push(labeled_number(
            fluent.get(Tr::Ed2kSearchMinSources),
            state.min_sources,
            1..=1000u32,
            1,
            SettingKey::Ed2kSearchMinSources,
        ))
        .push(labeled_number(
            fluent.get(Tr::Ed2kSearchTimeout),
            state.timeout_secs,
            10..=600u32,
            1,
            SettingKey::Ed2kSearchTimeout,
        ))
        .push(
            button(text(fluent.get(btn_label)).size(FONT_BODY))
                .on_press(btn_msg)
                .padding(PADDING_BUTTON_SM)
                .style(theme::style::button::secondary()),
        );
    if !state.sessions.is_empty() {
        col = col.push(
            text(format!(
                "{}: {}",
                fluent.get(Tr::Ed2kSearchProgress),
                state.sessions.len()
            ))
            .size(FONT_SMALL)
            .style(theme::style::text::secondary),
        );
        let mut sessions: Vec<(&String, &Ed2kSearchSession)> = state.sessions.iter().collect();
        sessions.sort_by_key(|s| std::cmp::Reverse(s.1.started_at_ms));
        for (_, session) in sessions {
            col = col.push(ed2k_search_session_card(fluent, session));
        }
    }
    col.into()
}

#[allow(dead_code)]
pub(super) fn ed2k_search_session_card<'a>(
    fluent: &'a Fluent,
    session: &'a Ed2kSearchSession,
) -> Element<'a, Message> {
    let entries = parse_ed2k_results(&session.results);
    let is_complete = session
        .results
        .get("moreResults")
        .and_then(|v| v.as_bool())
        .map(|more| !more)
        .unwrap_or(false);
    let status_text = if is_complete {
        fluent.get(Tr::Ed2kSearchCompleted)
    } else {
        fluent.get(Tr::Ed2kSearchProgress)
    };
    let all_uris: Vec<String> = entries.iter().filter_map(|e| e.ed2k_link.clone()).collect();
    let mut body = column![].spacing(SPACE_XS);
    if entries.is_empty() {
        body = body.push(
            text(fluent.get(Tr::Ed2kSearchEmpty))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        );
    } else {
        if all_uris.len() > 1 {
            body = body.push(
                button(text(fluent.get(Tr::Ed2kSearchAddAll)).size(FONT_BODY))
                    .on_press(Message::Add(AddMsg::AddFromEd2kResult(all_uris)))
                    .padding(PADDING_BUTTON_SM)
                    .style(theme::style::button::secondary()),
            );
        }
        for entry in &entries {
            let name = entry.name.clone();
            let size = entry.size_bytes.map(format_size).unwrap_or_default();
            let sources = entry.source_count.to_string();
            let uris = entry.ed2k_link.clone().map(|u| vec![u]).unwrap_or_default();
            body = body.push(
                container(
                    row![
                        text(name)
                            .size(FONT_SMALL)
                            .width(Length::Fill)
                            .wrapping(text::Wrapping::Glyph),
                        text(size).size(FONT_SMALL).width(Length::Fixed(80.0)),
                        text(sources).size(FONT_SMALL).width(Length::Fixed(50.0)),
                        button(icon::plus().size(FONT_BODY))
                            .on_press(Message::Add(AddMsg::AddFromEd2kResult(uris)))
                            .padding(PADDING_BUTTON_SM)
                            .style(theme::style::button::text()),
                    ]
                    .spacing(super::SPACE_SM)
                    .align_y(Alignment::Center)
                    .width(Length::Fill),
                )
                .padding([4, 8])
                .width(Length::Fill)
                .style(theme::style::card),
            );
        }
    }
    column![]
        .spacing(SPACE_XS)
        .push(
            row![
                text(session.keyword.clone())
                    .size(FONT_MEDIUM)
                    .width(Length::Fill),
                text(status_text).size(FONT_SMALL),
            ]
            .spacing(super::SPACE_SM)
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .push(body)
        .into()
}

#[allow(dead_code)]
pub(super) struct Ed2kResultEntry {
    pub name: String,
    pub size_bytes: Option<u64>,
    pub source_count: u64,
    pub ed2k_link: Option<String>,
}

#[allow(dead_code)]
pub(super) fn parse_ed2k_results(value: &serde_json::Value) -> Vec<Ed2kResultEntry> {
    let Some(arr) = value.get("results").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .map(|v| {
            let name = v
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let size_bytes = v
                .get("length")
                .and_then(|x| x.as_str())
                .and_then(|s| s.parse::<u64>().ok());
            let source_count = v.get("sourceCount").and_then(|x| x.as_u64()).unwrap_or(0);
            let ed2k_link = v
                .get("ed2kLink")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            Ed2kResultEntry {
                name,
                size_bytes,
                source_count,
                ed2k_link,
            }
        })
        .collect()
}

// engine_status_color is defined in the parent module since it is used by
// advanced_view but kept alongside other engine-status helpers.
