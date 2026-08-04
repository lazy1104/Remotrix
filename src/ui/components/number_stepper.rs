use std::rc::Rc;

use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::renderer;
use iced::advanced::widget::{self, tree, Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::widget::{button, container, row, text_input, Space};
use iced::{Alignment, Background, Color, Element, Event, Length, Point, Rectangle, Size, Vector};

use super::CONTROL_HEIGHT;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

fn separator<'a, Message: 'a>() -> Element<'a, Message, iced::Theme, iced::Renderer> {
    container(Space::new())
        .width(Length::Fixed(1.0))
        .height(Length::Fill)
        .style(theme::style::separator)
        .into()
}

fn icon_content<'a, Message: 'a>(
    icon: iced::widget::Text<'a>,
) -> Element<'a, Message, iced::Theme, iced::Renderer> {
    container(icon.line_height(1.0))
        .center_y(Length::Fill)
        .into()
}

fn clamp_value<T: PartialOrd>(v: T, min: T, max: T) -> T {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

struct FocusInput;

impl widget::Operation for FocusInput {
    fn focusable(
        &mut self,
        _id: Option<&iced::widget::Id>,
        _bounds: Rectangle,
        state: &mut dyn iced::advanced::widget::operation::Focusable,
    ) {
        if !state.is_focused() {
            state.focus();
        }
    }

    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn widget::Operation)) {
        operate(self);
    }
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

    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn widget::Operation)) {
        operate(self);
    }
}

struct BufferReader {
    text: String,
}

impl widget::Operation for BufferReader {
    fn text_input(
        &mut self,
        _id: Option<&iced::widget::Id>,
        _bounds: Rectangle,
        state: &mut dyn iced::advanced::widget::operation::TextInput,
    ) {
        self.text = state.text().to_string();
    }

    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn widget::Operation)) {
        operate(self);
    }
}

struct StepperState {
    buffer: String,
    focused: bool,
    hovered: bool,
}

pub struct NumberStepper<'a, T, Message: 'a> {
    value: T,
    min: T,
    max: T,
    on_change: Rc<dyn Fn(T) -> Message + 'a>,
    read_only: bool,
    width: Length,
    child: Element<'a, Message, iced::Theme, iced::Renderer>,
}

fn build_row<'a, T, Message: Clone + 'a>(
    value: T,
    min: T,
    max: T,
    step: T,
    on_change: &Rc<dyn Fn(T) -> Message + 'a>,
    read_only: bool,
) -> Element<'a, Message, iced::Theme, iced::Renderer>
where
    T: num_traits::Num
        + PartialOrd
        + std::fmt::Display
        + std::str::FromStr
        + Clone
        + Copy
        + 'static,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    let mut input = theme::grouped_input_layout(
        text_input("", &value.to_string())
            .style(theme::style::input::grouped)
            .width(Length::Fill),
    );

    if !read_only {
        let oc = on_change.clone();
        let v = value;
        input = input.on_input(move |s| {
            let current = v;
            let parsed = s.parse::<T>().ok();
            let clamped = clamp_value(parsed.unwrap_or(current), min, max);
            (oc)(clamped)
        });
    }

    let mut r = row![]
        .spacing(SPACE_NONE)
        .align_y(Alignment::Center)
        .height(Length::Fill)
        .push(input)
        .push(separator());

    let minus_btn = button(icon_content::<Message>(icon::minus().size(FONT_ICON)))
        .style(theme::style::button::grouped_icon(false))
        .height(Length::Fill);
    if !read_only {
        let minus_val = if value >= min + step {
            value - step
        } else {
            min
        };
        let clamped = clamp_value(minus_val, min, max);
        r = r.push(minus_btn.on_press((on_change)(clamped)));
    } else {
        r = r.push(minus_btn);
    }

    r = r.push(separator());

    let plus_btn = button(icon_content::<Message>(icon::plus().size(FONT_ICON)))
        .style(theme::style::button::grouped_icon(true))
        .height(Length::Fill);
    if !read_only {
        let plus_val = if value <= max - step {
            value + step
        } else {
            max
        };
        let clamped = clamp_value(plus_val, min, max);
        r = r.push(plus_btn.on_press((on_change)(clamped)));
    } else {
        r = r.push(plus_btn);
    }

    r.into()
}

