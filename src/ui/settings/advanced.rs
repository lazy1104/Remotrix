use iced::widget::{button, column, container, pick_list, row, text};
use iced::{Alignment, Element, Length};

use super::{
    group_title, labeled_checkbox, labeled_hint, labeled_number, labeled_pick, labeled_port,
    labeled_toggle, level_label, setting_row, sub_items, Fluent, Labeled, Message, SettingKey,
    SettingValue, Settings, SettingsMsg, SettingsUiState, Tr, FONT_BODY, FONT_MEDIUM, FONT_SMALL,
    PADDING_BUTTON_SM, SPACE_2XL, SPACE_LG, SPACE_SM,
};
use crate::message::{AddMsg, EngineMsg, ExtensionMsg, PathPickerId};
use crate::ui::components::path_picker::PathPicker;
use crate::ui::theme;

pub(super) fn extension_view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    settings: &'a Settings,
    port_status: &'a std::collections::HashMap<
        crate::port_guard::PortKind,
        (u16, crate::port_guard::PortStatus),
    >,
) -> Element<'a, Message> {
    let accent = theme::accent(theme);
    let secret = settings.extension.secret.clone();
    let port_placeholder = crate::config::EXTENSION_API_DEFAULT_PORT.to_string();
    let mut col = column![].spacing(SPACE_SM);
    col = col
        .push(group_title(fluent, Tr::ExtensionCategory, accent))
        .push(labeled_toggle(
            fluent.get(Tr::ExtensionApiEnabled),
            settings.extension.enabled,
            SettingKey::ExtensionApiEnabled,
        ));
    if settings.extension.enabled {
        col = col.push(sub_items([
            labeled_port(
                fluent,
                theme,
                fluent.get(Tr::ExtensionApiPort),
                settings.extension.port,
                crate::config::EXTENSION_API_MIN_PORT..=crate::config::EXTENSION_API_MAX_PORT,
                SettingKey::ExtensionApiPort,
                crate::port_guard::PortKind::ExtensionApi,
                port_status,
            ),
            setting_row(
                fluent.get(Tr::ExtensionApiSecret),
                crate::ui::components::secret_input::secret_input(
                    fluent,
                    theme,
                    &secret,
                    &port_placeholder,
                    move |s| {
                        Message::Settings(SettingsMsg::SettingChanged(
                            SettingKey::ExtensionApiSecret,
                            SettingValue::Text(s),
                        ))
                    },
                    Message::Extension(ExtensionMsg::GenerateSecret),
                    Message::CopyText(secret.clone()),
                ),
            ),
            labeled_toggle(
                fluent.get(Tr::ExtensionAutoSubmit),
                settings.extension.auto_submit,
                SettingKey::ExtensionAutoSubmit,
            ),
            setting_row(
                String::new(),
                text(fluent.get_args(Tr::ExtensionSetupHint, &{
                    let mut args = std::collections::HashMap::new();
                    args.insert(
                        std::borrow::Cow::from("port"),
                        settings.extension.port.to_string().into(),
                    );
                    args
                }))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary)
                .wrapping(text::Wrapping::Glyph)
                .into(),
            ),
        ]));
    }
    col.into()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn advanced_view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    settings: &'a Settings,
    applied_settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
    engine_restart_pending: bool,
    aria2_status: Option<(&'a str, &'a str)>,
    aria2_fetch_error: Option<&'a str>,
    port_status: &'a std::collections::HashMap<
        crate::port_guard::PortKind,
        (u16, crate::port_guard::PortStatus),
    >,
) -> Element<'a, Message> {
    let accent = theme::accent(theme);

    let mut engine_rows: Vec<Element<Message>> = Vec::new();

    if let Some((_stage, message)) = aria2_status {
        let status_color = engine_status_color(theme, aria2_status, aria2_fetch_error);
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

    if let Some(_err) = aria2_fetch_error {
        btn_row = btn_row.push(
            button(text(fluent.get(Tr::Retry)).size(FONT_SMALL))
                .on_press(Message::Engine(EngineMsg::RetryAria2Fetch))
                .padding(PADDING_BUTTON_SM)
                .style(theme::style::button::secondary()),
        );
    }

    engine_rows.push(btn_row.into());

    let mut engine_col = column![].spacing(SPACE_LG);
    for elem in engine_rows {
        engine_col = engine_col.push(elem);
    }
    engine_col = engine_col.push(labeled_port(
        fluent,
        theme,
        fluent.get(Tr::RpcListenPort),
        settings.aria2.rpc_listen_port,
        0..=65535u16,
        SettingKey::RpcListenPort,
        crate::port_guard::PortKind::Rpc,
        port_status,
    ));

    let mut clipboard_col = column![].spacing(SPACE_SM);
    clipboard_col = clipboard_col
        .push(group_title(fluent, Tr::Clipboard, accent))
        .push(labeled_toggle(
            fluent.get(Tr::DetectClipboardOnStart),
            settings.detect_clipboard_on_start,
            SettingKey::DetectClipboardOnStart,
        ));
    if settings.detect_clipboard_on_start {
        clipboard_col = clipboard_col.push(sub_items([
            labeled_pick(
                fluent,
                fluent.get(Tr::WebpageFilter),
                vec![
                    Labeled {
                        value: crate::clipboard_watch::WebpageFilterMode::Smart,
                        label: fluent.get(Tr::WebpageFilterSmart),
                    },
                    Labeled {
                        value: crate::clipboard_watch::WebpageFilterMode::Static,
                        label: fluent.get(Tr::WebpageFilterStatic),
                    },
                    Labeled {
                        value: crate::clipboard_watch::WebpageFilterMode::Off,
                        label: fluent.get(Tr::WebpageFilterOff),
                    },
                ],
                Some(settings.webpage_filter),
                |opt| {
                    Message::Settings(SettingsMsg::SettingChanged(
                        SettingKey::ClipboardWebpageFilter,
                        SettingValue::Text(opt.value.as_str().into()),
                    ))
                },
            ),
            labeled_checkbox(
                fluent.get(Tr::LinkTypeHttp),
                settings.clipboard_types.http,
                SettingKey::ClipboardHttp,
            ),
            labeled_checkbox(
                fluent.get(Tr::LinkTypeFtp),
                settings.clipboard_types.ftp,
                SettingKey::ClipboardFtp,
            ),
            labeled_checkbox(
                fluent.get(Tr::LinkTypeMagnet),
                settings.clipboard_types.magnet,
                SettingKey::ClipboardMagnet,
            ),
            labeled_checkbox(
                fluent.get(Tr::LinkTypeEd2k),
                settings.clipboard_types.ed2k,
                SettingKey::ClipboardEd2k,
            ),
            labeled_checkbox(
                fluent.get(Tr::LinkTypeThunder),
                settings.clipboard_types.thunder,
                SettingKey::ClipboardThunder,
            ),
            labeled_checkbox(
                fluent.get(Tr::LinkTypeBtInfohash),
                settings.clipboard_types.bt_infohash,
                SettingKey::ClipboardBtInfohash,
            ),
        ]));
    }

    column![]
        .spacing(SPACE_2XL)
        .push(crate::ui::components::scroll_top_gap::view())
        .push(extension_view(fluent, theme, settings, port_status))
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
            settings,
            applied_settings,
            engine_restart_pending,
        ))
        .push(group_title(fluent, Tr::PathsSectionTitle, accent))
        .push(paths_section(
            fluent,
            theme,
            settings,
            applied_settings,
            settings_ui,
        ))
        .push(group_title(fluent, Tr::Engine, accent))
        .push(engine_col)
        .into()
}

