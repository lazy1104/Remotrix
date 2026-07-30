use iced::widget::scrollable::{self, Anchor, Scrollbar};
use iced::Element;

use crate::ui::theme;

pub fn slim_scrollable<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> iced::widget::Scrollable<'a, Message> {
    iced::widget::scrollable(content)
        .direction(scrollable::Direction::Vertical(
            Scrollbar::new()
                .width(6.0)
                .scroller_width(6.0)
        ))
        .spacing(5)
        .style(theme::style::scrollable::standard)
}
