use iced::widget::canvas;
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Theme};

const DOT_DIAMETER: f32 = 8.0;

#[derive(Debug, Clone, Copy)]
pub struct StatusDot {
    color: Color,
}

impl StatusDot {
    pub fn new(color: Color) -> Self {
        Self { color }
    }

    pub fn view<M>(self) -> Element<'static, M>
    where
        M: 'static,
    {
        canvas::Canvas::new(DotProgram { color: self.color })
            .width(Length::Fixed(DOT_DIAMETER))
            .height(Length::Fixed(DOT_DIAMETER))
            .into()
    }
}

struct DotProgram {
    color: Color,
}

impl<M> canvas::Program<M> for DotProgram {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let radius = DOT_DIAMETER / 2.0;
        let path = canvas::Path::circle(Point::new(radius, radius), radius);
        frame.fill(&path, self.color);
        vec![frame.into_geometry()]
    }
}
