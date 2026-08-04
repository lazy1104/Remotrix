use iced::alignment::Alignment;
use iced::widget::{button, row, text};
use iced::{Element, Length};

use crate::ui::components::truncated_text::truncated_text;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

pub fn copyable_text<Message>(text: impl Into<String>, on_copy: Message) -> CopyableText<Message> {
    CopyableText {
        text: text.into(),
        width: Length::Shrink,
        on_copy,
    }
}

pub struct CopyableText<Message> {
    text: String,
    width: Length,
    on_copy: Message,
}

impl<Message> CopyableText<Message> {
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }
}

impl<'a, Message: Clone + 'a> From<CopyableText<Message>> for Element<'a, Message> {
    fn from(c: CopyableText<Message>) -> Self {
        let CopyableText {
            text: content,
            width,
            on_copy,
        } = c;
        let label: Element<'a, Message> = if matches!(width, Length::Shrink) {
            text(content).size(FONT_MEDIUM).into()
        } else {
            truncated_text(content)
                .size(FONT_MEDIUM)
                .max_lines(1)
                .width(Length::Fill)
                .into()
        };
        let content = row![label, icon::copy().size(FONT_SMALL)]
            .spacing(SPACE_MD)
            .align_y(Alignment::Center);
        button(content)
            .on_press(on_copy)
            .padding(PADDING_BUTTON_SM)
            .width(width)
            .style(theme::style::button::copyable())
            .into()
    }
}
