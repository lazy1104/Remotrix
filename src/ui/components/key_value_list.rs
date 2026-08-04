use iced::widget::text::Wrapping;
use iced::widget::{column, container, row, text, Space};
use iced::{Alignment, Element, Length};

use crate::ui::components::truncated_text::truncated_text;
use crate::ui::dims::*;
use crate::ui::theme;

pub fn key_value_row<'a, Message: 'a>(
    key: impl Into<String>,
    value: impl Into<String>,
) -> Element<'a, Message> {
    row![]
        .push(
            text(key.into())
                .size(FONT_MEDIUM)
                .style(theme::style::text::secondary)
                .width(Length::Fixed(140.0)),
        )
        .push(
            truncated_text(value.into())
                .size(FONT_MEDIUM)
                .max_lines(2)
                .wrapping(Wrapping::Glyph),
        )
        .spacing(SPACE_LG)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

pub fn key_value_list<'a, Message: 'a, S, T>(
    rows: impl IntoIterator<Item = (S, T)>,
) -> Element<'a, Message>
where
    S: Into<String>,
    T: Into<String>,
{
    let mut list = column![].width(Length::Fill).spacing(SPACE_MD);
    let mut first = true;
    for (key, value) in rows {
        if !first {
            list = list.push(
                container(Space::new())
                    .height(Length::Fixed(1.0))
                    .width(Length::Fill)
                    .style(theme::style::separator),
            );
        }
        list = list.push(key_value_row(key.into(), value.into()));
        first = false;
    }

    container(list)
        .padding(PADDING_CARD)
        .width(Length::Fill)
        .style(theme::style::tree_frame)
        .into()
}
