use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, SettingsMsg};
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateComponent {
    App,
    Aria2,
}

#[derive(Debug, Clone)]
pub struct UpdateOffer {
    pub component: UpdateComponent,
    pub current: String,
    pub latest: String,
    pub changelog: String,
    pub download_url: String,
    pub sha256: Option<String>,
    pub asset_name: String,
}

fn component_label(fluent: &Fluent, c: UpdateComponent) -> String {
    match c {
        UpdateComponent::App => fluent.get(Tr::ComponentApp),
        UpdateComponent::Aria2 => fluent.get(Tr::ComponentEngine),
    }
}

fn tab_button<'a>(label: String, active: bool, on_press: Message) -> Element<'a, Message> {
    let btn = button(row![text(label).size(FONT_BODY),])
        .on_press(on_press)
        .padding(PADDING_TAB)
        .style(theme::style::button::text());
    if active {
        container(btn).style(theme::style::active_filter).into()
    } else {
        btn.into()
    }
}

pub fn view<'a>(
    fluent: &'a Fluent,
    _theme: &iced::Theme,
    offers: &[UpdateOffer],
    active_tab: usize,
) -> Element<'a, Message> {
    let active_tab = active_tab.min(offers.len().saturating_sub(1));
    let offer = &offers[active_tab];

    let mut body = column![].spacing(SPACE_3XL).width(Length::Fill);

    if offers.len() > 1 {
        let mut tabs = row![].spacing(SPACE_SM).width(Length::Fill);
        for (i, o) in offers.iter().enumerate() {
            tabs = tabs.push(tab_button(
                component_label(fluent, o.component),
                i == active_tab,
                Message::Settings(SettingsMsg::UpdateDialogTab(i)),
            ));
        }
        body = body.push(tabs);
    }

    let transition = fluent.get_args(Tr::UpdateDialogVersionTransition, &{
        let mut a = std::collections::HashMap::new();
        a.insert(
            std::borrow::Cow::from("component"),
            component_label(fluent, offer.component).into(),
        );
        a.insert(std::borrow::Cow::from("from"), offer.current.clone().into());
        a.insert(std::borrow::Cow::from("to"), offer.latest.clone().into());
        a
    });
    body = body.push(
        container(text(transition).size(FONT_BODY))
            .width(Length::Fill)
            .padding(PADDING_CARD)
            .style(theme::style::card),
    );

    body = body.push(text(fluent.get(Tr::UpdateDialogChangelog)).size(FONT_MEDIUM));
    let changelog: Element<'a, Message> = if offer.changelog.trim().is_empty() {
        column![text(fluent.get(Tr::UpdateChangelogEmpty))
            .size(FONT_SMALL)
            .style(theme::style::text::secondary)]
        .into()
    } else {
        column![text(offer.changelog.clone()).size(FONT_SMALL)]
            .width(Length::Fill)
            .into()
    };
    body = body.push(
        container(
            slim_scrollable(changelog)
                .height(Length::Fixed(220.0))
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .padding(PADDING_CARD)
        .style(theme::style::card),
    );

    let footer = row![
        button(text(fluent.get(Tr::Cancel)).size(FONT_BODY))
            .on_press(Message::Settings(SettingsMsg::UpdateDialogCancel))
            .padding(PADDING_BUTTON_XL)
            .style(theme::style::button::secondary()),
        button(
            row![
                icon::download().size(FONT_ICON),
                text(fluent.get(Tr::UpdateNow)).size(FONT_BODY),
            ]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center),
        )
        .on_press(Message::Settings(SettingsMsg::UpdateDialogApply))
        .padding(PADDING_BUTTON_XL)
        .style(theme::style::button::primary()),
    ]
    .spacing(SPACE_2XL)
    .align_y(Alignment::Center);

    overlay(
        Dialog::new()
            .width(460.0)
            .title(fluent.get(Tr::UpdateDialogTitle))
            .with_close(Message::Settings(SettingsMsg::UpdateDialogCancel))
            .body(body)
            .footer(footer)
            .build(),
    )
}
