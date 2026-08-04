// Generated automatically by iced_lucide at build time.
// Do not edit manually.
// 1b26fabcae9ec29dac3cc1699fa8b4bdd7c1b1dace99c67b8c30e658b8eb57ba
use iced::widget::{text, Text};
use iced::Font;

pub const FONT: &[u8] = include_bytes!("../../fonts/lucide.ttf");

/// All icons as `(name, codepoint_str)` pairs.
/// Use this to populate an icon-picker widget.
#[allow(dead_code)]
pub const ALL_ICONS: &[(&str, &str)] = &[
    ("arrow_up", "\u{E45A}"),
    ("chevron_down", "\u{E06D}"),
    ("chevron_right", "\u{E06F}"),
    ("circle_alert", "\u{E077}"),
    ("circle_check", "\u{E226}"),
    ("circle_help", "\u{E082}"),
    ("circle_x", "\u{E084}"),
    ("clock", "\u{E087}"),
    ("collapse", "\u{E228}"),
    ("connections", "\u{E37F}"),
    ("copy", "\u{E09E}"),
    ("download", "\u{E0B2}"),
    ("download_arrow", "\u{E455}"),
    ("eraser", "\u{E28F}"),
    ("expand", "\u{E211}"),
    ("external_link", "\u{E0B9}"),
    ("file", "\u{E0C0}"),
    ("folder", "\u{E0D7}"),
    ("folder_clock", "\u{E32F}"),
    ("folder_open", "\u{E247}"),
    ("globe", "\u{E0E8}"),
    ("info", "\u{E0F9}"),
    ("layers", "\u{E529}"),
    ("list", "\u{E106}"),
    ("magnet", "\u{E2B5}"),
    ("minus", "\u{E11C}"),
    ("pause", "\u{E12E}"),
    ("play", "\u{E13C}"),
    ("plus", "\u{E13D}"),
    ("refresh", "\u{E145}"),
    ("settings", "\u{E154}"),
    ("share", "\u{E156}"),
    ("sliders", "\u{E29A}"),
    ("sort", "\u{E37D}"),
    ("square", "\u{E167}"),
    ("trash", "\u{E18E}"),
    ("triangle_alert", "\u{E193}"),
    ("wrench", "\u{E1B1}"),
    ("x", "\u{E1B2}"),
];

pub fn arrow_up<'a>() -> Text<'a> {
    icon("\u{E45A}")
}

pub fn chevron_down<'a>() -> Text<'a> {
    icon("\u{E06D}")
}

pub fn chevron_right<'a>() -> Text<'a> {
    icon("\u{E06F}")
}

pub fn circle_alert<'a>() -> Text<'a> {
    icon("\u{E077}")
}

pub fn circle_check<'a>() -> Text<'a> {
    icon("\u{E226}")
}

pub fn circle_help<'a>() -> Text<'a> {
    icon("\u{E082}")
}

pub fn circle_x<'a>() -> Text<'a> {
    icon("\u{E084}")
}

pub fn collapse<'a>() -> Text<'a> {
    icon("\u{E228}")
}

pub fn connections<'a>() -> Text<'a> {
    icon("\u{E37F}")
}

pub fn copy<'a>() -> Text<'a> {
    icon("\u{E09E}")
}

pub fn download<'a>() -> Text<'a> {
    icon("\u{E0B2}")
}

pub fn download_arrow<'a>() -> Text<'a> {
    icon("\u{E455}")
}

pub fn expand<'a>() -> Text<'a> {
    icon("\u{E211}")
}

pub fn external_link<'a>() -> Text<'a> {
    icon("\u{E0B9}")
}

pub fn file<'a>() -> Text<'a> {
    icon("\u{E0C0}")
}

pub fn folder<'a>() -> Text<'a> {
    icon("\u{E0D7}")
}

pub fn folder_clock<'a>() -> Text<'a> {
    icon("\u{E32F}")
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

pub fn minus<'a>() -> Text<'a> {
    icon("\u{E11C}")
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

pub fn square<'a>() -> Text<'a> {
    icon("\u{E167}")
}

pub fn trash<'a>() -> Text<'a> {
    icon("\u{E18E}")
}

pub fn triangle_alert<'a>() -> Text<'a> {
    icon("\u{E193}")
}

pub fn wrench<'a>() -> Text<'a> {
    icon("\u{E1B1}")
}

pub fn x<'a>() -> Text<'a> {
    icon("\u{E1B2}")
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
