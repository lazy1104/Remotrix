use serde::{Deserialize, Serialize};

use iced::theme::Palette;
use iced::{Color, Theme};

use crate::ui::dims;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeMode {
    Dark,
    Light,
    #[default]
    System,
}

pub fn detect_dark() -> bool {
    matches!(dark_light::detect(), dark_light::Mode::Dark)
}

pub fn resolve_mode(mode: ThemeMode, system_dark: Option<bool>) -> bool {
    match mode {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        ThemeMode::System => system_dark.unwrap_or_else(detect_dark),
    }
}

pub const OVERLAY: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.55,
};

pub const RADIUS_CARD: f32 = 8.0;
pub const RADIUS_BUTTON: f32 = 6.0;
pub const RADIUS_PILL: f32 = 40.0;
pub const RADIUS_PROGRESS: f32 = 4.0;
pub const RADIUS_NAV: f32 = 20.0;

pub const INPUT_PADDING: iced::Padding = iced::Padding::new(8.0);
pub const INPUT_PADDING_GROUPED: iced::Padding = iced::Padding {
    top: 0.0,
    right: 10.0,
    bottom: 0.0,
    left: 10.0,
};

pub fn input_layout<'a, Message: Clone>(
    input: iced::widget::TextInput<'a, Message>,
) -> iced::widget::TextInput<'a, Message> {
    input.padding(INPUT_PADDING).size(dims::FONT_MEDIUM)
}

pub fn grouped_input_layout<'a, Message: Clone>(
    input: iced::widget::TextInput<'a, Message>,
) -> iced::widget::TextInput<'a, Message> {
    input.padding(INPUT_PADDING_GROUPED).size(dims::FONT_MEDIUM)
}

pub fn editor_layout<'a, H, Message>(
    editor: iced::widget::TextEditor<'a, H, Message>,
) -> iced::widget::TextEditor<'a, H, Message>
where
    H: iced::advanced::text::Highlighter,
{
    editor.padding(INPUT_PADDING).size(dims::FONT_MEDIUM)
}

pub const DEFAULT_THEME_COLOR: Color = Color::from_rgb8(0x58, 0x65, 0xF2);

pub const SURFACE_TONE_DARK_S: f32 = 0.12;
pub const SURFACE_TONE_DARK_L: f32 = 0.18;
pub const SURFACE_TONE_LIGHT_S: f32 = 0.10;
pub const SURFACE_TONE_LIGHT_L: f32 = 0.96;

fn rgb_to_hsl(c: Color) -> (f32, f32, f32) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let (h, s) = if d == 0.0 {
        (0.0, 0.0)
    } else {
        let s = d / (1.0 - (2.0 * l - 1.0).abs());
        let h = if max == c.r {
            (c.g - c.b) / d % 6.0
        } else if max == c.g {
            (c.b - c.r) / d + 2.0
        } else {
            (c.r - c.g) / d + 4.0
        };
        (h / 6.0, s)
    };
    (h.rem_euclid(1.0), s, l)
}

fn hue_to_rgb(p: f32, q: f32, t: f32) -> f32 {
    let t = t.rem_euclid(1.0);
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color {
    if s == 0.0 {
        return Color {
            r: l,
            g: l,
            b: l,
            a: 1.0,
        };
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    Color {
        r: hue_to_rgb(p, q, h + 1.0 / 3.0),
        g: hue_to_rgb(p, q, h),
        b: hue_to_rgb(p, q, h - 1.0 / 3.0),
        a: 1.0,
    }
}

pub fn surface_from_seed(seed: Color, dark: bool) -> Color {
    let (h, _, _) = rgb_to_hsl(seed);
    if dark {
        hsl_to_rgb(h, SURFACE_TONE_DARK_S, SURFACE_TONE_DARK_L)
    } else {
        hsl_to_rgb(h, SURFACE_TONE_LIGHT_S, SURFACE_TONE_LIGHT_L)
    }
}

pub fn build_iced(color: Color, dark: bool) -> iced::Theme {
    let palette = if dark {
        Palette {
            background: surface_from_seed(color, true),
            primary: color,
            ..Palette::DARK
        }
    } else {
        Palette {
            background: surface_from_seed(color, false),
            primary: color,
            ..Palette::LIGHT
        }
    };
    iced::Theme::custom("remotrix", palette)
}

pub fn color_to_hex(c: Color) -> String {
    let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", to(c.r), to(c.g), to(c.b))
}

pub fn color_from_hex(s: &str) -> Option<Color> {
    let h = s.trim().strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some(Color::from_rgb8(r, g, b))
}

pub fn accent_color(hex: &str) -> Color {
    color_from_hex(hex).unwrap_or(DEFAULT_THEME_COLOR)
}

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

pub fn candidate_colors() -> &'static [(Color, &'static str)] {
    CANDIDATE_COLORS
}

