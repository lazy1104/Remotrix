//! Drop zone + file-picker widget for selecting a single `.torrent` file in
//! the Add dialog. Tracks its own `dragging` / `hovered` mouse state and
//! forwards user intents through the [`TorrentUploadEvent`] →
//! [`TorrentUploadAction`] pipeline so the parent can drive the file picker
//! and clear the selection.

use std::path::Path;

use iced::mouse;
use iced::widget::{button, canvas, column, container, mouse_area, row, stack, text};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use super::tooltip;
use crate::i18n::{Fluent, Tr};
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

/// Maximum `.torrent` file size, in bytes, that [`is_valid_torrent_file`]
/// will accept. The 50 MiB ceiling is a defensive guard against malicious
/// or accidentally gigantic files being loaded into memory during
/// validation; legitimate torrents are well under this bound.
pub const MAX_TORRENT_SIZE: u64 = 50 * 1024 * 1024;

const DROP_ZONE_HEIGHT: f32 = 120.0;
const BORDER_WIDTH: f32 = 1.0;
const DASH_SEGMENTS: [f32; 2] = [4.0, 4.0];

/// User-initiated signals the drop zone forwards to its parent via
/// the message mapper closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorrentUploadEvent {
    /// The user asked to pick a file (clicked the drop zone in its empty
    /// state, or clicked the file-name label in its loaded state).
    Browse,
    /// The user pressed the inline "×" button to clear the loaded path.
    Clear,
    /// The pointer entered the drop zone.
    Entered,
    /// The pointer left the drop zone.
    Exited,
}

/// Actions the widget asks its parent to perform in response to an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorrentUploadAction {
    /// Open the platform file picker; the parent owns the dialog and
    /// will call [`TorrentUpload::set_path`] with the chosen file.
    Browse,
}

#[derive(Debug, Clone)]
pub struct TorrentUpload {
    /// Absolute path of the currently selected torrent file, or empty when
    /// no file is loaded. Consumed by the parent to feed `add_torrent` /
    /// bencode parsing.
    path: String,
    /// True while an OS drag operation with file payloads is hovering over
    /// the drop zone; toggled by the parent in response to drag events.
    dragging: bool,
    /// True when the pointer is inside the drop zone; drives the accent
    /// border color in the view.
    hovered: bool,
}

/// Return `true` when `p` ends with a `.torrent` extension (ASCII-case
/// insensitive). Matches on the last extension component only, so paths
/// like `dir/.torrent/foo` are rejected.
///
/// Does not touch the filesystem; the parent decides whether to also call
/// [`is_valid_torrent_file`] before accepting the file.
pub fn is_torrent_file(p: &Path) -> bool {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase() == "torrent")
        .unwrap_or(false)
}

