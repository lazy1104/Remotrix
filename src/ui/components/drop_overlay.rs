use iced::widget::{column, container, text};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::Message;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

pub fn view<'a>(fluent: &'a Fluent, _theme: &'a iced::Theme) -> Element<'a, Message> {
    let hint = text(fluent.get(Tr::DropFilesHint)).size(FONT_MEDIUM);
    let content = column![icon::arrow_up().size(FONT_HERO), hint]
        .spacing(SPACE_LG)
        .align_x(Alignment::Center);
    container(
        container(content)
            .padding(PADDING_CARD)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(theme::style::drop_zone(true)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(theme::style::drop_overlay)
    .into()
}
