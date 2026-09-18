//! Custom RGB accent picker: HSV canvases (saturation×value + hue bar),
//! HEX text input, and a recently-used history row. Designed for the
//! "Appearance > Theme Color" section; emits high-level `SettingsMsg`s
//! so the owning page owns persistence and theme rebuilds.
//!
//! The widget is purely a view layer: it stores no state of its own, reads
//! `CustomColorPickerUi` from the caller, and forwards user edits via the
//! supplied message mapper.

use iced::mouse;
use iced::widget::canvas::{self, Event, Fill, Geometry, Path, Stroke, Style};
use iced::widget::{button as ibutton, column, container, row, text, text_input};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::config::MAX_CUSTOM_COLOR_HISTORY;
use crate::i18n::{Fluent, Tr};
use crate::message::{Message, SettingsMsg};
use crate::ui::dims::*;
use crate::ui::theme;

const SV_PANEL_SIZE: f32 = 200.0;
const HUE_BAR_HEIGHT: f32 = 16.0;
const MARKER_RADIUS: f32 = 6.0;
const PICKER_INSET: f32 = 2.0;

/// HSV coordinates used internally by the picker. `hue` is in degrees
/// 0–360, `sat` and `val` are 0.0–1.0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HsvColor {
    /// Hue in degrees, 0–360.
    pub hue: f32,
    /// Saturation, 0–1.
    pub sat: f32,
    /// Value (brightness), 0–1.
    pub val: f32,
}

impl Default for HsvColor {
    fn default() -> Self {
        Self {
            hue: 0.0,
            sat: 0.0,
            val: 0.0,
        }
    }
}

/// Convert HSV to an opaque [`iced::Color`].
pub fn hsv_to_color(h: &HsvColor) -> Color {
    let hue_norm = (h.hue.rem_euclid(360.0)) / 360.0;
    let s = h.sat.clamp(0.0, 1.0);
    let v = h.val.clamp(0.0, 1.0);
    let i = (hue_norm * 6.0).floor();
    let f = hue_norm * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match (i as i32) % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::from_rgba(r, g, b, 1.0)
}

/// Convert an [`iced::Color`] into HSV (ignoring alpha). Hue is in
/// degrees 0–360; saturation and value are 0–1.
pub fn color_to_hsv(c: Color) -> HsvColor {
    let r = c.r.clamp(0.0, 1.0);
    let g = c.g.clamp(0.0, 1.0);
    let b = c.b.clamp(0.0, 1.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let v = max;
    let d = max - min;
    let s = if max <= 0.0 { 0.0 } else { d / max };
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / d) + 2.0)
    } else {
        60.0 * (((r - g) / d) + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    HsvColor {
        hue: h,
        sat: s,
        val: v,
    }
}

/// Snapshot of the picker's transient state. Owned by the parent
/// (`SettingsUiState::custom_color_picker`) and rendered read-only here.
#[derive(Debug, Clone)]
pub struct CustomColorPickerUi {
    /// Whether the inline panel is currently visible.
    pub open: bool,
    /// Current HSV; hue in 0–360, sat/val 0–1.
    pub hsv: HsvColor,
    /// Edit buffer for the HEX input (`#RRGGBBAA` or `#RRGGBB`).
    pub hex_input: String,
    /// Whether `hex_input` currently parses to a valid RGBA/RGB color.
    pub hex_valid: bool,
}

impl Default for CustomColorPickerUi {
    fn default() -> Self {
        Self {
            open: false,
            hsv: HsvColor::default(),
            hex_input: String::new(),
            hex_valid: true,
        }
    }
}

impl CustomColorPickerUi {
    /// Seed the picker from an accent hex string (e.g. `#RRGGBBAA`).
    /// Falls back to the package default accent if `hex` does not parse.
    pub fn seed_from(hex: &str) -> Self {
        let color = theme::accent_color(hex);
        let mut hsv = color_to_hsv(color);
        if hsv.val < 0.0001 {
            hsv.val = 1.0;
        }
        Self {
            open: true,
            hsv,
            hex_input: theme::color_to_hex(color),
            hex_valid: true,
        }
    }
}

/// Normalise user input into a candidate HEX string by stripping any
/// non-hex/non-`#` characters, uppercasing, and prepending `#` if missing.
#[allow(dead_code)]
pub fn sanitize_hex_input(raw: &str) -> String {
    let mut buf = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch == '#' || ch.is_ascii_hexdigit() {
            buf.push(ch);
        }
    }
    if buf.is_empty() {
        return String::new();
    }
    if !buf.starts_with('#') {
        buf.insert(0, '#');
    }
    buf.to_ascii_uppercase()
}

struct SvProgram {
    hsv: HsvColor,
    border: Color,
    marker_border: Color,
}

struct HueProgram {
    hsv: HsvColor,
    border: Color,
    marker_border: Color,
}

#[derive(Debug, Clone, Copy, Default)]
struct PickerState {
    pressed: bool,
}

