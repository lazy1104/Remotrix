//! Theme system: colour tokens, accent-based palette generation, and the
//! `iced::Theme` factory used by every page.
//!
//! All colours flow from a single accent string in `Settings.theme_color`;
//! [`build_iced`] derives a light and dark palette from it via
//! [`crate::ui::color::hct`]. Component styling rules live in the [`style`]
//! submodule.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use iced::theme::Palette;
use iced::{Color, Font, Theme};

use crate::ui::dims;

/// User-facing choice for the colour scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeMode {
    Dark,
    Light,
    #[default]
    System,
}

/// Best-effort detection of the OS-level dark-mode preference. Falls back
/// to `false` (light) when the platform or `dark-light` cannot answer.
pub fn detect_dark() -> bool {
    matches!(
        dark_light::detect().unwrap_or(dark_light::Mode::Light),
        dark_light::Mode::Dark
    )
}

/// Resolve a [`ThemeMode`] to a concrete `is_dark` boolean. When `mode`
/// is [`ThemeMode::System`], `system_dark` is consulted first so the
/// caller can pin the value for tests.
pub fn resolve_mode(mode: ThemeMode, system_dark: Option<bool>) -> bool {
    match mode {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        ThemeMode::System => system_dark.unwrap_or_else(detect_dark),
    }
}

/// Name of the bundled CJK fallback font (HarmonyOS Sans SC).
pub const BUNDLED_FONT_NAME: &str = "HarmonyOS Sans SC";

static FONT_CACHE: LazyLock<Mutex<HashMap<String, Font>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Resolve a font family string into an [`iced::Font`], with a process-wide
/// cache so the `&'static str` lifetime requirement doesn't leak every
/// distinct call site.
pub fn font_from_family(family: &str) -> Font {
    let trimmed = family.trim();
    if trimmed.is_empty() {
        return Font::DEFAULT;
    }
    if let Some(font) = FONT_CACHE.lock().ok().and_then(|c| c.get(trimmed).copied()) {
        return font;
    }
    let leaked: &'static str = Box::leak(trimmed.to_string().into_boxed_str());
    let font = Font::with_name(leaked);
    if let Ok(mut cache) = FONT_CACHE.lock() {
        cache.insert(trimmed.to_string(), font);
    }
    font
}

static FONT_FAMILIES: std::sync::Mutex<Option<(crate::i18n::Locale, &'static [FontFamily])>> =
    std::sync::Mutex::new(None);
static FONT_RAW: OnceLock<Vec<Vec<(String, fontdb::Language)>>> = OnceLock::new();

/// One font family surfaced by [`system_font_families`]: `id` is the
/// English (or otherwise first) name recorded in the font's `name`
/// table and is what `iced::Font::with_name` plus
/// `Settings.font_family` expect; `display` is the localised name
/// resolved against the active UI locale, or `id` when no match is
/// found.
#[derive(Debug, Clone)]
pub struct FontFamily {
    pub id: String,
    pub display: String,
}

fn pick_display_name(
    families: &[(String, fontdb::Language)],
    locale: crate::i18n::Locale,
) -> (String, String) {
    let id = match families.first() {
        Some((name, _)) => name.clone(),
        None => return (String::new(), String::new()),
    };
    if locale != crate::i18n::Locale::ZhCN {
        return (id.clone(), id);
    }
    let display = families
        .iter()
        .skip(1)
        .find_map(|(name, lang)| {
            if lang.primary_language() == "Chinese" {
                Some(name.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| id.clone());
    (id, display)
}

fn raw_font_families() -> &'static [Vec<(String, fontdb::Language)>] {
    FONT_RAW
        .get_or_init(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            let mut raw: Vec<Vec<(String, fontdb::Language)>> =
                db.faces().map(|f| f.families.clone()).collect();
            raw.sort_by(|a, b| {
                let ka = a.first().map(|(n, _)| n.to_lowercase()).unwrap_or_default();
                let kb = b.first().map(|(n, _)| n.to_lowercase()).unwrap_or_default();
                ka.cmp(&kb)
            });
            raw.dedup_by(|a, b| {
                a.first().map(|(n, _)| n.to_lowercase()) == b.first().map(|(n, _)| n.to_lowercase())
            });
            raw
        })
        .as_slice()
}

/// Sorted, de-duplicated list of font families available on the system,
/// used by the settings dialog font picker. Each entry exposes both
/// the English `id` (for `Font::with_name` and persistence) and a
/// locale-aware `display` name. The cache rebuilds when the active
/// locale changes; each rebuild `Box::leak`s a fresh slice (rare,
/// only at startup and on locale switch).
pub fn system_font_families() -> &'static [FontFamily] {
    let locale = crate::i18n::current_locale();
    let mut cache = FONT_FAMILIES.lock().unwrap_or_else(|e| e.into_inner());
    let needs_rebuild = cache
        .as_ref()
        .map(|(cached_locale, _)| *cached_locale != locale)
        .unwrap_or(true);
    if needs_rebuild {
        let families: Vec<FontFamily> = raw_font_families()
            .iter()
            .map(|f| {
                let (id, display) = pick_display_name(f, locale);
                FontFamily { id, display }
            })
            .filter(|f| !f.id.is_empty())
            .collect();
        let leaked: &'static [FontFamily] = Box::leak(families.into_boxed_slice());
        *cache = Some((locale, leaked));
    }
    cache.as_ref().map(|(_, v)| *v).unwrap_or(&[])
}

/// Translucent black used as a scrim behind dialogs and dropdowns.
pub const OVERLAY: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.55,
};

