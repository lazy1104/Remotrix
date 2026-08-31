//! Settings card controlling the post-task-completion shutdown behaviour.
//!
//! Two toggles: shut down after all tasks complete, or shut down after a
//! user-configured number of minutes. Used by the Download settings
//! category.

use iced::widget::{column, container, row, text, toggler};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, ShutdownMsg};
use crate::shutdown::ShutdownControl;
use crate::ui::components::number_stepper::number_stepper;
use crate::ui::dims::*;
use crate::ui::theme;

const TOGGLE_W: f32 = 50.0;

/// Build the shutdown-card element for the settings page.
pub fn view<'a>(
    fluent: &'a Fluent,
    _theme: &'a iced::Theme,
    ctrl: &ShutdownControl,
) -> Element<'a, Message> {
    let labels_col = column![
        text(fluent.get(Tr::ShutdownAfterComplete)).size(FONT_MEDIUM),
        text(fluent.get(Tr::ShutdownTimer)).size(FONT_MEDIUM),
    ]
    .spacing(SPACE_SM);

    let switches_col = column![
        toggler(ctrl.after_complete)
            .on_toggle(|v| Message::Shutdown(ShutdownMsg::SetAfterComplete(v)))
            .width(Length::Fixed(TOGGLE_W)),
        toggler(ctrl.timer_enabled)
            .on_toggle(|v| Message::Shutdown(ShutdownMsg::SetTimerEnabled(v)))
            .width(Length::Fixed(TOGGLE_W)),
    ]
    .spacing(SPACE_SM);

    let switches_row = row![labels_col, switches_col]
        .spacing(SPACE_LG)
        .align_y(Alignment::Center);

    let mut col = column![]
        .spacing(SPACE_MD)
        .push(text(fluent.get(Tr::Shutdown)).size(FONT_TITLE))
        .push(switches_row);

    if ctrl.timer_enabled {
        let stepper = number_stepper(
            ctrl.timer_minutes,
            1..=720,
            1,
            |n| Message::Shutdown(ShutdownMsg::SetTimerMinutes(n)),
            Length::Fill,
        );

        let hint = if let Some(deadline) = ctrl.timer_deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            text(format!("{}s", remaining.as_secs()))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary)
        } else {
            text(fluent.get(Tr::ShutdownTimerHint))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary)
        };

        col = col
            .push(text(fluent.get(Tr::ShutdownTimerMinutes)).size(FONT_MEDIUM))
            .push(stepper)
            .push(hint);
    }

    container(col)
        .padding(PADDING_CARD)
        .width(Length::Shrink)
        .style(theme::style::subtle)
        .into()
}