impl SvProgram {
    fn draw_gradient(&self, frame: &mut canvas::Frame, bounds: Rectangle) {
        let cols = (bounds.width as i32).max(1);
        let rows = (bounds.height as i32).max(1);
        let pure = hsv_to_color(&HsvColor {
            hue: self.hsv.hue,
            sat: 1.0,
            val: 1.0,
        });
        let pw = bounds.width / cols as f32;
        let ph = bounds.height / rows as f32;
        for j in 0..rows {
            let v = 1.0 - (j as f32 + 0.5) / rows as f32;
            let base_r = pure.r * v;
            let base_g = pure.g * v;
            let base_b = pure.b * v;
            for i in 0..cols {
                let s = (i as f32 + 0.5) / cols as f32;
                let r = base_r + (1.0 - base_r) * (1.0 - s);
                let g = base_g + (1.0 - base_g) * (1.0 - s);
                let b = base_b + (1.0 - base_b) * (1.0 - s);
                frame.fill_rectangle(
                    Point::new(i as f32 * pw, j as f32 * ph),
                    Size::new(pw + 1.0, ph + 1.0),
                    Fill::from(Color::from_rgba(r, g, b, 1.0)),
                );
            }
        }
    }
}

impl canvas::Program<Message> for SvProgram {
    type State = PickerState;

    fn draw(
        &self,
        _state: &PickerState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        self.draw_gradient(&mut frame, bounds);
        let path = Path::rounded_rectangle(
            Point::new(0.0, 0.0),
            Size::new(bounds.width, bounds.height),
            PICKER_INSET.into(),
        );
        frame.stroke(
            &path,
            Stroke {
                style: Style::Solid(self.border),
                width: 1.0,
                ..Default::default()
            },
        );
        let cx = self.hsv.sat.clamp(0.0, 1.0) * bounds.width;
        let cy = (1.0 - self.hsv.val.clamp(0.0, 1.0)) * bounds.height;
        let marker = Path::circle(Point::new(cx, cy), MARKER_RADIUS);
        frame.stroke(
            &marker,
            Stroke {
                style: Style::Solid(self.marker_border),
                width: 2.0,
                ..Default::default()
            },
        );
        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        state: &mut PickerState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let Event::Mouse(mouse_event) = event else {
            return None;
        };
        match mouse_event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                if cursor.position_in(bounds).is_some() {
                    state.pressed = true;
                }
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                state.pressed = false;
            }
            _ => {}
        }
        if !state.pressed {
            return None;
        }
        let pos = cursor.position_in(bounds)?;
        let sat = (pos.x / bounds.width).clamp(0.0, 1.0);
        let val = 1.0 - (pos.y / bounds.height).clamp(0.0, 1.0);
        if (self.hsv.sat - sat).abs() < f32::EPSILON && (self.hsv.val - val).abs() < f32::EPSILON {
            return Some(canvas::Action::request_redraw());
        }
        let new_hsv = HsvColor {
            sat,
            val,
            ..self.hsv
        };
        Some(
            canvas::Action::publish(Message::Settings(SettingsMsg::CustomColorHsvChanged(
                new_hsv,
            )))
            .and_capture(),
        )
    }
}

impl HueProgram {
    fn draw_gradient(&self, frame: &mut canvas::Frame, bounds: Rectangle) {
        let cols = (bounds.width as i32).max(1);
        let pw = bounds.width / cols as f32;
        for i in 0..cols {
            let hue = (i as f32 + 0.5) / cols as f32 * 360.0;
            let color = hsv_to_color(&HsvColor {
                hue,
                sat: 1.0,
                val: 1.0,
            });
            frame.fill_rectangle(
                Point::new(i as f32 * pw, 0.0),
                Size::new(pw + 1.0, bounds.height),
                Fill::from(color),
            );
        }
    }
}

impl canvas::Program<Message> for HueProgram {
    type State = PickerState;

    fn draw(
        &self,
        _state: &PickerState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        self.draw_gradient(&mut frame, bounds);
        let path = Path::rounded_rectangle(
            Point::new(0.0, 0.0),
            Size::new(bounds.width, bounds.height),
            PICKER_INSET.into(),
        );
        frame.stroke(
            &path,
            Stroke {
                style: Style::Solid(self.border),
                width: 1.0,
                ..Default::default()
            },
        );
        let cx = (self.hsv.hue.clamp(0.0, 360.0) / 360.0) * bounds.width;
        let marker = Path::circle(Point::new(cx, bounds.height / 2.0), MARKER_RADIUS);
        frame.stroke(
            &marker,
            Stroke {
                style: Style::Solid(self.marker_border),
                width: 2.0,
                ..Default::default()
            },
        );
        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        state: &mut PickerState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let Event::Mouse(mouse_event) = event else {
            return None;
        };
        match mouse_event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                if cursor.position_in(bounds).is_some() {
                    state.pressed = true;
                }
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                state.pressed = false;
            }
            _ => {}
        }
        if !state.pressed {
            return None;
        }
        let pos = cursor.position_in(bounds)?;
        let raw = (pos.x / bounds.width).clamp(0.0, 1.0) * 360.0;
        let new_hue = raw.rem_euclid(360.0);
        if (self.hsv.hue - new_hue).abs() < f32::EPSILON {
            return Some(canvas::Action::request_redraw());
        }
        let new_hsv = HsvColor {
            hue: new_hue,
            ..self.hsv
        };
        Some(
            canvas::Action::publish(Message::Settings(SettingsMsg::CustomColorHsvChanged(
                new_hsv,
            )))
            .and_capture(),
        )
    }
}

