use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Element, Length};

use crate::ui::icon;
use crate::ui::theme;

pub fn overlay<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::style::overlay)
        .into()
}

pub struct Dialog<'a, Message> {
    width: f32,
    spacing: f32,
    title: Option<String>,
    close: Option<Message>,
    body: Option<Element<'a, Message>>,
    footer: Option<Element<'a, Message>>,
}

impl<'a, Message: Clone + 'a> Dialog<'a, Message> {
    pub fn new() -> Self {
        Self {
            width: 420.0,
            spacing: 16.0,
            title: None,
            close: None,
            body: None,
            footer: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn with_close(mut self, message: Message) -> Self {
        self.close = Some(message);
        self
    }

    pub fn body(mut self, body: impl Into<Element<'a, Message>>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn footer(mut self, footer: impl Into<Element<'a, Message>>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn build(self) -> Element<'a, Message> {
        let mut inner = column![].spacing(self.spacing);

        if let Some(header) = self.header() {
            inner = inner.push(header);
        }
        if let Some(body) = self.body {
            inner = inner.push(body);
        }
        if let Some(footer) = self.footer {
            inner = inner.push(
                row![Space::new().width(Length::Fill), footer]
                    .align_y(Alignment::Center)
                    .width(Length::Fill),
            );
        }

        container(inner)
            .width(Length::Fixed(self.width))
            .padding(28)
            .style(theme::style::card)
            .into()
    }

    fn header(&self) -> Option<Element<'a, Message>> {
        let title = self.title.clone();
        let close = self.close.clone();
        if title.is_none() && close.is_none() {
            return None;
        }

        let mut bar = row![].align_y(Alignment::Center);
        if let Some(title) = title {
            bar = bar.push(text(title).size(20));
        }
        if let Some(close) = close {
            bar = bar.push(Space::new().width(Length::Fill)).push(
                button(icon::x().size(18).line_height(1.0))
                    .on_press(close)
                    .padding(6)
                    .style(theme::style::button::sidebar_icon(false)),
            );
        }
        Some(bar.into())
    }
}
