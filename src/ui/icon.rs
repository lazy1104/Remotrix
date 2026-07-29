// Generated automatically by iced_lucide at build time.
// Do not edit manually.
// c11bf35a1c6528432f2433c45c4daf981a4100c4bbe6b58c8330a0f0e1dd4ce4
use iced::widget::{text, Text};
use iced::Font;

pub const FONT: &[u8] = include_bytes!("../../fonts/lucide.ttf");

/// All icons as `(name, codepoint_str)` pairs.
/// Use this to populate an icon-picker widget.
#[allow(dead_code)]
pub const ALL_ICONS: &[(&str, &str)] = &[
    ("circle_check", "\u{E226}"),
    ("connections", "\u{E37F}"),
    ("copy", "\u{E09E}"),
    ("details", "\u{E0CC}"),
    ("download", "\u{E0B2}"),
    ("download_arrow", "\u{E455}"),
    ("eraser", "\u{E28F}"),
    ("folder_open", "\u{E247}"),
    ("globe", "\u{E0E8}"),
    ("info", "\u{E0F9}"),
    ("layers", "\u{E529}"),
    ("list", "\u{E106}"),
    ("magnet", "\u{E2B5}"),
    ("pause", "\u{E12E}"),
    ("play", "\u{E13C}"),
    ("plus", "\u{E13D}"),
    ("refresh", "\u{E145}"),
    ("settings", "\u{E154}"),
    ("share", "\u{E156}"),
    ("sliders", "\u{E29A}"),
    ("sort", "\u{E37D}"),
    ("trash", "\u{E18E}"),
    ("wrench", "\u{E1B1}"),
];

pub fn circle_check<'a>() -> Text<'a> {
    icon("\u{E226}")
}

pub fn connections<'a>() -> Text<'a> {
    icon("\u{E37F}")
}

pub fn copy<'a>() -> Text<'a> {
    icon("\u{E09E}")
}

pub fn details<'a>() -> Text<'a> {
    icon("\u{E0CC}")
}

pub fn download<'a>() -> Text<'a> {
    icon("\u{E0B2}")
}

pub fn download_arrow<'a>() -> Text<'a> {
    icon("\u{E455}")
}

pub fn eraser<'a>() -> Text<'a> {
    icon("\u{E28F}")
}

pub fn folder_open<'a>() -> Text<'a> {
    icon("\u{E247}")
}

pub fn globe<'a>() -> Text<'a> {
    icon("\u{E0E8}")
}

pub fn info<'a>() -> Text<'a> {
    icon("\u{E0F9}")
}

pub fn layers<'a>() -> Text<'a> {
    icon("\u{E529}")
}

pub fn list<'a>() -> Text<'a> {
    icon("\u{E106}")
}

pub fn magnet<'a>() -> Text<'a> {
    icon("\u{E2B5}")
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

pub fn share<'a>() -> Text<'a> {
    icon("\u{E156}")
}

pub fn sliders<'a>() -> Text<'a> {
    icon("\u{E29A}")
}

pub fn sort<'a>() -> Text<'a> {
    icon("\u{E37D}")
}

pub fn trash<'a>() -> Text<'a> {
    icon("\u{E18E}")
}

pub fn wrench<'a>() -> Text<'a> {
    icon("\u{E1B1}")
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
