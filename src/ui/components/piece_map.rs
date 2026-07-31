use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::message::Message;

const CELL_SIZE: f32 = 8.0;
const CELL_GAP: f32 = 1.0;
const WIDGET_HEIGHT: f32 = 160.0;

pub fn view<'a>(
    bitfield: Option<String>,
    num_pieces: u64,
    color_done: Color,
    color_missing: Color,
) -> Element<'a, Message> {
    let program = PieceMap {
        bitfield,
        num_pieces,
        color_done,
        color_missing,
        cache: canvas::Cache::new(),
    };
    canvas::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fixed(WIDGET_HEIGHT))
        .into()
}

struct PieceMap {
    bitfield: Option<String>,
    num_pieces: u64,
    color_done: Color,
    color_missing: Color,
    cache: canvas::Cache,
}

impl canvas::Program<Message> for PieceMap {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geom = self.cache.draw(renderer, bounds.size(), |frame| {
            let cell_step = CELL_SIZE + CELL_GAP;
            let cols = ((bounds.width - CELL_GAP) / cell_step).max(1.0) as u64;
            if cols == 0 || self.num_pieces == 0 {
                return;
            }

            let bits = self
                .bitfield
                .as_deref()
                .and_then(|bf| hex::decode(bf).ok())
                .unwrap_or_default();

            let mut piece_idx = 0u64;
            for &byte in &bits {
                for i in (0..8).rev() {
                    if piece_idx >= self.num_pieces {
                        break;
                    }
                    let col = piece_idx % cols;
                    let row = piece_idx / cols;
                    let x = CELL_GAP + col as f32 * cell_step;
                    let y = CELL_GAP + row as f32 * cell_step;
                    let color = if (byte >> i) & 1 == 1 {
                        self.color_done
                    } else {
                        self.color_missing
                    };
                    frame.fill_rectangle(Point::new(x, y), Size::new(CELL_SIZE, CELL_SIZE), color);
                    piece_idx += 1;
                }
                if piece_idx >= self.num_pieces {
                    break;
                }
            }
        });
        vec![geom]
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
