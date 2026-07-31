use iced::widget::{button, column, row, text};
use iced::{Alignment, Element};

use crate::i18n::{Fluent, Tr};
use crate::message::{CloseDialogChoice, Message};
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::theme;

pub fn view<'a>(fluent: &'a Fluent, _theme: &iced::Theme) -> Element<'a, Message> {
    let body = text(fluent.get(Tr::ConfirmCloseBody))
        .size(13)
        .style(theme::style::text::secondary);

    let coming_soon = text(fluent.get(Tr::TrayComingSoon))
        .size(11)
        .style(theme::style::text::secondary);

    let close_btn = button(text(fluent.get(Tr::CloseAction)).size(14))
        .on_press(Message::CloseDialog(CloseDialogChoice::Close))
        .padding([10, 22])
        .style(theme::style::button::danger());

    let cancel_btn = button(text(fluent.get(Tr::Cancel)).size(14))
        .on_press(Message::CloseDialog(CloseDialogChoice::Cancel))
        .padding([10, 22])
        .style(theme::style::button::secondary());

    let tray_btn = button(
        column![]
            .push(
                text(fluent.get(Tr::TrayAction))
                    .size(14)
                    .style(theme::style::text::secondary),
            )
            .push(coming_soon)
            .spacing(2)
            .align_x(Alignment::Center),
    )
    .padding([8, 22])
    .style(theme::style::button::text());

    let buttons = row![]
        .push(cancel_btn)
        .push(tray_btn)
        .push(close_btn)
        .spacing(10)
        .align_y(Alignment::Center);

    overlay(
        Dialog::new()
            .title(fluent.get(Tr::ConfirmCloseTitle))
            .with_close(Message::CloseDialog(CloseDialogChoice::Cancel))
            .body(body)
            .footer(buttons)
            .build(),
    )
}
