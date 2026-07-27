use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{CloseDialogChoice, Message};
use crate::ui::theme;

pub fn view<'a>(fluent: &'a Fluent, _theme: &iced::Theme) -> Element<'a, Message> {
    let title = text(fluent.get(Tr::ConfirmCloseTitle)).size(20);
    let body = text(fluent.get(Tr::ConfirmCloseBody))
        .size(13)
        .style(theme::style::text::secondary);

    let coming_soon = text(fluent.get(Tr::TrayComingSoon))
        .size(11)
        .style(theme::style::text::secondary);

    let close_btn = button(text(fluent.get(Tr::CloseAction)).size(14))
        .on_press(Message::CloseDialog(CloseDialogChoice::Close))
        .padding([10, 22])
        .style(button::danger);

    let cancel_btn = button(text(fluent.get(Tr::Cancel)).size(14))
        .on_press(Message::CloseDialog(CloseDialogChoice::Cancel))
        .padding([10, 22])
        .style(button::secondary);

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
    .style(button::text);

    let buttons = row![]
        .push(cancel_btn)
        .push(tray_btn)
        .push(close_btn)
        .spacing(10)
        .align_y(Alignment::Center);

    let panel = container(
        column![]
            .spacing(16)
            .push(title)
            .push(body)
            .push(iced::widget::Space::new().height(Length::Fixed(4.0)))
            .push(buttons),
    )
    .width(Length::Fixed(420.0))
    .padding(28)
    .style(theme::style::card);

    container(panel)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::style::overlay)
        .into()
}
