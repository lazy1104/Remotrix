use iced::advanced::layout::{self, Node};
use iced::advanced::renderer;
use iced::advanced::widget::{self, tree, Widget};
use iced::advanced::{Clipboard, Layout, Renderer, Shell};
use iced::{mouse, Element, Event, Length, Rectangle, Size, Transformation, Vector};

use crate::ui::animation::SWAP_MIN;

/// Magnitude of the slide, in pixels, reached when `factor` is at its
/// minimum (`SWAP_MIN`). Direction (up/down) comes from the sign of
/// `index_delta` passed to [`translate_y`].
pub const SLIDE_PX: f32 = 18.0;

/// Slide-only transition wrapper: translates `content` vertically based
/// on the animated `factor` and the signed `index_delta` of the swap.
///
/// `factor = 1.0` → no offset. As `factor` drops toward `SWAP_MIN`, the
/// content shifts by up to `|index_delta| * SLIDE_PX` pixels. A positive
/// `index_delta` (`from_index - to_index > 0`, i.e. the user picked an
/// item above the current one) slides **down**; a negative delta slides
/// **up**. No scaling, no background wash.
pub fn translate_y<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    factor: f32,
    index_delta: i8,
) -> Element<'a, Message> {
    let factor = factor.clamp(0.0, 1.0);
    let dip = ((1.0 - factor) / (1.0 - SWAP_MIN)).clamp(0.0, 1.0);
    let offset_y = dip * f32::from(index_delta) * SLIDE_PX;
    Element::new(TranslateY {
        content: content.into(),
        offset_y,
    })
}

struct TranslateY<'a, Message> {
    content: Element<'a, Message>,
    offset_y: f32,
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for TranslateY<'a, Message> {
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
        if self.offset_y.abs() < 0.001 {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
            return;
        }
        let affine = Transformation::translate(0.0, self.offset_y);
        renderer.with_transformation(affine, |renderer| {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        });
    }
}

impl<'a, Message: 'a> From<TranslateY<'a, Message>> for Element<'a, Message> {
    fn from(t: TranslateY<'a, Message>) -> Self {
        Element::new(t)
    }
}
