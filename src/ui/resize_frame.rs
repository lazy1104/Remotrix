//! Eight-edge invisible resize frame drawn around the window content so
//! the user can drag to resize from any side or corner.
//!
//! Each edge posts a [`WindowMsg::ResizeWindow`] with the corresponding
//! `Direction`, which `iced` translates to the native resize cursor and
//! gesture.

use iced::widget::{column, container, mouse_area, row, text};
use iced::{mouse, Element, Length};

use crate::message::{Message, WindowMsg};
use crate::ui::dims::*;

/// Width of the resize hit-target on every edge.
pub const BORDER: f32 = 6.0;

fn strip(
    direction: iced::window::Direction,
    interaction: mouse::Interaction,
    width: Length,
    height: Length,
) -> Element<'static, Message> {
    mouse_area(
        container(text("").size(FONT_HIDDEN))
            .width(width)
            .height(height),
    )
    .on_press(Message::Window(WindowMsg::ResizeWindow(direction)))
    .interaction(interaction)
    .into()
}

/// Build the invisible resize frame. The element fills the window; the
/// actual visible chrome is drawn by the parent.
pub fn view<'a>() -> Element<'a, Message> {
    let top = row![
        strip(
            iced::window::Direction::NorthWest,
            mouse::Interaction::ResizingDiagonallyDown,
            Length::Fixed(BORDER),
            Length::Fixed(BORDER),
        ),
        strip(
            iced::window::Direction::North,
            mouse::Interaction::ResizingVertically,
            Length::Fill,
            Length::Fixed(BORDER),
        ),
        strip(
            iced::window::Direction::NorthEast,
            mouse::Interaction::ResizingDiagonallyUp,
            Length::Fixed(BORDER),
            Length::Fixed(BORDER),
        ),
    ]
    .width(Length::Fill)
    .height(Length::Fixed(BORDER));

    let mid = row![
        strip(
            iced::window::Direction::West,
            mouse::Interaction::ResizingHorizontally,
            Length::Fixed(BORDER),
            Length::Fill,
        ),
        container(text("").size(FONT_HIDDEN))
            .width(Length::Fill)
            .height(Length::Fill),
        strip(
            iced::window::Direction::East,
            mouse::Interaction::ResizingHorizontally,
            Length::Fixed(BORDER),
            Length::Fill,
        ),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    let bot = row![
        strip(
            iced::window::Direction::SouthWest,
            mouse::Interaction::ResizingDiagonallyUp,
            Length::Fixed(BORDER),
            Length::Fixed(BORDER),
        ),
        strip(
            iced::window::Direction::South,
            mouse::Interaction::ResizingVertically,
            Length::Fill,
            Length::Fixed(BORDER),
        ),
        strip(
            iced::window::Direction::SouthEast,
            mouse::Interaction::ResizingDiagonallyDown,
            Length::Fixed(BORDER),
            Length::Fixed(BORDER),
        ),
    ]
    .width(Length::Fill)
    .height(Length::Fixed(BORDER));

    column![top, mid, bot]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
