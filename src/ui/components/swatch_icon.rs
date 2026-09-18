//! Geometry-only icons rendered with [`iced::widget::canvas`] so they
//! do not depend on any font metrics. Used by the theme-color swatch
//! buttons in `Settings → Theme` so the selected/plus marks look the
//! same on Linux (fontdb+FreeType) and Windows (DirectWrite) —
//! cosmic-text's line-box measurements differ slightly between the two
//! backends, which made the previous `lucide.ttf`-glyph icons appear
//! offset by a couple of pixels on Windows.

use iced::widget::canvas;
use iced::{Color, Element, Length, Point, Renderer, Theme};

const CHECK_FRAME: f32 = 16.0;
const PLUS_FRAME: f32 = 12.0;
const STROKE_WIDTH: f32 = 2.0;

pub fn swatch_check<'a, Message>(color: Color) -> Element<'a, Message>
where
    Message: 'a + Clone,
{
    canvas::Canvas::new(CheckProgram { color })
        .width(Length::Fixed(CHECK_FRAME))
        .height(Length::Fixed(CHECK_FRAME))
        .into()
}

pub fn swatch_plus<'a, Message>(color: Color) -> Element<'a, Message>
where
    Message: 'a + Clone,
{
    canvas::Canvas::new(PlusProgram { color })
        .width(Length::Fixed(PLUS_FRAME))
        .height(Length::Fixed(PLUS_FRAME))
        .into()
}

// keep in sync with theme::style::button::swatch luminance check
pub fn swatch_text_color(color: Color) -> Color {
    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance > 0.55 {
        Color::from_rgb8(0x11, 0x11, 0x11)
    } else {
        Color::WHITE
    }
}

struct CheckProgram {
    color: Color,
}

impl<M> canvas::Program<M> for CheckProgram {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let path = canvas::Path::new(|p| {
            p.move_to(Point::new(3.0, 8.5));
            p.line_to(Point::new(6.5, 12.0));
            p.line_to(Point::new(13.0, 4.0));
        });
        frame.stroke(
            &path,
            canvas::Stroke {
                style: canvas::Style::Solid(self.color),
                width: STROKE_WIDTH,
                line_cap: canvas::LineCap::Round,
                line_join: canvas::LineJoin::Round,
                ..Default::default()
            },
        );
        vec![frame.into_geometry()]
    }
}

struct PlusProgram {
    color: Color,
}

impl<M> canvas::Program<M> for PlusProgram {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let path = canvas::Path::new(|p| {
            p.move_to(Point::new(2.0, 6.0));
            p.line_to(Point::new(10.0, 6.0));
            p.move_to(Point::new(6.0, 2.0));
            p.line_to(Point::new(6.0, 10.0));
        });
        frame.stroke(
            &path,
            canvas::Stroke {
                style: canvas::Style::Solid(self.color),
                width: STROKE_WIDTH,
                line_cap: canvas::LineCap::Round,
                line_join: canvas::LineJoin::Round,
                ..Default::default()
            },
        );
        vec![frame.into_geometry()]
    }
}
