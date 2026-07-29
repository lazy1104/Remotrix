use std::sync::Arc;

use serde::{Deserialize, Serialize};

use iced::{Color, Theme};

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

pub fn build_iced(theme_id: &str) -> iced::Theme {
    let opaline = opaline::builtins::load_by_name(theme_id).unwrap_or_default();
    let custom = opaline::adapters::iced::to_iced_custom(&opaline);
    iced::Theme::Custom(Arc::new(custom))
}

fn themes_for_variant(variant: opaline::schema::ThemeVariant) -> Vec<(String, String)> {
    let mut v: Vec<_> = opaline::builtins::list_available_themes()
        .into_iter()
        .filter(|i| i.variant == variant)
        .map(|i| (i.name, i.display_name))
        .collect();
    v.sort_by_key(|a| a.1.to_lowercase());
    v
}

pub fn light_themes() -> Vec<(String, String)> {
    themes_for_variant(opaline::schema::ThemeVariant::Light)
}

pub fn dark_themes() -> Vec<(String, String)> {
    themes_for_variant(opaline::schema::ThemeVariant::Dark)
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

    pub fn overlay(_t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(super::OVERLAY.into()),
            ..Default::default()
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
                let p = t.extended_palette().secondary;
                filled(p.base.color, p.strong.color, p.base.text, status)
            }
        }

        pub fn danger<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
            move |t: &iced::Theme, status: Status| -> Style {
                let p = t.extended_palette().danger;
                filled(p.base.color, p.strong.color, p.base.text, status)
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

    pub mod text {
        pub fn secondary(t: &iced::Theme) -> iced::widget::text::Style {
            iced::widget::text::Style {
                color: Some(super::super::text_secondary(t)),
            }
        }
    }
}
