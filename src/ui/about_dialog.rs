use iced::alignment::Alignment;
use iced::widget::{column, container, row, text};
use iced::{Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{DialogMsg, Message};
use crate::ui::components::copyable_text::copyable_text;
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::components::logo;
use crate::ui::dims::*;
use crate::ui::theme;

pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    aria2_version: Option<&'a str>,
) -> Element<'a, Message> {
    let gui_text = format!("Remotrix {}", env!("CARGO_PKG_VERSION"));
    let engine_text = match aria2_version {
        Some(v) => format!("Engine: aria2-next v{v}"),
        None => "Engine: aria2-next (--)".to_string(),
    };
    let iced_text = fluent.get_args(Tr::AboutBuiltWith, &{
        let mut a = std::collections::HashMap::new();
        a.insert(std::borrow::Cow::from("version"), "0.14".into());
        a
    });

    let body = column![]
        .spacing(SPACE_4XL)
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .push(
            container(logo::view_brand(ABOUT_LOGO_SIZE, ABOUT_LOGO_SIZE))
                .center_x(Length::Fill)
                .width(Length::Fill),
        )
        .push(
            row![
                text("Re").color(theme::primary(theme)).size(FONT_TITLE),
                text("Motrix").size(FONT_TITLE),
            ]
            .spacing(0)
            .align_y(Alignment::Center),
        )
        .push(
            text(fluent.get(Tr::AboutTagline))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .push(copyable_text(gui_text.clone(), Message::CopyText(gui_text)).width(Length::Fill))
        .push(
            copyable_text(engine_text.clone(), Message::CopyText(engine_text)).width(Length::Fill),
        )
        .push(
            text(iced_text)
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .push(
            text(fluent.get(Tr::LicenseNotice))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        );

    overlay(
        Dialog::new()
            .width(380.0)
            .with_close(Message::Dialog(DialogMsg::CloseAbout))
            .body(body)
            .build(),
    )
}
