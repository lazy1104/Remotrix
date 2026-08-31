use iced::widget::container;
use iced::widget::scrollable::{self, Scrollbar, Viewport};
use iced::{Element, Length};

use crate::ui::dims::*;
use crate::ui::theme;

pub fn slim_scrollable<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    id: iced::widget::Id,
    on_scroll: impl Fn(Viewport) -> Message + 'a,
) -> iced::widget::Scrollable<'a, Message> {
    iced::widget::scrollable(
        container(content)
            .width(Length::Fill)
            .padding(iced::padding::bottom(5.0)),
    )
    .id(id.clone())
    .direction(scrollable::Direction::Vertical(
        Scrollbar::new().width(6.0).scroller_width(6.0),
    ))
    .spacing(SPACE_SCROLL)
    .on_scroll(on_scroll)
    .style(theme::style::scrollable::animating(id))
}
