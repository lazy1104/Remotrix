use iced::mouse;
use iced::widget::canvas;
use iced::{Element, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::message::Message;

const CELL_SIZE: f32 = 8.0;
const CELL_GAP: f32 = 1.0;
const MAX_HEIGHT: f32 = 160.0;
const DOWNLOADING_WINDOW: i64 = 8;

pub fn view<'a>(
    bitfield: Option<String>,
    num_pieces: u64,
    avail_width: f32,
) -> Option<Element<'a, Message>> {
    if num_pieces == 0 || bitfield.as_deref().unwrap_or("").trim().is_empty() {
        return None;
    }

    let cell_step = CELL_SIZE + CELL_GAP;
    let cols = ((avail_width - CELL_GAP) / cell_step).max(1.0) as u64;
    let rows = num_pieces.div_ceil(cols);
    let height = (rows as f32 * cell_step + CELL_GAP).min(MAX_HEIGHT);

    let program = PieceMap {
        bitfield,
        num_pieces,
        cache: canvas::Cache::new(),
    };
    Some(
        canvas::Canvas::new(program)
            .width(Length::Fill)
            .height(Length::Fixed(height))
            .into(),
    )
}

struct PieceMap {
    bitfield: Option<String>,
    num_pieces: u64,
    cache: canvas::Cache,
}

impl canvas::Program<Message> for PieceMap {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geom = self.cache.draw(renderer, bounds.size(), |frame| {
            let cell_step = CELL_SIZE + CELL_GAP;
            let cols = ((bounds.width - CELL_GAP) / cell_step).max(1.0) as u64;
            if cols == 0 || self.num_pieces == 0 {
                return;
            }

            let palette = theme.extended_palette();
            let color_done = palette.success.base.color;
            let color_missing = palette.background.weak.color;
            let color_downloading = color_done.scale_alpha(0.4);

            let bits = self
                .bitfield
                .as_deref()
                .and_then(|bf| hex::decode(bf).ok())
                .unwrap_or_default();

            let high_water = {
                let mut h = -1i64;
                let mut idx = 0u64;
                for &byte in &bits {
                    for i in (0..8).rev() {
                        if idx >= self.num_pieces {
                            break;
                        }
                        if (byte >> i) & 1 == 1 {
                            h = idx as i64;
                        }
                        idx += 1;
                    }
                    if idx >= self.num_pieces {
                        break;
                    }
                }
                h
            };

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
                        color_done
                    } else if piece_idx as i64 <= high_water + DOWNLOADING_WINDOW {
                        color_downloading
                    } else {
                        color_missing
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
