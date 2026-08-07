use iced::advanced::layout::{Limits, Node};
use iced::advanced::renderer;
use iced::advanced::widget::{self, tree, Widget};
use iced::advanced::{mouse, Clipboard, Layout, Renderer, Shell};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

pub struct Expand<'a, Message> {
    content: Element<'a, Message>,
    progress: f32,
    pinned: bool,
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for Expand<'a, Message> {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<widget::Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &iced::Renderer,
        limits: &Limits,
    ) -> Node {
        let node = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &limits.loose());
        if self.pinned {
            return node;
        }
        let t = self.progress.clamp(0.0, 1.0);
        if t >= 1.0 {
            return node;
        }
        Node::with_children(
            Size::new(node.size().width, node.size().height * t),
            vec![node],
        )
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
        let t = self.progress.clamp(0.0, 1.0);
        if t >= 1.0 {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        } else {
            let bounds = layout.bounds();
            let layer = if self.pinned {
                self.visible_rect(bounds)
            } else {
                bounds
            };
            let Some(clipped_viewport) = layer.intersection(viewport) else {
                return;
            };
            renderer.with_layer(layer, |renderer| {
                let child = if self.pinned {
                    layout
                } else {
                    layout.children().next().unwrap()
                };
                self.content.as_widget().draw(
                    tree,
                    renderer,
                    theme,
                    style,
                    child,
                    cursor,
                    &clipped_viewport,
                );
            });
        }
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.pinned && self.progress < 1.0 {
            if let Event::Mouse(_) | Event::Touch(_) = event {
                let bounds = layout.bounds();
                if !cursor.is_over(self.visible_rect(bounds)) {
                    return;
                }
            }
        }
        let child = if self.pinned || self.progress >= 1.0 {
            layout
        } else {
            layout.children().next().unwrap()
        };
        self.content.as_widget_mut().update(
            tree, event, child, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let child = if self.pinned || self.progress >= 1.0 {
            layout
        } else {
            layout.children().next().unwrap()
        };
        self.content
            .as_widget_mut()
            .operate(tree, child, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if self.pinned && self.progress < 1.0 {
            let bounds = layout.bounds();
            if !cursor.is_over(self.visible_rect(bounds)) {
                return mouse::Interaction::None;
            }
        }
        let child = if self.pinned || self.progress >= 1.0 {
            layout
        } else {
            layout.children().next().unwrap()
        };
        self.content
            .as_widget()
            .mouse_interaction(tree, child, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let child = if self.pinned || self.progress >= 1.0 {
            layout
        } else {
            layout.children().next().unwrap()
        };
        self.content
            .as_widget_mut()
            .overlay(tree, child, renderer, viewport, translation)
    }
}

impl<'a, Message: 'a> From<Expand<'a, Message>> for Element<'a, Message> {
    fn from(expand: Expand<'a, Message>) -> Self {
        Element::new(expand)
    }
}

impl<'a, Message> Expand<'a, Message> {
    fn visible_rect(&self, bounds: Rectangle) -> Rectangle {
        let t = self.progress.clamp(0.0, 1.0);
        if self.pinned && t < 1.0 {
            let h = bounds.height * t;
            Rectangle {
                x: bounds.x,
                y: bounds.y + (bounds.height - h) / 2.0,
                width: bounds.width,
                height: h,
            }
        } else {
            bounds
        }
    }
}

pub fn expand<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    progress: f32,
) -> Element<'a, Message> {
    Expand {
        content: content.into(),
        progress,
        pinned: false,
    }
    .into()
}

pub fn expand_pinned<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    progress: f32,
) -> Element<'a, Message> {
    Expand {
        content: content.into(),
        progress,
        pinned: true,
    }
    .into()
}
