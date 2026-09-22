use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::widget::{self, tree, Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::widget::{button, container, row, text, text_input, Space, Text};
use iced::{Alignment, Background, Color, Element, Event, Length, Point, Rectangle, Size, Vector};

use crate::i18n::{Fluent, Tr};
use crate::ui::components::tooltip;
use crate::ui::components::CONTROL_HEIGHT;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

struct SecretInputState {
    focused: bool,
    hovered: bool,
}

struct SecretInput<'a, Message> {
    child: Element<'a, Message, iced::Theme, iced::Renderer>,
}

pub fn secret_input<'a, Message>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    value: &str,
    placeholder: &str,
    on_change: impl Fn(String) -> Message + 'a,
    on_generate: Message,
    on_copy: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let text_secondary = theme::text_secondary(theme);

    let input = theme::grouped_input_layout(
        text_input(placeholder, value)
            .on_input(on_change)
            .style(theme::style::input::grouped)
            .width(Length::Fixed(180.0)),
    );
    let mut row = row![]
        .spacing(SPACE_NONE)
        .align_y(Alignment::Center)
        .height(Length::Fill);
    row = row.push(input);
    row = row.push(separator());

    let copy_btn = tooltip::standard(
        button(icon_content(
            icon::copy().size(FONT_ICON).color(text_secondary),
        ))
        .on_press(on_copy)
        .style(theme::style::button::grouped_icon(
            false,
            false,
            value.is_empty(),
        ))
        .height(Length::Fill),
        text(fluent.get(Tr::Copy)),
        iced::widget::tooltip::Position::Bottom,
    );
    row = row.push(copy_btn);
    row = row.push(separator());

    let generate_btn = tooltip::standard(
        button(icon_content(
            icon::dices().size(FONT_ICON).color(text_secondary),
        ))
        .on_press(on_generate)
        .style(theme::style::button::grouped_icon(true, false, false))
        .height(Length::Fill),
        text(fluent.get(Tr::GenerateSecret)),
        iced::widget::tooltip::Position::Bottom,
    );
    row = row.push(generate_btn);

    Element::new(SecretInput { child: row.into() })
}

fn icon_content<'a, Message: 'a>(
    icon: Text<'a>,
) -> Element<'a, Message, iced::Theme, iced::Renderer> {
    container(icon.line_height(1.0))
        .center_y(Length::Fill)
        .into()
}

fn separator<'a, Message: 'a>() -> Element<'a, Message, iced::Theme, iced::Renderer> {
    container(Space::new())
        .width(Length::Fixed(1.0))
        .height(Length::Fill)
        .style(theme::style::separator)
        .into()
}

struct FocusProbe {
    focused: bool,
}

impl widget::Operation for FocusProbe {
    fn focusable(
        &mut self,
        _id: Option<&iced::widget::Id>,
        _bounds: Rectangle,
        state: &mut dyn iced::advanced::widget::operation::Focusable,
    ) {
        if state.is_focused() {
            self.focused = true;
        }
    }

    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for SecretInput<'a, Message>
where
    Message: 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SecretInputState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SecretInputState {
            focused: false,
            hovered: false,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.child));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Fixed(CONTROL_HEIGHT))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let padding = iced::Padding::new(PADDING_GROUPED);
        let limits = limits
            .width(Length::Shrink)
            .height(Length::Fixed(CONTROL_HEIGHT));
        let content = self.child.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &limits.shrink(padding),
        );
        let inner = limits.shrink(padding).resolve(
            Length::Shrink,
            Length::Fixed(CONTROL_HEIGHT),
            content.size(),
        );
        let outer = Size::new(
            inner.width + padding.left + padding.right,
            inner.height + padding.top + padding.bottom,
        );
        layout::Node::with_children(
            outer,
            vec![content.move_to(Point::new(padding.left, padding.top))],
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
        use iced::advanced::Renderer as _;

        let state = tree.state.downcast_ref::<SecretInputState>();
        let hovered = cursor.is_over(layout.bounds());
        let frame_style = theme::style::grouped_frame_state(state.focused, hovered);
        let s = frame_style(theme);
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border: s.border,
                ..Default::default()
            },
            s.background
                .unwrap_or(Background::Color(Color::TRANSPARENT)),
        );

        if let Some(child_layout) = layout.children().next() {
            self.child.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                viewport,
            );
        }
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
        let state = tree.state.downcast_mut::<SecretInputState>();
        state.hovered = cursor.is_over(layout.bounds());

        if let Some(child_layout) = layout.children().next() {
            self.child.as_widget_mut().update(
                &mut tree.children[0],
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }

        let mut probe = FocusProbe { focused: false };
        if let Some(child_layout) = layout.children().next() {
            self.child.as_widget_mut().operate(
                &mut tree.children[0],
                child_layout,
                renderer,
                &mut probe,
            );
        }
        state.focused = probe.focused;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.traverse(&mut |operation| {
            if let Some(child_layout) = layout.children().next() {
                self.child.as_widget_mut().operate(
                    &mut tree.children[0],
                    child_layout,
                    renderer,
                    operation,
                );
            }
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::None;
        }
        if let Some(child_layout) = layout.children().next() {
            self.child.as_widget().mouse_interaction(
                &tree.children[0],
                child_layout,
                cursor,
                viewport,
                renderer,
            )
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
    ) -> Option<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        if let Some(child_layout) = layout.children().next() {
            self.child.as_widget_mut().overlay(
                &mut tree.children[0],
                child_layout,
                renderer,
                viewport,
                translation,
            )
        } else {
            None
        }
    }
}
