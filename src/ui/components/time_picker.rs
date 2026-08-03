use iced::advanced::layout::{Limits, Node};
use iced::advanced::widget::{tree, Operation, Tree, Widget};
use iced::advanced::{mouse, overlay, renderer, Clipboard, Layout, Shell};
use iced::mouse::Cursor;
use iced::widget::{button, row};
use iced::{Alignment, Element, Event, Length, Rectangle, Size, Vector};

use iced_aw::time_picker::{Period, State as PickerState, Time, TimePicker as AwTimePicker};

use super::CONTROL_HEIGHT;
use crate::scheduler::parse_hhmm;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

pub(crate) fn picker_button<'a>() -> impl Fn(&iced::Theme, button::Status) -> button::Style + 'a {
    move |t: &iced::Theme, status: button::Status| button::Style {
        background: Some(t.extended_palette().background.base.color.into()),
        text_color: t.extended_palette().background.base.text,
        border: iced::Border {
            color: match status {
                button::Status::Hovered | button::Status::Pressed => {
                    t.extended_palette().primary.base.color
                }
                _ => theme::border_color(t),
            },
            width: 1.0,
            radius: theme::RADIUS_BUTTON.into(),
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

pub fn time_picker<'a, M>(
    value: &'a str,
    open: bool,
    on_toggle: M,
    on_change: impl Fn(String) -> M + 'static,
) -> Element<'a, M, iced::Theme, iced::Renderer>
where
    M: 'static + Clone,
{
    let underlay: Element<'a, M> = button(
        row![
            iced::widget::text(value).size(FONT_MEDIUM),
            icon::clock().size(FONT_ICON),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_LG)
        .height(Length::Fill),
    )
    .on_press(on_toggle.clone())
    .padding(PADDING_GROUPED)
    .height(Length::Fixed(CONTROL_HEIGHT))
    .style(picker_button())
    .into();

    let time = parse_hhmm(value)
        .map(|(h, m)| Time::Hm {
            hour: h as u32,
            minute: m as u32,
            period: Period::H24,
        })
        .unwrap_or_else(|| Time::now_hm(true));

    let inner: Element<'a, M, iced::Theme, iced::Renderer> =
        AwTimePicker::new(open, time, underlay, on_toggle.clone(), move |t: Time| {
            (on_change)(t.to_string())
        })
        .use_24h()
        .into();

    Element::new(TimePickerStateful { open, value, inner })
}

struct TimePickerStateful<'a, Message> {
    open: bool,
    value: &'a str,
    inner: Element<'a, Message, iced::Theme, iced::Renderer>,
}

struct TimePickerStatefulState {
    prev_open: bool,
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for TimePickerStateful<'a, Message>
where
    Message: 'static + Clone,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<TimePickerStatefulState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(TimePickerStatefulState { prev_open: false })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.inner)]
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<TimePickerStatefulState>();
        if self.open && !state.prev_open {
            if let Some((h, m)) = parse_hhmm(self.value) {
                tree.children[0].state = tree::State::new(PickerState::new(
                    Time::Hm {
                        hour: h as u32,
                        minute: m as u32,
                        period: Period::H24,
                    },
                    true,
                    false,
                ));
            }
        }
        state.prev_open = self.open;
        tree.diff_children(&[&self.inner]);
    }

    fn size(&self) -> Size<Length> {
        self.inner.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.inner
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.inner.as_widget().draw(
            &state.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.inner
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.inner.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.inner.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.inner.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