pub(super) fn engine_status_color(
    theme: &iced::Theme,
    aria2_status: Option<(&str, &str)>,
    aria2_fetch_error: Option<&str>,
) -> iced::Color {
    if aria2_fetch_error.is_some() {
        return theme::danger(theme);
    }
    match aria2_status.map(|(stage, _)| stage) {
        Some("ready") => theme::success(theme),
        Some("update-downloading" | "update-verifying" | "starting") => theme::accent(theme),
        _ => theme::text_secondary(theme),
    }
}

pub(super) fn logging_view<'a>(
    fluent: &'a Fluent,
    settings: &'a Settings,
    applied_settings: &'a Settings,
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
        col = col.push(labeled_hint(fluent.get(Tr::LogLevelEngineHint)));
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

#[allow(clippy::too_many_arguments)]
pub(super) fn paths_section<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    settings: &'a Settings,
    applied_settings: &'a Settings,
    settings_ui: &'a SettingsUiState,
) -> Element<'a, Message> {
    let mut col = column![].spacing(SPACE_SM);

    col = col.push(paths_row(
        fluent,
        theme,
        &fluent.get(Tr::PathAria2DirLabel),
        &settings_ui.aria2_dir_picker,
        PathPickerId::CustomAria2Dir,
        settings.paths.aria2_bin_dir.is_some(),
    ));
    col = col.push(paths_row(
        fluent,
        theme,
        &fluent.get(Tr::PathAppDataDirLabel),
        &settings_ui.app_data_dir_picker,
        PathPickerId::CustomAppDataDir,
        settings.paths.app_data_dir.is_some(),
    ));
    col = col.push(paths_row(
        fluent,
        theme,
        &fluent.get(Tr::PathLogDirLabel),
        &settings_ui.log_dir_picker,
        PathPickerId::CustomLogDir,
        settings.paths.log_dir.is_some(),
    ));

    let paths_changed = settings.paths != applied_settings.paths;
    if paths_changed {
        col = col.push(
            text(fluent.get(Tr::PathRestartHint))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        );
        col = col.push(
            button(text(fluent.get(Tr::SaveAndRestartApp)).size(FONT_SMALL))
                .on_press(Message::Settings(SettingsMsg::RestartApp))
                .padding(PADDING_BUTTON_SM)
                .style(theme::style::button::primary()),
        );
    }

    col.into()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paths_row<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    label: &str,
    picker: &'a PathPicker,
    id: PathPickerId,
    override_active: bool,
) -> Element<'a, Message> {
    let picker_elem = picker.view(fluent, theme, &[], move |e| {
        Message::Add(AddMsg::PathPicker(id, e))
    });

    let restore_btn: Element<'a, Message> = if override_active {
        button(text(fluent.get(Tr::PathRestoreDefault)).size(FONT_SMALL))
            .on_press(Message::Settings(SettingsMsg::RestoreDefaultPath(id)))
            .padding(PADDING_BUTTON_SM)
            .style(theme::style::button::secondary())
            .into()
    } else {
        iced::widget::Space::new()
            .width(Length::Fixed(0.0))
            .height(Length::Fixed(0.0))
            .into()
    };

    column![]
        .push(
            row![]
                .spacing(SPACE_LG)
                .align_y(Alignment::Center)
                .push(
                    text(label.to_string())
                        .size(FONT_MEDIUM)
                        .width(Length::Fixed(200.0)),
                )
                .push(picker_elem)
                .push(restore_btn),
        )
        .into()
}
