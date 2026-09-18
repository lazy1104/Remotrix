use iced::advanced::layout::{self, Node};
use iced::advanced::renderer;
use iced::advanced::widget::{self, tree, Widget};
use iced::advanced::{Layout, Renderer};
use iced::{mouse, Element, Length, Rectangle, Size, Transformation};

use crate::ui::animation::SWAP_MIN;

pub fn scale<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    factor: f32,
) -> Element<'a, Message> {
    scale_with(content, factor, SWAP_MIN, WASH_STRENGTH)
}

pub fn scale_with<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    factor: f32,
    min_factor: f32,
    wash_strength: f32,
) -> Element<'a, Message> {
    let factor = factor.clamp(0.0, 1.0);
    let min_factor = min_factor.clamp(0.0, 1.0);
    Element::new(Scale {
        content: content.into(),
        factor,
        min_factor,
        wash_strength,
    })
}

const WASH_STRENGTH: f32 = 0.30;

struct Scale<'a, Message> {
    content: Element<'a, Message>,
    factor: f32,
    min_factor: f32,
    wash_strength: f32,
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for Scale<'a, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
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

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
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
        if self.factor >= 0.99999 {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
            return;
        }
        let bounds = layout.bounds();
        let center_x = bounds.x + bounds.width / 2.0;
        let center_y = bounds.y + bounds.height / 2.0;
        let affine = Transformation::translate(center_x, center_y)
            * Transformation::scale(self.factor)
            * Transformation::translate(-center_x, -center_y);
        let Some(clipped_viewport) = bounds.intersection(viewport) else {
            return;
        };

        let wash_t = if self.min_factor < 1.0 {
            ((self.factor - self.min_factor) / (1.0 - self.min_factor)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let wash_alpha = (1.0 - wash_t) * self.wash_strength;

        renderer.with_layer(bounds, |renderer| {
            renderer.with_transformation(affine, |renderer| {
                self.content.as_widget().draw(
                    tree,
                    renderer,
                    theme,
                    style,
                    layout,
                    cursor,
                    &clipped_viewport,
                );
            });
            if wash_alpha > 0.001 {
                let bg = theme
                    .extended_palette()
                    .background
                    .base
                    .color
                    .scale_alpha(wash_alpha);
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        ..renderer::Quad::default()
                    },
                    bg,
                );
            }
        });
    }
}

impl<'a, Message: 'a> From<Scale<'a, Message>> for Element<'a, Message> {
    fn from(scale: Scale<'a, Message>) -> Self {
        Element::new(scale)
    }
}
