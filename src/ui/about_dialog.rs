use iced::widget::{button, column, text};
use iced::Element;

use crate::i18n::{Fluent, Tr};
use crate::message::Message;
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::theme;

pub fn view<'a>(
    fluent: &'a Fluent,
    _theme: &iced::Theme,
    aria2_version: Option<&'a str>,
) -> Element<'a, Message> {
    let engine_text = match aria2_version {
        Some(v) => format!("Engine: aria2-next v{v}"),
        None => "Engine: aria2-next (--)".to_string(),
    };

    let body = column![]
        .spacing(16)
        .push(
            text(format!("Remotrix {}", env!("CARGO_PKG_VERSION")))
                .size(14)
                .style(theme::style::text::secondary),
        )
        .push(
            text(engine_text)
                .size(13)
                .style(theme::style::text::secondary),
        )
        .push(
            text("GUI: iced 0.14")
                .size(13)
                .style(theme::style::text::secondary),
        );

    let close_btn = button(text(fluent.get(Tr::CloseAbout)).size(14))
        .on_press(Message::CloseAbout)
        .padding([10, 22])
        .style(theme::style::button::secondary());

    overlay(
        Dialog::new()
            .width(380.0)
            .title(fluent.get(Tr::AboutTitle))
            .with_close(Message::CloseAbout)
            .body(body)
            .footer(close_btn)
            .build(),
    )
}
