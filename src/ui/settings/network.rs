use iced::widget::{column, row, text};
use iced::{Element, Length};

use super::{
    group_title, labeled_editor, labeled_hint, labeled_number, labeled_text_input, labeled_toggle,
    sub_items, CtxTarget, Fluent, Message, SettingKey, Settings, SettingsMsg, Tr, FONT_MEDIUM,
    PADDING_BUTTON_SM, SPACE_SM,
};
use crate::ui::theme;

pub(super) fn network_view<'a>(
    fluent: &'a Fluent,
    settings: &'a Settings,
    ua_editor: &'a iced::widget::text_editor::Content,
    accent: iced::Color,
) -> Element<'a, Message> {
    column![]
        .spacing(SPACE_SM)
        .push(crate::ui::components::scroll_top_gap::view())
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
                CtxTarget::SettingsUa,
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

pub(super) fn proxy_fields<'a>(fluent: &'a Fluent, settings: &'a Settings) -> Element<'a, Message> {
    if settings.aria2.proxy_enabled {
        let address = fluent.get(Tr::ProxyAddressPlaceholder);
        let username = fluent.get(Tr::ProxyUsernamePlaceholder);
        let password = fluent.get(Tr::ProxyPasswordPlaceholder);
        sub_items([
            labeled_text_input(
                fluent.get(Tr::ProxyAddress),
                &settings.aria2.proxy_server,
                SettingKey::ProxyServer,
                false,
                &address,
            ),
            labeled_hint(fluent.get(Tr::ProxyProtocolHint)),
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
        ])
    } else {
        iced::widget::Space::new().height(Length::Fixed(0.0)).into()
    }
}