/// Corner radius used by task cards and large surfaces.
pub const RADIUS_CARD: f32 = 8.0;
/// Corner radius for primary buttons.
pub const RADIUS_BUTTON: f32 = 6.0;
/// Corner radius for pill-shaped indicators (filter, tags, toast close).
pub const RADIUS_PILL: f32 = 40.0;
/// Corner radius for progress bars and toast progress.
pub const RADIUS_PROGRESS: f32 = 4.0;
/// Corner radius for the sidebar/active-pill nav highlight.
pub const RADIUS_NAV: f32 = 20.0;

/// Padding applied by [`input_layout`] to standalone text inputs.
pub const INPUT_PADDING: iced::Padding = iced::Padding::new(8.0);
/// Padding applied by [`grouped_input_layout`] to grouped inputs.
pub const INPUT_PADDING_GROUPED: iced::Padding = iced::Padding {
    top: 0.0,
    right: 10.0,
    bottom: 0.0,
    left: 10.0,
};

/// Apply the app-standard padding and font size to a standalone
/// `TextInput`.
pub fn input_layout<'a, Message: Clone>(
    input: iced::widget::TextInput<'a, Message>,
) -> iced::widget::TextInput<'a, Message> {
    input.padding(INPUT_PADDING).size(dims::FONT_MEDIUM)
}

/// Apply the app-standard padding and font size to a grouped
/// `TextInput` (a row inside a path picker / number stepper).
pub fn grouped_input_layout<'a, Message: Clone>(
    input: iced::widget::TextInput<'a, Message>,
) -> iced::widget::TextInput<'a, Message> {
    input.padding(INPUT_PADDING_GROUPED).size(dims::FONT_MEDIUM)
}

/// Apply the app-standard padding and font size to a `TextEditor`
/// (multi-line URL box).
pub fn editor_layout<'a, H, Message>(
    editor: iced::widget::TextEditor<'a, H, Message>,
) -> iced::widget::TextEditor<'a, H, Message>
where
    H: iced::advanced::text::Highlighter,
{
    editor.padding(INPUT_PADDING).size(dims::FONT_MEDIUM)
}

/// Default accent colour (`#5865F2`) used for first-launch themes.
pub const DEFAULT_THEME_COLOR: Color = Color::from_rgb8(0x58, 0x65, 0xF2);

const MIN_SEP: f64 = 25.0;

/// Build an [`iced::Theme`] from an accent colour and a dark/light flag,
/// deriving background, text, primary, danger, success and warning tones
/// via the HCT ramps in [`crate::ui::color::hct`].
pub fn build_iced(color: Color, dark: bool) -> iced::Theme {
    let seed = super::hct::Hct::from_rgb(color);
    let h = seed.hue;
    let c = seed.chroma;

    let error_h = super::hct::push_hue_away(25.0, h, MIN_SEP);
    let success_h = super::hct::push_hue_away(140.0, h, MIN_SEP);
    let warning_h = super::hct::push_hue_away(60.0, h, MIN_SEP);

    let (bg, text, primary_tone, danger_tone, success_tone, warning_tone) = if dark {
        (10.0, 90.0, 80.0, 80.0, 65.0, 70.0)
    } else {
        (98.0, 10.0, 40.0, 40.0, 35.0, 45.0)
    };

    let palette = Palette {
        background: super::hct::ramp(h, c * 0.10, bg),
        text: super::hct::ramp(h, c * 0.10, text),
        primary: super::hct::ramp(h, c, primary_tone),
        danger: super::hct::ramp(error_h, 84.0, danger_tone),
        success: super::hct::ramp(success_h, 70.0, success_tone),
        warning: super::hct::ramp(warning_h, 80.0, warning_tone),
    };
    iced::Theme::custom("remotrix", palette)
}

/// Format an [`iced::Color`] as `"#RRGGBBAA"`. Out-of-range components are
/// clamped before rounding.
pub fn color_to_hex(c: Color) -> String {
    let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        to(c.r),
        to(c.g),
        to(c.b),
        to(c.a)
    )
}

