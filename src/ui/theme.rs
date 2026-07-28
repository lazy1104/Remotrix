use std::sync::Arc;

use serde::{Deserialize, Serialize};

use iced::Color;
use iced::Theme;

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
    use iced::Color;

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
            border: iced::border::rounded(8),
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
            border: iced::border::rounded(6),
            ..Default::default()
        }
    }

    pub mod button {
        use iced::Color;

        pub fn sidebar_icon<'a>(
            active: bool,
        ) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'a
        {
            move |t: &iced::Theme,
                  status: iced::widget::button::Status|
                  -> iced::widget::button::Style {
                let accent = t.extended_palette().primary.base.color;
                if active {
                    iced::widget::button::Style {
                        background: Some(
                            Color::from_rgba(accent.r, accent.g, accent.b, 0.25).into(),
                        ),
                        text_color: accent,
                        border: iced::border::rounded(6),
                        ..Default::default()
                    }
                } else {
                    let text = t.extended_palette().background.base.text;
                    iced::widget::button::Style {
                        background: match status {
                            iced::widget::button::Status::Hovered
                            | iced::widget::button::Status::Pressed => {
                                Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                            }
                            _ => None,
                        },
                        text_color: text,
                        border: iced::border::rounded(6),
                        ..Default::default()
                    }
                }
            }
        }

        pub fn window_control<'a>(
            is_close: bool,
        ) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'a
        {
            move |t: &iced::Theme,
                  status: iced::widget::button::Status|
                  -> iced::widget::button::Style {
                let hover = if is_close {
                    Color::from_rgba(0.961, 0.263, 0.212, 0.85)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.12)
                };
                iced::widget::button::Style {
                    background: match status {
                        iced::widget::button::Status::Hovered
                        | iced::widget::button::Status::Pressed => Some(hover.into()),
                        _ => None,
                    },
                    text_color: t.extended_palette().background.base.text,
                    border: iced::border::rounded(0),
                    ..Default::default()
                }
            }
        }

        pub fn new_download<'a>(
        ) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'a
        {
            move |t: &iced::Theme,
                  status: iced::widget::button::Status|
                  -> iced::widget::button::Style {
                let accent = t.extended_palette().primary.base.color;
                let background = match status {
                    iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed => {
                        Color::from_rgba(accent.r * 1.1, accent.g * 1.1, accent.b * 1.1, 1.0)
                    }
                    _ => accent,
                };
                iced::widget::button::Style {
                    background: Some(background.into()),
                    text_color: t.extended_palette().background.base.text,
                    border: iced::border::rounded(40),
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
                border: iced::border::rounded(4),
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
