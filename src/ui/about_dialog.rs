use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::Message;
use crate::ui::theme;

pub fn view<'a>(fluent: &'a Fluent, dark: bool) -> Element<'a, Message> {
    let text_primary = if dark {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_PRIMARY_LIGHT
    };
    let text_secondary = if dark {
        theme::TEXT_SECONDARY
    } else {
        theme::TEXT_SECONDARY_LIGHT
    };
    let card_bg = if dark {
        theme::BG_CARD
    } else {
        theme::BG_CARD_LIGHT
    };

    let panel = container(
        column![]
            .spacing(16)
            .push(
                text(fluent.get(Tr::AboutTitle))
                    .size(20)
                    .color(text_primary),
            )
            .push(text("Remotrix 0.1.0").size(14).color(text_secondary))
            .push(
                text("Engine: aria2-core 0.2.3")
                    .size(13)
                    .color(text_secondary),
            )
            .push(text("GUI: iced 0.13").size(13).color(text_secondary))
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
    .style(move |_theme| container::Style {
        background: Some(card_bg.into()),
        border: iced::border::rounded(12),
        ..Default::default()
    });

    container(panel)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(theme::OVERLAY.into()),
            ..Default::default()
        })
        .into()
}