fn history_row<'a, F>(colors: &'a [String], on_select: F) -> Element<'a, Message>
where
    F: Fn(String) -> SettingsMsg + 'a,
{
    let mut items: Vec<Element<'a, Message>> = Vec::with_capacity(MAX_CUSTOM_COLOR_HISTORY);
    for slot in 0..MAX_CUSTOM_COLOR_HISTORY {
        if let Some(hex) = colors.get(slot) {
            let color = theme::accent_color(hex);
            let hex_owned = hex.clone();
            let btn = ibutton(iced::widget::Space::new())
                .on_press(Message::Settings(on_select(hex_owned)))
                .width(Length::Fixed(SWATCH_SIZE))
                .height(Length::Fixed(SWATCH_SIZE))
                .padding(0)
                .style(theme::style::button::swatch(color, false));
            items.push(btn.into());
        } else {
            items.push(
                iced::widget::Space::new()
                    .width(Length::Fixed(SWATCH_SIZE))
                    .height(Length::Fixed(SWATCH_SIZE))
                    .into(),
            );
        }
    }
    row(items)
        .spacing(SPACE_SM)
        .align_y(Alignment::Center)
        .into()
}

/// Layout the SV picker square + hue bar + HEX input + recently-used
/// history + Cancel/Apply row in a single vertical column. The SV/Hue
/// canvases publish `SettingsMsg::CustomColorHsvChanged` directly; the
/// remaining widgets call `on_hex` / `on_apply` / `on_cancel` /
/// `on_history_select` which must return the matching [`SettingsMsg`]s.
#[allow(clippy::too_many_arguments)]
pub fn view<'a, F3, F4, F5, F6>(
    fluent: &'a Fluent,
    theme: &'a Theme,
    ui: &'a CustomColorPickerUi,
    history: &'a [String],
    on_hex: F3,
    on_apply: F4,
    on_cancel: F5,
    on_history_select: F6,
) -> Element<'a, Message>
where
    F3: Fn(String) -> SettingsMsg + 'a,
    F4: Fn() -> SettingsMsg + 'a,
    F5: Fn() -> SettingsMsg + 'a,
    F6: Fn(String) -> SettingsMsg + 'a,
{
    let border = theme::border_color(theme);
    let marker_border = theme.extended_palette().background.base.text;

    let sv_canvas = canvas::Canvas::new(SvProgram {
        hsv: ui.hsv,
        border,
        marker_border,
    })
    .width(Length::Fill)
    .height(Length::Fixed(SV_PANEL_SIZE));

    let hue_canvas = canvas::Canvas::new(HueProgram {
        hsv: ui.hsv,
        border,
        marker_border,
    })
    .width(Length::Fill)
    .height(Length::Fixed(HUE_BAR_HEIGHT));

    let hue_label = format!("Hue: {:.0}°", ui.hsv.hue);

    let hex_placeholder = "#RRGGBB";
    let hex_input = theme::input_layout(
        text_input(hex_placeholder, &ui.hex_input)
            .on_input(move |s| Message::Settings(on_hex(sanitize_hex_input(&s))))
            .width(Length::Fill)
            .style(theme::style::input::standard),
    );

    let cancel_btn = ibutton(text(fluent.get(Tr::Cancel)).size(FONT_BODY))
        .on_press(Message::Settings(on_cancel()))
        .padding(PADDING_BUTTON_SM)
        .style(theme::style::button::secondary());

    let apply_btn = ibutton(text(fluent.get(Tr::Apply)).size(FONT_BODY))
        .on_press_maybe(if ui.hex_valid {
            Some(Message::Settings(on_apply()))
        } else {
            None
        })
        .padding(PADDING_BUTTON_SM)
        .style(theme::style::button::primary());

    let sv_column = column![sv_canvas].spacing(SPACE_XS).width(Length::Fill);

    let panel = column![
        sv_column,
        text(hue_label).size(FONT_SMALL),
        hue_canvas,
        row![text("Hex:").size(FONT_SMALL), hex_input]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center),
        history_row(history, on_history_select),
        row![
            iced::widget::Space::new().width(Length::Fill),
            cancel_btn,
            apply_btn,
        ]
        .spacing(SPACE_SM)
        .align_y(Alignment::Center),
    ]
    .spacing(SPACE_MD)
    .padding(iced::Padding {
        top: SPACE_MD,
        right: SPACE_LG,
        bottom: SPACE_MD,
        left: SPACE_LG,
    })
    .width(Length::Fill);

    container(panel)
        .width(Length::Fill)
        .style(theme::style::subtle)
        .into()
}
