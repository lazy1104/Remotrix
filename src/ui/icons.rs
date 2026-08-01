use iced::widget::text;
use iced::Color;

use crate::ui::dims::*;

pub const SIDEBAR_W: f32 = 64.0;
pub const CATEGORY_W: f32 = 180.0;

pub fn icon_text<'a>(ch: char, color: Color) -> text::Text<'a, iced::Theme, iced::Renderer> {
    text(ch.to_string()).size(FONT_HERO).color(color)
}

pub fn icon_small<'a>(ch: char) -> text::Text<'a, iced::Theme, iced::Renderer> {
    text(ch.to_string()).size(FONT_ICON)
}
