use iced::advanced::layout::{self, Node};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget};
use iced::advanced::Layout;
use iced::{mouse, Element, Length, Rectangle, Size, Vector};

pub fn translate<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    x: f32,
    y: f32,
) -> Element<'a, Message> {
    Element::new(Translate {
        content: content.into(),
        offset: Vector::new(x, y),
    })
}

struct Translate<'a, Message> {
    content: Element<'a, Message>,
    offset: Vector,
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for Translate<'a, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        let node = self.content.as_widget_mut().layout(tree, renderer, limits);
        node.translate(self.offset)
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }
}

impl<'a, Message: 'a> From<Translate<'a, Message>> for Element<'a, Message> {
    fn from(translate: Translate<'a, Message>) -> Self {
        Element::new(translate)
    }
}
