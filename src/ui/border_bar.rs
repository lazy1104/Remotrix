//! Animated top-border progress bar.
//!
//! When the engine reports any background work in flight (aria2 startup,
//! aria2-next download, remote update check, app self-update download,
//! engine restart), the static 1px hairline border is replaced with a 2px
//! gradient bar whose primary-coloured hotspot slides from left to right
//! on a fixed 1.5 s loop. Idle state falls back to the existing static
//! border so the rest of the chrome is unchanged.

use std::time::{Duration, Instant};

use iced::widget::canvas::{self, Gradient};
use iced::widget::{container, Canvas};
use iced::{mouse, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

use crate::message::Message;
use crate::ui::theme;

/// Total cycle period of the moving hotspot. One full left-to-right sweep
/// per period; the visual repeats seamlessly because the gradient is
/// periodic in `2 * bounds.width`.
pub const PERIOD: Duration = Duration::from_millis(2000);

/// Height of the animated bar in logical pixels. Slightly thicker than the
/// 1px hairline so the gradient is visible at any DPI.
pub const HEIGHT: f32 = 3.0;

#[derive(Clone, Copy)]
struct BarProgram {
    phase: f32,
    primary: Color,
    border: Color,
    opacity: f32,
}

fn scale_alpha(c: Color, alpha: f32) -> Color {
    Color {
        a: c.a * alpha.clamp(0.0, 1.0),
        ..c
    }
}

impl canvas::Program<Message> for BarProgram {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let width = bounds.width;
        if width <= 0.0 {
            return vec![frame.into_geometry()];
        }

        let span = (width * 2.0).max(1.0);
        let shift = self.phase * width;

        frame.translate(Vector::new(-shift, 0.0));

        let path = canvas::Path::rectangle(Point::new(0.0, 0.0), Size::new(span, bounds.height));
        let alpha = self.opacity.clamp(0.0, 1.0);
        let linear = canvas::gradient::Linear::new(Point::new(0.0, 0.0), Point::new(span, 0.0))
            .add_stop(0.0, scale_alpha(self.border, alpha))
            .add_stop(0.25, scale_alpha(self.primary, alpha))
            .add_stop(0.5, scale_alpha(self.border, alpha))
            .add_stop(0.75, scale_alpha(self.primary, alpha))
            .add_stop(1.0, scale_alpha(self.border, alpha));

        frame.fill(&path, Gradient::Linear(linear));
        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut (),
        _event: &canvas::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        None
    }
}

/// Build the moving top-border bar. `opacity` (0..=1) scales the alpha
/// of every gradient stop so the canvas can fade in/out smoothly without
/// changing its shape. The caller is responsible for keeping the element
/// mounted; the value of `opacity` decides how visible the bar is.
pub fn view<'a>(t: &Theme, opacity: f32) -> Element<'a, Message> {
    let phase = crate::ui::animation::cycle(Instant::now(), PERIOD);
    let program = BarProgram {
        phase,
        primary: theme::primary(t),
        border: theme::border_color(t),
        opacity,
    };
    let canvas: Canvas<BarProgram, Message> = canvas::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fixed(HEIGHT));
    container(canvas)
        .width(Length::Fill)
        .height(Length::Fixed(HEIGHT))
        .into()
}
