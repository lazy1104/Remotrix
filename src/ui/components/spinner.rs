use std::time::{Duration, Instant};

use iced::widget::canvas;
use iced::{
    alignment, mouse, Color, Element, Font, Length, Pixels, Point, Rectangle, Renderer, Theme,
    Vector,
};

use crate::message::Message;

const SPIN_PERIOD: Duration = Duration::from_millis(900);
const REDRAW_PERIOD: Duration = Duration::from_millis(16);

pub struct Spinner {
    codepoint: char,
    color: Color,
    size: f32,
}

impl Spinner {
    pub fn glyph(codepoint: char, color: Color, size: f32) -> Self {
        Self {
            codepoint,
            color,
            size,
        }
    }

    pub fn hourglass(color: Color, size: f32) -> Self {
        Self::glyph('\u{E296}', color, size)
    }

    pub fn refresh(color: Color, size: f32) -> Self {
        Self::glyph('\u{E145}', color, size)
    }

    pub fn view(self) -> Element<'static, Message> {
        let size = self.size;
        canvas::Canvas::new(SpinnerProgram { spinner: self })
            .width(Length::Fixed(size * 1.6))
            .height(Length::Fixed(size * 1.6))
            .into()
    }
}

struct SpinnerProgram {
    spinner: Spinner,
}

impl canvas::Program<Message> for SpinnerProgram {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        _event: &canvas::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        Some(canvas::Action::request_redraw_at(
            Instant::now() + REDRAW_PERIOD,
        ))
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let angle = crate::ui::animation::spin(Instant::now(), SPIN_PERIOD);
        frame.with_save(|f| {
            f.translate(Vector::new(bounds.width / 2.0, bounds.height / 2.0));
            f.rotate(iced::Degrees(angle));
            f.fill_text(canvas::Text {
                content: self.spinner.codepoint.to_string(),
                position: Point::ORIGIN,
                max_width: f32::INFINITY,
                color: self.spinner.color,
                size: Pixels(self.spinner.size),
                line_height: Default::default(),
                font: Font::with_name("lucide"),
                align_x: iced::advanced::text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: Default::default(),
            });
        });
        vec![frame.into_geometry()]
    }
}
