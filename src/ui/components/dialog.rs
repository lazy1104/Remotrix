use iced::advanced::{
    layout::{Limits, Node},
    mouse, overlay, renderer,
    widget::{Operation, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Element, Event, Length, Rectangle, Size, Vector};

use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

struct BlockingOverlay<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: renderer::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> BlockingOverlay<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for BlockingOverlay<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let limits = limits.width(Length::Fill).height(Length::Fill);
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, &limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content]);
    }

    fn operate<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if shell.is_event_captured() {
            return;
        }

        match event {
            Event::Mouse(_) | Event::Touch(_) if cursor.is_over(layout.bounds()) => {
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            let interaction = self.content.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            );

            if interaction != mouse::Interaction::None {
                interaction
            } else {
                mouse::Interaction::Idle
            }
        } else {
            mouse::Interaction::None
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

pub fn overlay<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(BlockingOverlay::new(
        container(content)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::style::overlay),
    ))
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
            .padding(PADDING_DIALOG)
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
            bar = bar.push(text(title).size(FONT_DIALOG_TITLE));
        }
        if let Some(close) = close {
            bar = bar.push(Space::new().width(Length::Fill)).push(
                button(icon::x().size(FONT_HERO).line_height(1.0))
                    .on_press(close)
                    .padding(PADDING_DROPDOWN)
                    .style(theme::style::button::sidebar_icon(false)),
            );
        }
        Some(bar.into())
    }
}
