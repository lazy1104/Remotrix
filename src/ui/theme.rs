use serde::{Deserialize, Serialize};

use iced::theme::palette::{self, Pair};
use iced::theme::Palette;
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

pub const BG_PRIMARY: Color = Color::from_rgb(0.118, 0.118, 0.176);
pub const BG_SIDEBAR: Color = Color::from_rgb(0.094, 0.094, 0.149);
pub const BG_CARD: Color = Color::from_rgb(0.145, 0.145, 0.251);
pub const ACCENT: Color = Color::from_rgb(0.290, 0.565, 0.851);
pub const PROGRESS: Color = Color::from_rgb(0.298, 0.686, 0.314);
pub const SPEED: Color = Color::from_rgb(0.549, 0.757, 0.290);
pub const ERROR: Color = Color::from_rgb(0.961, 0.263, 0.212);
pub const PAUSED: Color = Color::from_rgb(1.0, 0.600, 0.0);
pub const TEXT_PRIMARY: Color = Color::from_rgb(1.0, 1.0, 1.0);
pub const TEXT_SECONDARY: Color = Color::from_rgb(0.627, 0.627, 0.690);
pub const BORDER: Color = Color::from_rgb(0.176, 0.176, 0.267);
pub const TITLE_BAR: Color = Color::from_rgb(0.078, 0.078, 0.125);
pub const OVERLAY: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.55,
};

pub const BG_PRIMARY_LIGHT: Color = Color::from_rgb(0.961, 0.961, 0.969);
pub const BG_SIDEBAR_LIGHT: Color = Color::from_rgb(0.910, 0.910, 0.929);
pub const BG_CARD_LIGHT: Color = Color::from_rgb(1.0, 1.0, 1.0);
pub const TEXT_PRIMARY_LIGHT: Color = Color::from_rgb(0.118, 0.118, 0.180);
pub const TEXT_SECONDARY_LIGHT: Color = Color::from_rgb(0.420, 0.420, 0.470);
pub const BORDER_LIGHT: Color = Color::from_rgb(0.804, 0.804, 0.851);
pub const TITLE_BAR_LIGHT: Color = Color::from_rgb(0.941, 0.941, 0.961);

fn seed_palette(dark: bool) -> Palette {
    if dark {
        Palette {
            background: BG_PRIMARY,
            text: TEXT_PRIMARY,
            primary: ACCENT,
            success: PROGRESS,
            warning: PAUSED,
            danger: ERROR,
        }
    } else {
        Palette {
            background: BG_PRIMARY_LIGHT,
            text: TEXT_PRIMARY_LIGHT,
            primary: ACCENT,
            success: PROGRESS,
            warning: PAUSED,
            danger: ERROR,
        }
    }
}

fn dark_background() -> palette::Background {
    palette::Background {
        base: Pair {
            color: BG_PRIMARY,
            text: TEXT_PRIMARY,
        },
        weakest: Pair {
            color: Color::from_rgb(0.20, 0.20, 0.28),
            text: TEXT_PRIMARY,
        },
        weaker: Pair {
            color: Color::from_rgb(0.17, 0.17, 0.26),
            text: TEXT_PRIMARY,
        },
        weak: Pair {
            color: BG_CARD,
            text: TEXT_PRIMARY,
        },
        neutral: Pair {
            color: Color::from_rgb(0.13, 0.13, 0.21),
            text: TEXT_PRIMARY,
        },
        strong: Pair {
            color: BG_SIDEBAR,
            text: TEXT_PRIMARY,
        },
        stronger: Pair {
            color: TITLE_BAR,
            text: TEXT_PRIMARY,
        },
        strongest: Pair {
            color: Color::from_rgb(0.05, 0.05, 0.10),
            text: TEXT_PRIMARY,
        },
    }
}

fn light_background() -> palette::Background {
    palette::Background {
        base: Pair {
            color: BG_PRIMARY_LIGHT,
            text: TEXT_PRIMARY_LIGHT,
        },
        weakest: Pair {
            color: Color::from_rgb(1.0, 1.0, 1.0),
            text: TEXT_PRIMARY_LIGHT,
        },
        weaker: Pair {
            color: Color::from_rgb(0.98, 0.98, 0.98),
            text: TEXT_PRIMARY_LIGHT,
        },
        weak: Pair {
            color: BG_CARD_LIGHT,
            text: TEXT_PRIMARY_LIGHT,
        },
        neutral: Pair {
            color: Color::from_rgb(0.94, 0.94, 0.95),
            text: TEXT_PRIMARY_LIGHT,
        },
        strong: Pair {
            color: BG_SIDEBAR_LIGHT,
            text: TEXT_PRIMARY_LIGHT,
        },
        stronger: Pair {
            color: TITLE_BAR_LIGHT,
            text: TEXT_PRIMARY_LIGHT,
        },
        strongest: Pair {
            color: Color::from_rgb(0.88, 0.88, 0.90),
            text: TEXT_PRIMARY_LIGHT,
        },
    }
}

pub fn build_iced(dark: bool) -> Theme {
    Theme::custom_with_fn(
        if dark {
            "Remotrix Dark"
        } else {
            "Remotrix Light"
        },
        seed_palette(dark),
        move |_| {
            let seed = seed_palette(dark);
            let mut extended = palette::Extended::generate(seed);
            extended.background = if dark {
                dark_background()
            } else {
                light_background()
            };
            extended.is_dark = dark;
            extended
        },
    )
}

pub fn text_secondary(t: &Theme) -> Color {
    if t.extended_palette().is_dark {
        TEXT_SECONDARY
    } else {
        TEXT_SECONDARY_LIGHT
    }
}

pub fn border_color(t: &Theme) -> Color {
    if t.extended_palette().is_dark {
        BORDER
    } else {
        BORDER_LIGHT
    }
}

pub mod style {
    use iced::Color;

    use super::ACCENT;

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

    pub fn active_filter(_t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(Color::from_rgba(0.29, 0.565, 0.851, 0.18).into()),
            text_color: Some(ACCENT),
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
                if active {
                    iced::widget::button::Style {
                        background: Some(Color::from_rgba(0.29, 0.565, 0.851, 0.25).into()),
                        text_color: super::super::ACCENT,
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
