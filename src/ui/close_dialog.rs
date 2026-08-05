use iced::widget::{button, row, text};
use iced::{Alignment, Element};

use crate::i18n::{Fluent, Tr};
use crate::message::{CloseDialogChoice, Message, WindowMsg};
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::dims::*;
use crate::ui::theme;

pub fn view<'a>(
    fluent: &'a Fluent,
    _theme: &iced::Theme,
    tray_available: bool,
) -> Element<'a, Message> {
    let body = text(fluent.get(Tr::ConfirmCloseBody))
        .size(FONT_MEDIUM)
        .style(theme::style::text::secondary);

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
        .padding(PADDING_TRAY)
        .style(theme::style::button::text());

    let buttons = row![]
        .push(cancel_btn)
        .push(tray_btn)
        .push(close_btn)
        .spacing(SPACE_XL)
        .align_y(Alignment::Center);

    overlay(
        Dialog::new()
            .title(fluent.get(Tr::ConfirmCloseTitle))
            .with_close(Message::Window(WindowMsg::CloseDialog(
                CloseDialogChoice::Cancel,
            )))
            .body(body)
            .footer(buttons)
            .build(),
    )
}
