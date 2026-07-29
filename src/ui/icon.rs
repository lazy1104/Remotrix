// Generated automatically by iced_lucide at build time.
// Do not edit manually.
// 63f3e43322f83834261885001641db7095d54898a1718ac57abd640685bd383c
use iced::widget::{text, Text};
use iced::Font;

pub const FONT: &[u8] = include_bytes!("../../fonts/lucide.ttf");

/// All icons as `(name, codepoint_str)` pairs.
/// Use this to populate an icon-picker widget.
#[allow(dead_code)]
pub const ALL_ICONS: &[(&str, &str)] = &[
    ("connections", "\u{E37F}"),
    ("copy", "\u{E09E}"),
    ("details", "\u{E0CC}"),
    ("eraser", "\u{E28F}"),
    ("folder_open", "\u{E247}"),
    ("info", "\u{E0F9}"),
    ("list", "\u{E106}"),
    ("pause", "\u{E12E}"),
    ("play", "\u{E13C}"),
    ("plus", "\u{E13D}"),
    ("refresh", "\u{E145}"),
    ("settings", "\u{E154}"),
    ("sort", "\u{E37D}"),
    ("trash", "\u{E18E}"),
];

pub fn connections<'a>() -> Text<'a> {
    icon("\u{E37F}")
}

pub fn copy<'a>() -> Text<'a> {
    icon("\u{E09E}")
}

pub fn details<'a>() -> Text<'a> {
    icon("\u{E0CC}")
}

pub fn eraser<'a>() -> Text<'a> {
    icon("\u{E28F}")
}

pub fn folder_open<'a>() -> Text<'a> {
    icon("\u{E247}")
}

pub fn info<'a>() -> Text<'a> {
    icon("\u{E0F9}")
}

pub fn list<'a>() -> Text<'a> {
    icon("\u{E106}")
}

pub fn pause<'a>() -> Text<'a> {
    icon("\u{E12E}")
}

pub fn play<'a>() -> Text<'a> {
    icon("\u{E13C}")
}

pub fn plus<'a>() -> Text<'a> {
    icon("\u{E13D}")
}

pub fn refresh<'a>() -> Text<'a> {
    icon("\u{E145}")
}

pub fn settings<'a>() -> Text<'a> {
    icon("\u{E154}")
}

pub fn sort<'a>() -> Text<'a> {
    icon("\u{E37D}")
}

pub fn trash<'a>() -> Text<'a> {
    icon("\u{E18E}")
}

/// Render any Lucide icon by its codepoint string.
/// Use this together with [`ALL_ICONS`] to display icons dynamically:
/// ```ignore
/// for (name, cp) in ALL_ICONS {
///     button(render(cp)).on_press(Msg::Pick(name.to_string()))
/// }
/// ```
pub fn render(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("lucide"))
}

fn icon(codepoint: &str) -> Text<'_> {
    render(codepoint)
}
