use iced::widget::text;
use iced::Color;

pub const SIDEBAR_W: f32 = 64.0;
pub const CATEGORY_W: f32 = 180.0;

pub fn icon_text<'a>(ch: char, color: Color) -> text::Text<'a, iced::Theme, iced::Renderer> {
    text(ch.to_string()).size(18).color(color)
}

pub fn icon_small<'a>(ch: char) -> text::Text<'a, iced::Theme, iced::Renderer> {
    text(ch.to_string()).size(15)
}
