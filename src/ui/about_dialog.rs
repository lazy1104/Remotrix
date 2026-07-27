use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::Message;
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

    let panel = container(
        column![]
            .spacing(16)
            .push(text(fluent.get(Tr::AboutTitle)).size(20))
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
            )
            .push(iced::widget::Space::new().height(Length::Fixed(8.0)))
            .push(
                row![]
                    .push(iced::widget::Space::new().width(Length::Fill))
                    .push(
                        button(text(fluent.get(Tr::CloseAbout)).size(14))
                            .on_press(Message::CloseAbout)
                            .padding([10, 22])
                            .style(button::secondary),
                    )
                    .width(Length::Fill),
            ),
    )
    .width(Length::Fixed(380.0))
    .padding(28)
    .style(theme::style::card);

    container(panel)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::style::overlay)
        .into()
}