fn flatten_bounds<T>(bounds: impl std::ops::RangeBounds<T>) -> (T, T)
where
    T: num_traits::Bounded + Clone,
{
    use std::ops::Bound;
    let min = match bounds.start_bound() {
        Bound::Included(v) => v.clone(),
        Bound::Excluded(_) => T::min_value(),
        Bound::Unbounded => T::min_value(),
    };
    let max = match bounds.end_bound() {
        Bound::Included(v) => v.clone(),
        Bound::Excluded(_) => T::max_value(),
        Bound::Unbounded => T::max_value(),
    };
    (min, max)
}

pub fn number_stepper<'a, T, Message>(
    value: T,
    bounds: impl std::ops::RangeBounds<T>,
    step: T,
    on_change: impl Fn(T) -> Message + 'a,
    width: Length,
) -> Element<'a, Message, iced::Theme, iced::Renderer>
where
    T: num_traits::Num
        + num_traits::NumAssignOps
        + PartialOrd
        + std::fmt::Display
        + std::str::FromStr
        + Clone
        + Copy
        + num_traits::Bounded
        + 'static,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
    Message: 'a + Clone,
{
    let (min, max) = flatten_bounds(bounds);
    let on_change: Rc<dyn Fn(T) -> Message + 'a> = Rc::new(on_change);
    let child = build_row(value, min, max, step, &on_change, false);
    Element::new(NumberStepper {
        value,
        min,
        max,
        on_change,
        read_only: false,
        width,
        child,
    })
}

impl<'a, T, Message> Widget<Message, iced::Theme, iced::Renderer> for NumberStepper<'a, T, Message>
where
    T: num_traits::Num
        + num_traits::NumAssignOps
        + PartialOrd
        + std::fmt::Display
        + std::str::FromStr
        + Clone
        + Copy
        + 'static,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
    Message: 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<StepperState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(StepperState {
            buffer: self.value.to_string(),
            focused: false,
            hovered: false,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.child)]
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<StepperState>();
        if !state.focused {
            state.buffer = self.value.to_string();
            tree.diff_children(std::slice::from_ref(&self.child));
        } else if tree.children.is_empty() {
            tree.diff_children(std::slice::from_ref(&self.child));
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Fixed(CONTROL_HEIGHT))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits
            .width(self.width)
            .height(Length::Fixed(CONTROL_HEIGHT));
        let content = self.child.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &limits.shrink(iced::Padding::new(PADDING_GROUPED)),
        );
        let size = limits.resolve(self.width, Length::Fixed(CONTROL_HEIGHT), content.size());
        layout::Node::with_children(
            size,
            vec![content.move_to(Point::new(PADDING_GROUPED, PADDING_GROUPED))],
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

        let state = tree.state.downcast_ref::<StepperState>();
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
        let state = tree.state.downcast_mut::<StepperState>();
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

        let mut reader = BufferReader {
            text: String::new(),
        };
        if let Some(child_layout) = layout.children().next() {
            self.child.as_widget_mut().operate(
                &mut tree.children[0],
                child_layout,
                renderer,
                &mut reader,
            );
        }
        state.buffer = reader.text;

        let mut probe = FocusProbe { focused: false };
        if let Some(child_layout) = layout.children().next() {
            self.child.as_widget_mut().operate(
                &mut tree.children[0],
                child_layout,
                renderer,
                &mut probe,
            );
        }

        let refocus = !self.read_only
            && matches!(
                event,
                Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
            )
            && cursor.is_over(layout.bounds());

        if refocus {
            let mut op = FocusInput;
            if let Some(child_layout) = layout.children().next() {
                self.child.as_widget_mut().operate(
                    &mut tree.children[0],
                    child_layout,
                    renderer,
                    &mut op,
                );
            }
        }

        if !state.focused && probe.focused {
            state.buffer = self.value.to_string();
        } else if state.focused && !probe.focused && !refocus {
            let parsed = state
                .buffer
                .parse::<T>()
                .ok()
                .filter(|v| v >= &self.min && v <= &self.max);
            let clamped = parsed.unwrap_or(self.value);
            state.buffer = clamped.to_string();
            shell.publish((self.on_change)(clamped));
        }
        state.focused = if refocus { true } else { probe.focused };
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