/// Parse a `"#RRGGBB"` (6 hex digits) or `"#RRGGBBAA"` (8 hex digits)
/// case-insensitive string into an [`iced::Color`]. Returns `None` for
/// any other shape; a 6-digit value gets `alpha = 1.0`.
pub fn color_from_hex(s: &str) -> Option<Color> {
    let h = s.trim().strip_prefix('#')?;
    if !h.is_ascii() || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let (r, g, b, a) = match h.len() {
        6 => (
            u8::from_str_radix(&h[0..2], 16).ok()?,
            u8::from_str_radix(&h[2..4], 16).ok()?,
            u8::from_str_radix(&h[4..6], 16).ok()?,
            0xFF,
        ),
        8 => (
            u8::from_str_radix(&h[0..2], 16).ok()?,
            u8::from_str_radix(&h[2..4], 16).ok()?,
            u8::from_str_radix(&h[4..6], 16).ok()?,
            u8::from_str_radix(&h[6..8], 16).ok()?,
        ),
        _ => return None,
    };
    Some(Color::from_rgba8(r, g, b, a as f32 / 255.0))
}

/// Parse `hex` as an accent color, falling back to
/// [`DEFAULT_THEME_COLOR`] on any parse error so a typo in settings can
/// never crash the UI.
pub fn accent_color(hex: &str) -> Color {
    color_from_hex(hex).unwrap_or(DEFAULT_THEME_COLOR)
}

/// Curated accent swatches exposed in the theme picker, paired with
/// their display name.
pub static CANDIDATE_COLORS: &[(Color, &str)] = &[
    (DEFAULT_THEME_COLOR, "Blue"),
    (Color::from_rgb8(0x63, 0x66, 0xF1), "Indigo"),
    (Color::from_rgb8(0xA8, 0x55, 0xF7), "Purple"),
    (Color::from_rgb8(0xEC, 0x48, 0x99), "Pink"),
    (Color::from_rgb8(0xEF, 0x44, 0x44), "Red"),
    (Color::from_rgb8(0xF9, 0x73, 0x16), "Orange"),
    (Color::from_rgb8(0xF5, 0x9E, 0x0B), "Amber"),
    (Color::from_rgb8(0x84, 0xCC, 0x16), "Lime"),
    (Color::from_rgb8(0x22, 0xC5, 0x5E), "Green"),
    (Color::from_rgb8(0x14, 0xB8, 0xA6), "Teal"),
    (Color::from_rgb8(0x0E, 0xA5, 0xE9), "Cyan"),
];

/// Borrow the [`CANDIDATE_COLORS`] table as `(Color, &str)` pairs for
/// widgets that want stable `'static` names.
pub fn candidate_colors() -> &'static [(Color, &'static str)] {
    CANDIDATE_COLORS
}

/// Primary accent colour of `t` (the active primary tone).
pub fn accent(t: &Theme) -> Color {
    t.extended_palette().primary.base.color
}

/// Success-tone colour of `t` (derived from green hue).
pub fn success(t: &Theme) -> Color {
    t.extended_palette().success.base.color
}

/// Warning-tone colour of `t` (amber/yellow hue).
pub fn warning(t: &Theme) -> Color {
    t.extended_palette().warning.base.color
}

/// Danger-tone colour of `t` (red hue, hue-pushed away from accent).
pub fn danger(t: &Theme) -> Color {
    t.extended_palette().danger.base.color
}

/// Alias for [`accent`] — the primary tone of the theme.
pub fn primary(t: &Theme) -> Color {
    t.extended_palette().primary.base.color
}

/// Weak (lower contrast) variant of the primary tone; used by progress
/// bars and chip backgrounds.
pub fn primary_weak(t: &Theme) -> Color {
    t.extended_palette().primary.weak.color
}

/// Colour for the per-task status bar: error/red, success/green,
/// primary-weak for paused and seeding, primary otherwise.
pub fn task_bar_color(t: &Theme, status: crate::task::TaskStatus, is_seeding: bool) -> Color {
    if is_seeding {
        return primary_weak(t);
    }
    match status {
        crate::task::TaskStatus::Paused => primary_weak(t),
        crate::task::TaskStatus::Error => danger(t),
        crate::task::TaskStatus::Completed => success(t),
        _ => primary(t),
    }
}

/// Text colour appropriate for rendering on the background base.
pub fn text_secondary(t: &Theme) -> Color {
    t.extended_palette().background.base.text
}

/// Lighter, lower-contrast variant of [`text_secondary`] used for
/// timestamps, metadata, and disabled labels.
pub fn text_weak(t: &Theme) -> Color {
    let bg = t.extended_palette().background.base.color;
    let txt = t.extended_palette().background.base.text;
    Color::from_rgba(
        txt.r * 0.4 + bg.r * 0.6,
        txt.g * 0.4 + bg.g * 0.6,
        txt.b * 0.4 + bg.b * 0.6,
        1.0,
    )
}

/// Hairline border colour derived from the background tones.
pub fn border_color(t: &Theme) -> Color {
    t.extended_palette().background.strong.color
}

/// Component-specific styling rules. Each function returns an
/// `iced::widget::*::Style` keyed off the active theme so callers can
/// just hand it to `.style(...)` on the widget.
pub mod style {
    use iced::{Color, Shadow, Vector};

