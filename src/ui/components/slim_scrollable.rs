use iced::widget::container;
use iced::widget::scrollable::{self, Scrollbar};
use iced::{Element, Length};

use crate::ui::theme;

pub fn slim_scrollable<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> iced::widget::Scrollable<'a, Message> {
    iced::widget::scrollable(
        container(content)
            .width(Length::Fill)
            .padding(iced::padding::bottom(5.0)),
    )
    .direction(scrollable::Direction::Vertical(
        Scrollbar::new().width(6.0).scroller_width(6.0),
    ))
    .spacing(3)
    .style(theme::style::scrollable::standard)
}