/// Validate that `p` is a reasonable torrent file before parsing it:
/// extension is `.torrent`, the file is non-empty, at most
/// [`MAX_TORRENT_SIZE`] bytes long, readable, and starts with the bencode
/// dictionary marker (`b'd'`). Any filesystem or read error causes the
/// function to return `false` rather than propagating the error.
pub fn is_valid_torrent_file(p: &Path) -> bool {
    if !is_torrent_file(p) {
        return false;
    }
    let Ok(meta) = std::fs::metadata(p) else {
        return false;
    };
    if meta.len() == 0 || meta.len() > MAX_TORRENT_SIZE {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(p) else {
        return false;
    };
    let mut buf = [0u8; 1];
    use std::io::Read;
    if f.read_exact(&mut buf).is_err() {
        return false;
    }
    buf[0] == b'd'
}

struct DashedBorder {
    color: Color,
    radius: f32,
    width: f32,
}

impl<M> canvas::Program<M> for DashedBorder {
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
        let inset = self.width / 2.0;
        let path = canvas::Path::rounded_rectangle(
            Point::new(inset, inset),
            Size::new(bounds.width - self.width, bounds.height - self.width),
            self.radius.into(),
        );
        let stroke = canvas::Stroke {
            style: canvas::Style::Solid(self.color),
            width: self.width,
            line_dash: canvas::LineDash {
                segments: &DASH_SEGMENTS,
                offset: 0,
            },
            ..Default::default()
        };
        frame.stroke(&path, stroke);
        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut (),
        _event: &canvas::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<M>> {
        None
    }
}

impl TorrentUpload {
    pub fn new() -> Self {
        Self {
            path: String::new(),
            dragging: false,
            hovered: false,
        }
    }

    pub fn set_path(&mut self, path: impl Into<String>) {
        self.path = path.into();
        self.dragging = false;
        self.hovered = false;
    }

    pub fn clear(&mut self) {
        self.path.clear();
        self.dragging = false;
        self.hovered = false;
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    pub fn set_dragging(&mut self, dragging: bool) {
        self.dragging = dragging;
    }

    pub fn update(&mut self, event: TorrentUploadEvent) -> Option<TorrentUploadAction> {
        match event {
            TorrentUploadEvent::Browse => Some(TorrentUploadAction::Browse),
            TorrentUploadEvent::Clear => {
                self.clear();
                None
            }
            TorrentUploadEvent::Entered => {
                self.hovered = true;
                None
            }
            TorrentUploadEvent::Exited => {
                self.hovered = false;
                None
            }
        }
    }

    pub fn view<'a, M>(
        &'a self,
        fluent: &'a Fluent,
        theme: &'a iced::Theme,
        map: impl Fn(TorrentUploadEvent) -> M + 'a,
    ) -> Element<'a, M>
    where
        M: Clone + 'a,
    {
        let border_color = if self.hovered || self.dragging {
            theme::accent(theme)
        } else {
            theme::border_color(theme)
        };
        let dashed = canvas::Canvas::new(DashedBorder {
            color: border_color,
            radius: theme::RADIUS_BUTTON,
            width: BORDER_WIDTH,
        })
        .width(Length::Fill)
        .height(Length::Fixed(DROP_ZONE_HEIGHT));

        if self.is_empty() {
            let hint = text(if self.dragging {
                fluent.get(Tr::DropTorrentActive)
            } else {
                fluent.get(Tr::DropTorrentHint)
            })
            .size(FONT_MEDIUM);
            let content = column![icon::arrow_up().size(FONT_HERO), hint]
                .spacing(SPACE_LG)
                .align_x(Alignment::Center);
            let zone = container(content)
                .width(Length::Fill)
                .height(Length::Fixed(DROP_ZONE_HEIGHT))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(theme::style::drop_zone(self.dragging));
            mouse_area(
                stack![zone, dashed]
                    .width(Length::Fill)
                    .height(Length::Fixed(DROP_ZONE_HEIGHT)),
            )
            .on_press(map(TorrentUploadEvent::Browse))
            .on_enter(map(TorrentUploadEvent::Entered))
            .on_exit(map(TorrentUploadEvent::Exited))
            .interaction(mouse::Interaction::Pointer)
            .into()
        } else {
            let text_secondary = theme::text_secondary(theme);
            let name = Path::new(&self.path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.path.clone());
            let info = column![
                text(name).size(FONT_BODY),
                text(&self.path)
                    .size(FONT_SMALL)
                    .style(theme::style::text::secondary),
            ]
            .spacing(SPACE_SM)
            .width(Length::Fill);

            let reselect = mouse_area(info)
                .on_press(map(TorrentUploadEvent::Browse))
                .interaction(mouse::Interaction::Pointer);

            let clear_btn = tooltip::standard(
                button(icon::x().size(FONT_ICON).color(text_secondary))
                    .on_press(map(TorrentUploadEvent::Clear))
                    .padding(PADDING_XS)
                    .style(theme::style::button::secondary()),
                text(fluent.get(Tr::Remove)).size(FONT_SMALL),
                iced::widget::tooltip::Position::Top,
            );

            let zone = container(
                row![reselect, clear_btn]
                    .spacing(SPACE_MD)
                    .align_y(Alignment::Center)
                    .padding(PADDING_CARD),
            )
            .width(Length::Fill)
            .height(Length::Fixed(DROP_ZONE_HEIGHT))
            .align_y(Alignment::Center)
            .style(theme::style::drop_zone(self.dragging));
            mouse_area(
                stack![zone, dashed]
                    .width(Length::Fill)
                    .height(Length::Fixed(DROP_ZONE_HEIGHT)),
            )
            .on_enter(map(TorrentUploadEvent::Entered))
            .on_exit(map(TorrentUploadEvent::Exited))
            .into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn is_torrent_file_basic() {
        assert!(is_torrent_file(&PathBuf::from("foo.torrent")));
    }

    #[test]
    fn is_torrent_file_uppercase() {
        assert!(is_torrent_file(&PathBuf::from("FOO.TORRENT")));
        assert!(is_torrent_file(&PathBuf::from("Foo.Torrent")));
    }

    #[test]
    fn is_torrent_file_no_extension() {
        assert!(!is_torrent_file(&PathBuf::from("foo")));
    }

    #[test]
    fn is_torrent_file_other_ext() {
        assert!(!is_torrent_file(&PathBuf::from("foo.txt")));
        assert!(!is_torrent_file(&PathBuf::from("foo.metalink")));
        assert!(!is_torrent_file(&PathBuf::from("foo.meta4")));
    }

    #[test]
    fn is_torrent_file_in_path() {
        assert!(!is_torrent_file(&PathBuf::from("dir/.torrent/foo")));
    }
}