    /// Container style that paints the background base colour; used as
    /// the root container of every page.
    pub fn base_background(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(t.extended_palette().background.base.color.into()),
            ..Default::default()
        }
    }

    /// Transparent container with a 1px hairline border — for the outer
    /// window frame.
    pub fn window_border(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: None,
            border: iced::Border {
                color: super::border_color(t),
                width: 1.0,
                radius: iced::border::radius(0),
            },
            ..Default::default()
        }
    }

    pub fn sidebar_background(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(t.extended_palette().background.strong.color.into()),
            ..Default::default()
        }
    }

    pub fn category_background(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(t.extended_palette().background.weak.color.into()),
            ..Default::default()
        }
    }

    pub fn card(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(t.extended_palette().background.weak.color.into()),
            border: iced::Border {
                color: super::border_color(t),
                width: 1.0,
                radius: iced::border::rounded(super::RADIUS_CARD).radius,
            },
            shadow: Shadow::default(),
            ..Default::default()
        }
    }

    pub fn subtle(t: &iced::Theme) -> iced::widget::container::Style {
        tree_frame(t)
    }

    pub fn tree_frame(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(t.extended_palette().background.base.color.into()),
            border: iced::Border {
                color: super::border_color(t),
                width: 1.0,
                radius: iced::border::rounded(super::RADIUS_CARD).radius,
            },
            ..Default::default()
        }
    }

    pub fn separator(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(super::border_color(t).into()),
            ..Default::default()
        }
    }

    pub fn grouped_frame_state(
        focused: bool,
        hovered: bool,
    ) -> impl Fn(&iced::Theme) -> iced::widget::container::Style {
        move |t| {
            let p = t.extended_palette();
            iced::widget::container::Style {
                background: Some(p.background.base.color.into()),
                border: iced::Border {
                    color: if focused || hovered {
                        p.primary.base.color
                    } else {
                        super::border_color(t)
                    },
                    width: 1.0,
                    radius: super::RADIUS_BUTTON.into(),
                },
                ..Default::default()
            }
        }
    }

    pub fn overlay(_t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(super::OVERLAY.into()),
            ..Default::default()
        }
    }

    pub fn drop_overlay(_t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(super::OVERLAY.into()),
            ..Default::default()
        }
    }

    pub fn drop_zone(active: bool) -> impl Fn(&iced::Theme) -> iced::widget::container::Style {
        move |t| {
            let accent = t.extended_palette().primary.base.color;
            let palette = t.extended_palette();
            iced::widget::container::Style {
                background: Some(if active {
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.18).into()
                } else {
                    palette.background.weak.color.into()
                }),
                text_color: Some(if active {
                    accent
                } else {
                    palette.background.weak.text
                }),
                border: iced::Border::default(),
                ..Default::default()
            }
        }
    }

    pub fn active_filter(t: &iced::Theme) -> iced::widget::container::Style {
        let accent = t.extended_palette().primary.base.color;
        iced::widget::container::Style {
            background: Some(Color::from_rgba(accent.r, accent.g, accent.b, 0.18).into()),
            text_color: Some(accent),
            border: iced::border::rounded(super::RADIUS_BUTTON),
            ..Default::default()
        }
    }

    pub fn count_badge(active: bool) -> impl Fn(&iced::Theme) -> iced::widget::container::Style {
        move |t: &iced::Theme| -> iced::widget::container::Style {
            let palette = t.extended_palette();
            let accent = palette.primary.base.color;
            iced::widget::container::Style {
                background: Some(if active {
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.18).into()
                } else {
                    palette.background.base.color.into()
                }),
                text_color: Some(if active {
                    accent
                } else {
                    palette.background.base.text
                }),
                border: iced::Border {
                    radius: super::RADIUS_PILL.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }
    }

    fn capsule_pill(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(t.extended_palette().background.base.color.into()),
            border: iced::Border {
                color: t.extended_palette().background.strong.color,
                width: 1.0,
                radius: super::RADIUS_PILL.into(),
            },
            ..Default::default()
        }
    }

    pub fn toolbar_capsule(t: &iced::Theme) -> iced::widget::container::Style {
        capsule_pill(t)
    }

    pub fn toast(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(t.extended_palette().background.base.color.into()),
            border: iced::Border {
                color: t.extended_palette().background.strong.color,
                width: 1.0,
                radius: super::RADIUS_BUTTON.into(),
            },
            ..Default::default()
        }
    }

    pub fn tooltip(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(t.extended_palette().background.weak.color.into()),
            text_color: Some(t.extended_palette().background.weak.text),
            border: iced::Border {
                color: super::border_color(t),
                width: 1.0,
                radius: super::RADIUS_BUTTON.into(),
            },
            shadow: card_shadow(),
            ..Default::default()
        }
    }

    fn lighten(c: Color, amt: f32) -> Color {
        Color {
            r: c.r + (1.0 - c.r) * amt,
            g: c.g + (1.0 - c.g) * amt,
            b: c.b + (1.0 - c.b) * amt,
            a: c.a,
        }
    }

    fn darken(c: Color, amt: f32) -> Color {
        Color {
            r: c.r * (1.0 - amt),
            g: c.g * (1.0 - amt),
            b: c.b * (1.0 - amt),
            a: c.a,
        }
    }

    fn hover_overlay(t: &iced::Theme, alpha: f32) -> Color {
        t.extended_palette().background.base.text.scale_alpha(alpha)
    }

    fn button_shadow() -> Shadow {
        Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.18),
            offset: Vector::new(0.0, 1.0),
            blur_radius: 2.0,
        }
    }

    fn button_shadow_pressed() -> Shadow {
        Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.18),
            offset: Vector::new(0.0, 0.0),
            blur_radius: 1.0,
        }
    }

    fn card_shadow() -> Shadow {
        Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.08),
            offset: Vector::new(0.0, 1.0),
            blur_radius: 4.0,
        }
    }

    pub mod button {
        use iced::widget::button::{Status, Style};
        use iced::Color;
        use iced::{Background, Shadow};

        fn scale_alpha(c: Color, factor: f32) -> Color {
            Color {
                a: c.a * factor,
                ..c
            }
        }

        pub fn text<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let base_text = t.extended_palette().background.base.text;
                Style {
                    background: match status {
                        Status::Hovered => Some(super::hover_overlay(t, 0.08).into()),
                        Status::Pressed => Some(super::hover_overlay(t, 0.14).into()),
                        _ => None,
                    },
                    text_color: match status {
                        Status::Disabled => scale_alpha(base_text, 0.5),
                        _ => base_text,
                    },
                    border: iced::border::rounded(super::super::RADIUS_BUTTON),
                    shadow: Shadow::default(),
                    ..Default::default()
                }
            }
        }

        pub fn filter<'a>(active: bool) -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let accent = t.extended_palette().primary.base.color;
                let base_text = t.extended_palette().background.base.text;
                Style {
                    background: if active {
                        None
                    } else {
                        match status {
                            Status::Hovered | Status::Pressed => {
                                Some(super::hover_overlay(t, 0.08).into())
                            }
                            _ => None,
                        }
                    },
                    text_color: if active { accent } else { base_text },
                    border: iced::border::rounded(super::super::RADIUS_BUTTON),
                    shadow: Shadow::default(),
                    ..Default::default()
                }
            }
        }

        pub fn copyable<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let p = t.extended_palette();
                let border = match status {
                    Status::Hovered | Status::Pressed => p.primary.base.color,
                    _ => super::super::border_color(t),
                };
                Style {
                    background: Some(p.background.base.color.into()),
                    text_color: p.background.base.text,
                    border: iced::Border {
                        color: border,
                        width: 1.0,
                        radius: iced::border::rounded(super::super::RADIUS_BUTTON).radius,
                    },
                    shadow: Shadow::default(),
                    ..Default::default()
                }
            }
        }

        pub fn trigger<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let p = t.extended_palette();
                let border = match status {
                    Status::Hovered | Status::Pressed => p.primary.base.color,
                    _ => super::super::border_color(t),
                };
                Style {
                    background: Some(p.background.weak.color.into()),
                    text_color: p.background.base.text,
                    border: iced::Border {
                        color: border,
                        width: 1.0,
                        radius: iced::border::rounded(super::super::RADIUS_BUTTON).radius,
                    },
                    shadow: Shadow::default(),
                    ..Default::default()
                }
            }
        }

        pub fn picker_item<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let palette = t.extended_palette();
                let base_text = palette.background.base.text;
                let primary = palette.primary.base;
                Style {
                    background: match status {
                        Status::Hovered | Status::Pressed => Some(primary.color.into()),
                        _ => None,
                    },
                    text_color: match status {
                        Status::Hovered | Status::Pressed => primary.text,
                        Status::Disabled => scale_alpha(base_text, 0.5),
                        _ => base_text,
                    },
                    border: iced::border::rounded(super::super::RADIUS_BUTTON),
                    shadow: Shadow::default(),
                    ..Default::default()
                }
            }
        }

        pub fn chip<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let accent = t.extended_palette().primary.base.color;
                let alpha = match status {
                    Status::Hovered => 0.28,
                    Status::Pressed => 0.34,
                    Status::Disabled => 0.14,
                    _ => 0.18,
                };
                Style {
                    background: Some(Color::from_rgba(accent.r, accent.g, accent.b, alpha).into()),
                    text_color: accent,
                    border: iced::border::rounded(super::super::RADIUS_BUTTON),
                    shadow: Shadow::default(),
                    ..Default::default()
                }
            }
        }

        pub fn toolbar_icon<'a>(active: bool) -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let accent = t.extended_palette().primary.base.color;
                let base_text = t.extended_palette().background.base.text;
                if active {
                    Style {
                        background: match status {
                            Status::Disabled => None,
                            _ => Some(Color::from_rgba(accent.r, accent.g, accent.b, 0.18).into()),
                        },
                        text_color: match status {
                            Status::Disabled => scale_alpha(accent, 0.5),
                            _ => accent,
                        },
                        border: iced::border::rounded(super::super::RADIUS_PILL),
                        shadow: Shadow::default(),
                        ..Default::default()
                    }
                } else {
                    Style {
                        background: match status {
                            Status::Hovered => Some(super::hover_overlay(t, 0.08).into()),
                            Status::Pressed => Some(super::hover_overlay(t, 0.14).into()),
                            _ => None,
                        },
                        text_color: match status {
                            Status::Disabled => scale_alpha(base_text, 0.5),
                            _ => base_text,
                        },
                        border: iced::border::rounded(super::super::RADIUS_PILL),
                        shadow: Shadow::default(),
                        ..Default::default()
                    }
                }
            }
        }

        pub fn speed_hud<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let p = t.extended_palette();
                let border = match status {
                    Status::Hovered => p.primary.weak.color,
                    _ => p.background.strong.color,
                };
                Style {
                    background: Some(p.background.base.color.into()),
                    text_color: p.background.base.text,
                    border: iced::Border {
                        color: border,
                        width: 1.0,
                        radius: iced::border::rounded(super::super::RADIUS_PILL).radius,
                    },
                    shadow: super::card_shadow(),
                    ..Default::default()
                }
            }
        }

        fn filled(
            base_color: Color,
            strong_color: Color,
            text_color: Color,
            status: Status,
        ) -> Style {
            let actual_bg = match status {
                Status::Hovered => strong_color,
                Status::Pressed => super::darken(base_color, 0.15),
                _ => base_color,
            };
            let alpha = match status {
                Status::Disabled => 0.5,
                _ => 1.0,
            };
            let shadow = match status {
                Status::Pressed => super::button_shadow_pressed(),
                _ => super::button_shadow(),
            };
            Style {
                background: Some(Background::Color(Color {
                    a: actual_bg.a * alpha,
                    ..actual_bg
                })),
                text_color: Color {
                    a: text_color.a * alpha,
                    ..text_color
                },
                border: iced::border::rounded(super::super::RADIUS_BUTTON),
                shadow,
                ..Default::default()
            }
        }

        pub fn primary<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let p = t.extended_palette().primary;
                filled(p.base.color, p.strong.color, p.base.text, status)
            }
        }

        pub fn secondary<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let base_text = t.extended_palette().background.base.text;
                Style {
                    background: match status {
                        Status::Hovered => Some(super::hover_overlay(t, 0.08).into()),
                        Status::Pressed => Some(super::hover_overlay(t, 0.14).into()),
                        _ => None,
                    },
                    text_color: match status {
                        Status::Disabled => scale_alpha(base_text, 0.5),
                        _ => base_text,
                    },
                    border: iced::Border {
                        color: super::super::border_color(t),
                        width: 1.0,
                        radius: iced::border::rounded(super::super::RADIUS_BUTTON).radius,
                    },
                    shadow: Shadow::default(),
                    ..Default::default()
                }
            }
        }

        pub fn danger<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let p = t.extended_palette().danger;
                filled(p.base.color, p.strong.color, p.base.text, status)
            }
        }

        pub fn swatch<'a>(
            color: Color,
            selected: bool,
        ) -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let radius = crate::ui::dims::SWATCH_SIZE / 2.0;
                let actual_bg = match status {
                    Status::Hovered => super::lighten(color, 0.12),
                    Status::Pressed => super::darken(color, 0.15),
                    _ => color,
                };
                let border = if selected {
                    iced::Border {
                        color: t.extended_palette().background.base.text,
                        width: 2.0,
                        radius: iced::border::rounded(radius).radius,
                    }
                } else {
                    iced::Border {
                        color: Color::from_rgba(0.5, 0.5, 0.5, 0.6),
                        width: 1.0,
                        radius: iced::border::rounded(radius).radius,
                    }
                };
                let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
                let mark = if selected {
                    if luminance > 0.55 {
                        Color::from_rgb8(0x11, 0x11, 0x11)
                    } else {
                        Color::WHITE
                    }
                } else {
                    Color::TRANSPARENT
                };
                Style {
                    background: Some(Background::Color(actual_bg)),
                    text_color: mark,
                    border,
                    shadow: Shadow::default(),
                    ..Default::default()
                }
            }
        }

        pub fn sidebar_icon<'a>(active: bool) -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let accent = t.extended_palette().primary.base.color;
                if active {
                    Style {
                        background: Some(
                            Color::from_rgba(accent.r, accent.g, accent.b, 0.25).into(),
                        ),
                        text_color: accent,
                        border: iced::border::rounded(super::super::RADIUS_BUTTON),
                        ..Default::default()
                    }
                } else {
                    let text = t.extended_palette().background.base.text;
                    Style {
                        background: match status {
                            Status::Hovered | Status::Pressed => {
                                Some(super::hover_overlay(t, 0.08).into())
                            }
                            _ => None,
                        },
                        text_color: text,
                        border: iced::border::rounded(super::super::RADIUS_BUTTON),
                        ..Default::default()
                    }
                }
            }
        }

        pub fn sidebar_nav<'a>(active: bool) -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let accent = t.extended_palette().primary.base.color;
                if active {
                    Style {
                        background: Some(
                            Color::from_rgba(accent.r, accent.g, accent.b, 0.25).into(),
                        ),
                        text_color: accent,
                        border: iced::border::rounded(super::super::RADIUS_NAV),
                        ..Default::default()
                    }
                } else {
                    let text = t.extended_palette().background.base.text;
                    Style {
                        background: match status {
                            Status::Hovered | Status::Pressed => {
                                Some(super::hover_overlay(t, 0.08).into())
                            }
                            _ => None,
                        },
                        text_color: text,
                        border: iced::border::rounded(super::super::RADIUS_NAV),
                        ..Default::default()
                    }
                }
            }
        }

        pub fn window_control<'a>(is_close: bool) -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let hover = if is_close {
                    Color::from_rgba(0.961, 0.263, 0.212, 0.85)
                } else {
                    super::hover_overlay(t, 0.12)
                };
                Style {
                    background: match status {
                        Status::Hovered | Status::Pressed => Some(hover.into()),
                        _ => None,
                    },
                    text_color: t.extended_palette().background.base.text,
                    border: iced::border::rounded(0),
                    ..Default::default()
                }
            }
        }

        pub fn grouped_icon<'a>(
            trailing: bool,
            on_field: bool,
        ) -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t, status| {
                let base_text = t.extended_palette().background.base.text;
                let base_bg = if on_field {
                    t.extended_palette().background.base.color
                } else {
                    t.extended_palette().background.weak.color
                };
                let radius = if trailing {
                    iced::border::Radius::default().right(super::super::RADIUS_BUTTON)
                } else {
                    iced::border::Radius::default()
                };
                Style {
                    background: match status {
                        Status::Hovered => Some(super::lighten(base_bg, 0.08).into()),
                        Status::Pressed => Some(super::lighten(base_bg, 0.14).into()),
                        _ => Some(base_bg.into()),
                    },
                    text_color: base_text,
                    border: iced::Border {
                        radius,
                        ..Default::default()
                    },
                    shadow: Shadow::default(),
                    ..Default::default()
                }
            }
        }
    }

    pub mod input {
        use iced::widget::text_input;

        pub fn grouped(t: &iced::Theme, _status: text_input::Status) -> text_input::Style {
            let p = t.extended_palette();
            text_input::Style {
                background: iced::Background::Color(iced::Color::TRANSPARENT),
                border: iced::Border::default(),
                icon: p.background.weak.text,
                placeholder: p.secondary.base.color,
                value: p.background.base.text,
                selection: p.primary.weak.color,
            }
        }

        pub fn standard(t: &iced::Theme, status: text_input::Status) -> text_input::Style {
            let mut s = text_input::default(t, status);
            s.border.radius = super::super::RADIUS_BUTTON.into();
            if matches!(status, text_input::Status::Hovered) {
                s.border.color = t.extended_palette().primary.strong.color;
            }
            s
        }

        pub fn error(t: &iced::Theme, status: text_input::Status) -> text_input::Style {
            let mut s = text_input::default(t, status);
            s.border.radius = super::super::RADIUS_BUTTON.into();
            s.border.color = super::super::danger(t);
            if matches!(status, text_input::Status::Hovered) {
                s.border.width = 2.0;
            }
            s
        }
    }

    pub mod text_editor {
        use iced::widget::text_editor;

        pub fn standard(t: &iced::Theme, status: text_editor::Status) -> text_editor::Style {
            let mut s = text_editor::default(t, status);
            s.border.radius = super::super::RADIUS_BUTTON.into();
            if matches!(status, text_editor::Status::Hovered) {
                s.border.color = t.extended_palette().primary.strong.color;
            }
            s
        }
    }

    pub mod pick_list {
        use iced::widget::overlay::menu;
        use iced::widget::pick_list;

        pub fn standard(t: &iced::Theme, status: pick_list::Status) -> pick_list::Style {
            let mut s = pick_list::default(t, status);
            s.border.radius = super::super::RADIUS_BUTTON.into();
            s
        }

        pub fn menu(t: &iced::Theme) -> menu::Style {
            let mut s = menu::default(t);
            s.border.radius = super::super::RADIUS_BUTTON.into();
            s
        }
    }

    pub mod progress {
        use iced::{Background, Color};

        pub fn task(
            bar_color: Color,
        ) -> impl Fn(&iced::Theme) -> iced::widget::progress_bar::Style {
            move |t: &iced::Theme| iced::widget::progress_bar::Style {
                background: Background::Color(t.extended_palette().background.base.color),
                bar: Background::Color(bar_color),
                border: iced::border::rounded(super::super::RADIUS_PROGRESS),
            }
        }
    }

    pub mod scrollable {
        use iced::widget::scrollable::{self, AutoScroll, Rail, Scroller};
        use iced::{Border, Color, Shadow, Vector};
        use std::time::Instant;

        fn build(t: &iced::Theme, scroller_color: Color) -> scrollable::Style {
            let p = t.extended_palette();

            let scroller = Scroller {
                background: scroller_color.into(),
                border: Border {
                    radius: super::super::RADIUS_BUTTON.into(),
                    ..Default::default()
                },
            };

            let rail = Rail {
                background: None,
                border: Border::default(),
                scroller,
            };

            let auto_scroll = AutoScroll {
                background: p.background.base.color.into(),
                border: iced::border::rounded(u32::MAX)
                    .width(1)
                    .color(p.background.base.text.scale_alpha(0.8)),
                shadow: Shadow {
                    color: iced::Color::BLACK.scale_alpha(0.7),
                    offset: Vector::ZERO,
                    blur_radius: 2.0,
                },
                icon: p.background.base.text.scale_alpha(0.8),
            };

            scrollable::Style {
                container: iced::widget::container::Style::default(),
                vertical_rail: rail,
                horizontal_rail: rail,
                gap: None,
                auto_scroll,
            }
        }

        pub fn standard(t: &iced::Theme, status: scrollable::Status) -> scrollable::Style {
            let p = t.extended_palette();
            let scroller_visible = matches!(
                status,
                scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. }
            );
            let scroller_color = if scroller_visible {
                p.primary.base.color
            } else {
                Color::TRANSPARENT
            };
            build(t, scroller_color)
        }

        pub fn animating(
            id: iced::widget::Id,
        ) -> impl Fn(&iced::Theme, scrollable::Status) -> scrollable::Style {
            move |t, status| {
                let visible = matches!(
                    status,
                    scrollable::Status::Hovered {
                        is_vertical_scrollbar_hovered: true,
                        ..
                    } | scrollable::Status::Dragged {
                        is_vertical_scrollbar_dragged: true,
                        ..
                    }
                );
                let alpha = crate::ui::scroll_anim::tick(id.clone(), visible, Instant::now());
                let color = t.extended_palette().primary.base.color.scale_alpha(alpha);
                build(t, color)
            }
        }
    }

    pub mod text {
        pub fn secondary(t: &iced::Theme) -> iced::widget::text::Style {
            iced::widget::text::Style {
                color: Some(super::super::text_secondary(t)),
            }
        }

        pub fn tertiary(t: &iced::Theme) -> iced::widget::text::Style {
            iced::widget::text::Style {
                color: Some(super::super::text_weak(t)),
            }
        }

        pub fn error(t: &iced::Theme) -> iced::widget::text::Style {
            iced::widget::text::Style {
                color: Some(super::super::danger(t)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_from_hex_rgb_6_digits() {
        let c = color_from_hex("#5865F2").unwrap();
        assert_eq!(c.r, 0x58 as f32 / 255.0);
        assert_eq!(c.g, 0x65 as f32 / 255.0);
        assert_eq!(c.b, 0xF2 as f32 / 255.0);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn color_from_hex_rgba_8_digits() {
        let c = color_from_hex("#FF00AA80").unwrap();
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0xAA as f32 / 255.0);
        assert!((c.a - 0x80 as f32 / 255.0).abs() < f32::EPSILON);
    }

    #[test]
    fn color_from_hex_case_insensitive() {
        let a = color_from_hex("#ff00aa").unwrap();
        let b = color_from_hex("#FF00AA").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn color_from_hex_trims_whitespace() {
        let c = color_from_hex("  #ABCDEF  ").unwrap();
        assert_eq!(c.r, 0xAB as f32 / 255.0);
    }

    #[test]
    fn color_from_hex_rejects_invalid() {
        assert!(color_from_hex("5865F2").is_none());
        assert!(color_from_hex("#12345").is_none());
        assert!(color_from_hex("#1234567").is_none());
        assert!(color_from_hex("#GG0000").is_none());
    }

    #[test]
    fn color_to_hex_outputs_nine_chars() {
        let s = color_to_hex(Color::from_rgb8(0x58, 0x65, 0xF2));
        assert_eq!(s, "#5865F2FF");
    }

    #[test]
    fn color_round_trip_rgb() {
        let c = Color::from_rgb8(0x12, 0x34, 0x56);
        let s = color_to_hex(c);
        assert_eq!(color_from_hex(&s).unwrap(), c);
    }

    #[test]
    fn color_round_trip_rgba() {
        let c = Color::from_rgba8(0x12, 0x34, 0x56, 0.5);
        let s = color_to_hex(c);
        let parsed = color_from_hex(&s).unwrap();
        assert_eq!(parsed.r, c.r);
        assert_eq!(parsed.g, c.g);
        assert_eq!(parsed.b, c.b);
        assert!((parsed.a - c.a).abs() < 0.005);
    }
}
