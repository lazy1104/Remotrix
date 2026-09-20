use iced::advanced::layout::{self, Node};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::widget::{self, tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell};
use iced::{mouse, Background, Element, Event, Length, Rectangle, Size, Transformation, Vector};

const SLIDE_PX: f32 = 14.0;
const FADE_ALPHA: f32 = 0.40;

pub fn swap_translate_fade<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    factor: f32,
) -> Element<'a, Message> {
    let factor = factor.clamp(0.0, 1.0);
    Element::new(Swap {
        content: content.into(),
        factor,
    })
}

struct Swap<'a, Message> {
    content: Element<'a, Message>,
    factor: f32,
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for Swap<'a, Message> {
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
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation<()>,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
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
        use iced::advanced::Renderer as _;

        let bounds = layout.bounds();
        let Some(clipped_viewport) = bounds.intersection(viewport) else {
            return;
        };

        if self.factor >= 0.99999 {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
            return;
        }

        let dip = 1.0 - self.factor;
        let offset_y = dip * SLIDE_PX;
        let wash_alpha = dip * FADE_ALPHA;
        let bg = theme.extended_palette().background.base.color;

        renderer.with_layer(bounds, |renderer| {
            renderer.with_transformation(Transformation::translate(0.0, offset_y), |renderer| {
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
        });

        renderer.fill_quad(
            Quad {
                bounds,
                ..Default::default()
            },
            Background::Color(bg.scale_alpha(wash_alpha)),
        );
    }
}

impl<'a, Message: 'a> From<Swap<'a, Message>> for Element<'a, Message> {
    fn from(s: Swap<'a, Message>) -> Self {
        Element::new(s)
    }
}
