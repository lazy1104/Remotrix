use iced::widget::tooltip;
use iced::Element;

use crate::ui::theme;

pub fn standard<'a, Message>(
    content: impl Into<Element<'a, Message, iced::Theme, iced::Renderer>>,
    label: impl Into<Element<'a, Message, iced::Theme, iced::Renderer>>,
    position: tooltip::Position,
) -> Element<'a, Message, iced::Theme, iced::Renderer>
where
    Message: 'a,
{
    iced::widget::tooltip(content, label, position)
        .style(theme::style::tooltip)
        .into()
}
