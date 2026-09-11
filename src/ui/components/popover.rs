//! Anchored modal popover: fills the viewport, places a card at a chosen
//! anchor via `translate`, and treats the area outside the card as a
//! click-to-close backdrop that blocks input from reaching layers below.

use iced::advanced::{
    layout::{Limits, Node},
    mouse, overlay, renderer,
    widget::{Operation, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::{touch, Element, Event, Length, Point, Rectangle, Size, Vector};

pub struct Popover<'a, Message> {
    content: Element<'a, Message>,
    anchor: Box<dyn Fn(Rectangle, Rectangle) -> Vector + 'a>,
    on_outside: Option<Message>,
}

pub fn popover<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    anchor: impl Fn(Rectangle, Rectangle) -> Vector + 'a,
    on_outside: Option<Message>,
) -> Element<'a, Message> {
    Element::new(Popover {
        content: content.into(),
        anchor: Box::new(anchor),
        on_outside,
    })
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for Popover<'a, Message>
where
    Message: 'a + Clone,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let child =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &limits.loose());
        let child_bounds = child.bounds();
        let viewport_size = limits.max();
        let viewport = Rectangle {
            x: 0.0,
            y: 0.0,
            width: viewport_size.width,
            height: viewport_size.height,
        };
        let translation = (self.anchor)(child_bounds, viewport);
        Node::with_children(
            viewport_size,
            vec![child.move_to(Point::new(translation.x, translation.y))],
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let child_layout = layout
            .children()
            .next()
            .expect("popover: content layout has no children");
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            child_layout,
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
        renderer: &iced::Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        let child_layout = layout
            .children()
            .next()
            .expect("popover: content layout has no children");
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            child_layout,
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let child_layout = layout
            .children()
            .next()
            .expect("popover: content layout has no children");
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            child_layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if shell.is_event_captured() {
            return;
        }

        if !cursor.is_over(layout.bounds()) {
            return;
        }

        let is_press = matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(_))
                | Event::Touch(touch::Event::FingerPressed { .. })
        );
        let is_pointer = matches!(event, Event::Mouse(_) | Event::Touch(_));

        if is_pointer {
            shell.capture_event();
        }

        if is_press {
            if let Some(msg) = self.on_outside.clone() {
                let card_bounds = layout
                    .children()
                    .next()
                    .map(|c| c.bounds())
                    .unwrap_or(Rectangle::INFINITE);
                if !cursor.is_over(card_bounds) {
                    shell.publish(msg);
                }
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            let child_layout = layout
                .children()
                .next()
                .expect("popover: content layout has no children");
            let interaction = self.content.as_widget().mouse_interaction(
                &tree.children[0],
                child_layout,
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
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let child_layout = layout
            .children()
            .next()
            .expect("popover: content layout has no children");
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            child_layout,
            renderer,
            viewport,
            translation,
        )
    }
}
