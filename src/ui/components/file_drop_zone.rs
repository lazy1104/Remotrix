//! Generic single-file drop zone + file-picker widget. Reusable for any
//! file-type that the parent validates on its own (e.g. `.torrent` or
//! `.metalink`); this widget intentionally knows nothing about extensions
//! or file validation. Callers pass the localized hint / hint-active text
//! they want shown, plus a message mapper closure.

use std::path::Path;

use iced::mouse;
use iced::widget::{button, canvas, column, container, mouse_area, row, stack, text};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use super::tooltip;
use crate::i18n::{Fluent, Tr};
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

const DROP_ZONE_HEIGHT: f32 = 120.0;
const BORDER_WIDTH: f32 = 1.0;
const DASH_SEGMENTS: [f32; 2] = [4.0, 4.0];

/// User-initiated signals the drop zone forwards to its parent via
/// the message mapper closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDropEvent {
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
pub enum FileDropAction {
    /// Open the platform file picker; the parent owns the dialog and
    /// will call [`FileDropZone::set_path`] with the chosen file.
    Browse,
}

#[derive(Debug, Clone)]
pub struct FileDropZone {
    /// Absolute path of the currently selected file, or empty when no file
    /// is loaded. Consumed by the parent to feed whatever downstream logic
    /// the chosen file-type requires (bencode parsing, metalink XML, etc.).
    path: String,
    /// True while an OS drag operation with file payloads is hovering over
    /// the drop zone; toggled by the parent in response to drag events.
    dragging: bool,
    /// True when the pointer is inside the drop zone; drives the accent
    /// border color in the view.
    hovered: bool,
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

impl FileDropZone {
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

    pub fn update(&mut self, event: FileDropEvent) -> Option<FileDropAction> {
        match event {
            FileDropEvent::Browse => Some(FileDropAction::Browse),
            FileDropEvent::Clear => {
                self.clear();
                None
            }
            FileDropEvent::Entered => {
                self.hovered = true;
                None
            }
            FileDropEvent::Exited => {
                self.hovered = false;
                None
            }
        }
    }

    pub fn view<'a, M>(
        &'a self,
        fluent: &'a Fluent,
        theme: &'a iced::Theme,
        hint: Tr,
        hint_active: Tr,
        map: impl Fn(FileDropEvent) -> M + 'a,
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
            let hint_text = text(if self.dragging {
                fluent.get(hint_active)
            } else {
                fluent.get(hint)
            })
            .size(FONT_MEDIUM);
            let content = column![icon::arrow_up().size(FONT_HERO), hint_text]
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
            .on_press(map(FileDropEvent::Browse))
            .on_enter(map(FileDropEvent::Entered))
            .on_exit(map(FileDropEvent::Exited))
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
                .on_press(map(FileDropEvent::Browse))
                .interaction(mouse::Interaction::Pointer);

            let clear_btn = tooltip::standard(
                button(icon::x().size(FONT_ICON).color(text_secondary))
                    .on_press(map(FileDropEvent::Clear))
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
            .on_enter(map(FileDropEvent::Entered))
            .on_exit(map(FileDropEvent::Exited))
            .into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        let zone = FileDropZone::new();
        assert!(zone.is_empty());
        assert_eq!(zone.path(), "");
    }

    #[test]
    fn set_path_then_clear() {
        let mut zone = FileDropZone::new();
        zone.set_path("/tmp/foo.metalink");
        assert!(!zone.is_empty());
        assert_eq!(zone.path(), "/tmp/foo.metalink");
        zone.clear();
        assert!(zone.is_empty());
    }

    #[test]
    fn update_browse_emits_action() {
        let mut zone = FileDropZone::new();
        let action = zone.update(FileDropEvent::Browse);
        assert!(matches!(action, Some(FileDropAction::Browse)));
    }

    #[test]
    fn update_clear_resets_state() {
        let mut zone = FileDropZone::new();
        zone.set_path("/tmp/foo.torrent");
        zone.set_dragging(true);
        zone.update(FileDropEvent::Entered);
        let action = zone.update(FileDropEvent::Clear);
        assert!(action.is_none());
        assert!(zone.is_empty());
        assert_eq!(zone.path(), "");
    }

    #[test]
    fn update_enter_exit_toggles_hover() {
        let mut zone = FileDropZone::new();
        zone.update(FileDropEvent::Entered);
        zone.update(FileDropEvent::Exited);
        let action = zone.update(FileDropEvent::Exited);
        assert!(action.is_none());
    }
}
