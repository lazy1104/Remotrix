use iced::widget::{button, checkbox, column, row, text};
use iced::{Alignment, Element};

use crate::i18n::{Fluent, Tr};
use crate::message::{CloseDialogChoice, Message, WindowMsg};
use crate::ui::components::dialog::Dialog;
use crate::ui::components::expand::expand_pinned;
use crate::ui::dims::*;
use crate::ui::theme;

pub fn view<'a>(
    fluent: &'a Fluent,
    _theme: &iced::Theme,
    tray_available: bool,
    close_to_tray: bool,
    progress: f32,
) -> Element<'a, Message> {
    let body_text = text(fluent.get(Tr::ConfirmCloseBody))
        .size(FONT_MEDIUM)
        .style(theme::style::text::secondary);

    let mut body = column![body_text].spacing(SPACE_LG);
    if tray_available {
        body = body.push(
            checkbox(close_to_tray)
                .label(fluent.get(Tr::ConfirmCloseTrayPref))
                .on_toggle(|v| Message::Window(WindowMsg::CloseDialogTrayPrefChanged(v)))
                .text_size(FONT_MEDIUM),
        );
    }

    let close_btn = button(text(fluent.get(Tr::CloseAction)).size(FONT_BODY))
        .on_press(Message::Window(WindowMsg::CloseDialog(
            CloseDialogChoice::Close,
        )))
        .padding(PADDING_BUTTON_LG)
        .style(theme::style::button::danger());

    let cancel_btn = button(text(fluent.get(Tr::Cancel)).size(FONT_BODY))
        .on_press(Message::Window(WindowMsg::CloseDialog(
            CloseDialogChoice::Cancel,
        )))
        .padding(PADDING_BUTTON_LG)
        .style(theme::style::button::secondary());

    let tray_action = if tray_available {
        Message::Window(WindowMsg::HideToTray)
    } else {
        Message::Window(WindowMsg::CloseDialog(CloseDialogChoice::Close))
    };
    let tray_btn = button(text(fluent.get(Tr::TrayAction)).size(FONT_BODY))
        .on_press(tray_action)
        .padding(PADDING_BUTTON_LG)
        .style(theme::style::button::secondary());

    let buttons = row![]
        .push(cancel_btn)
        .push(tray_btn)
        .push(close_btn)
        .spacing(SPACE_XL)
        .align_y(Alignment::Center);

    expand_pinned(
        Dialog::new()
            .title(fluent.get(Tr::ConfirmCloseTitle))
            .with_close(Message::Window(WindowMsg::CloseDialog(
                CloseDialogChoice::Cancel,
            )))
            .body(body)
            .footer(buttons)
            .build(),
        progress,
    )
}
