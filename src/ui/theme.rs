use serde::{Deserialize, Serialize};

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

pub fn resolve_dark(mode: ThemeMode, system_dark: Option<bool>) -> bool {
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

pub fn palette(dark: bool) -> (Color, Color) {
    if dark {
        (BG_PRIMARY, TEXT_PRIMARY)
    } else {
        (BG_PRIMARY_LIGHT, TEXT_PRIMARY_LIGHT)
    }
}

pub fn build(dark: bool) -> Theme {
    let (background, text) = palette(dark);
    Theme::custom(
        if dark {
            "Remotrix Dark".to_string()
        } else {
            "Remotrix Light".to_string()
        },
        Palette {
            background,
            text,
            primary: ACCENT,
            success: PROGRESS,
            warning: PAUSED,
            danger: ERROR,
        },
    )
}