pub fn accent(t: &Theme) -> Color {
    t.extended_palette().primary.base.color
}

pub fn success(t: &Theme) -> Color {
    t.extended_palette().success.base.color
}

pub fn warning(t: &Theme) -> Color {
    t.extended_palette().warning.base.color
}

pub fn danger(t: &Theme) -> Color {
    t.extended_palette().danger.base.color
}

pub fn primary(t: &Theme) -> Color {
    t.extended_palette().primary.base.color
}

pub fn primary_weak(t: &Theme) -> Color {
    t.extended_palette().primary.weak.color
}

pub fn text_secondary(t: &Theme) -> Color {
    t.extended_palette().background.base.text
}

pub fn border_color(t: &Theme) -> Color {
    t.extended_palette().background.strong.color
}

pub mod style {
    use iced::{Color, Shadow, Vector};

    pub fn base_background(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(t.extended_palette().background.base.color.into()),
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
            shadow: card_shadow(),
            ..Default::default()
        }
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
        move |t| iced::widget::container::Style {
            background: Some(t.extended_palette().background.base.color.into()),
            border: iced::Border {
                color: if focused || hovered {
                    t.extended_palette().primary.base.color
                } else {
                    super::border_color(t)
                },
                width: 1.0,
                radius: super::RADIUS_BUTTON.into(),
            },
            ..Default::default()
        }
    }

    pub fn overlay(_t: &iced::Theme) -> iced::widget::container::Style {
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
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.12),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 6.0,
        }
    }

    pub mod button {
        use iced::widget::button::{Status, Style};
        use iced::Color;
        use iced::{Background, Shadow, Vector};

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
                        Status::Hovered => Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into()),
                        Status::Pressed => Some(Color::from_rgba(1.0, 1.0, 1.0, 0.14).into()),
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
                        border: iced::border::rounded(super::super::RADIUS_BUTTON),
                        shadow: Shadow::default(),
                        ..Default::default()
                    }
                } else {
                    Style {
                        background: match status {
                            Status::Hovered => Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into()),
                            Status::Pressed => Some(Color::from_rgba(1.0, 1.0, 1.0, 0.14).into()),
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
                    shadow: Shadow::default(),
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
                        Status::Hovered => Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into()),
                        Status::Pressed => Some(Color::from_rgba(1.0, 1.0, 1.0, 0.14).into()),
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
                                Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
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
                                Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
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
                    Color::from_rgba(1.0, 1.0, 1.0, 0.12)
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

        pub fn grouped_icon<'a>(trailing: bool) -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t, status| {
                let base_text = t.extended_palette().background.base.text;
                let weak_bg = t.extended_palette().background.weak.color;
                let radius = if trailing {
                    iced::border::Radius::default().right(super::super::RADIUS_BUTTON)
                } else {
                    iced::border::Radius::default()
                };
                Style {
                    background: match status {
                        Status::Hovered => Some(super::lighten(weak_bg, 0.08).into()),
                        Status::Pressed => Some(super::lighten(weak_bg, 0.14).into()),
                        _ => Some(weak_bg.into()),
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

        pub fn new_download<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let pal = t.extended_palette().primary;
                let accent = pal.base.color;
                let bg_text = t.extended_palette().background.base.text;
                let actual_bg = match status {
                    Status::Hovered => pal.strong.color,
                    Status::Pressed => super::darken(accent, 0.15),
                    _ => accent,
                };
                let (shadow, alpha) = match status {
                    Status::Pressed => (
                        Shadow {
                            color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                            offset: Vector::new(0.0, 0.0),
                            blur_radius: 1.0,
                        },
                        1.0,
                    ),
                    Status::Disabled => (Shadow::default(), 0.5),
                    _ => (
                        Shadow {
                            color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                            offset: Vector::new(0.0, 3.0),
                            blur_radius: 6.0,
                        },
                        1.0,
                    ),
                };
                Style {
                    background: Some(Background::Color(Color {
                        a: actual_bg.a * alpha,
                        ..actual_bg
                    })),
                    text_color: scale_alpha(bg_text, alpha),
                    border: iced::border::rounded(super::super::RADIUS_PILL),
                    shadow,
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
    }

    pub mod text_editor {
        use iced::widget::text_editor;

        pub fn standard(t: &iced::Theme, status: text_editor::Status) -> text_editor::Style {
            let mut s = text_editor::default(t, status);
            s.border.radius = super::super::RADIUS_BUTTON.into();
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
        use iced::{Border, Shadow, Vector};

        pub fn standard(t: &iced::Theme, _status: scrollable::Status) -> scrollable::Style {
            let p = t.extended_palette();

            let scroller = Scroller {
                background: p.primary.base.color.into(),
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
    }

    pub mod text {
        pub fn secondary(t: &iced::Theme) -> iced::widget::text::Style {
            iced::widget::text::Style {
                color: Some(super::super::text_secondary(t)),
            }
        }
    }
}
