use iced::widget::{button, row, text};
use iced::{Alignment, Element};

use crate::i18n::{Fluent, Tr};
use crate::message::{ConfirmAction, Message};
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::theme;

pub fn view<'a>(
    fluent: &'a Fluent,
    _theme: &iced::Theme,
    action: &'a ConfirmAction,
) -> Element<'a, Message> {
    let (title_key, body_key) = match action {
        ConfirmAction::DeleteTask(_) => (Tr::ConfirmDeleteTitle, Tr::ConfirmDeleteBody),
        ConfirmAction::DeleteAll => (Tr::ConfirmDeleteAllTitle, Tr::ConfirmDeleteAllBody),
        ConfirmAction::ClearCompleted => (Tr::ConfirmClearTitle, Tr::ConfirmClearBody),
        ConfirmAction::LeaveSettings { .. } => {
            (Tr::ConfirmUnappliedTitle, Tr::ConfirmUnappliedBody)
        }
    };

    let body = text(fluent.get(body_key))
        .size(13)
        .style(theme::style::text::secondary);

    let cancel_btn = button(text(fluent.get(Tr::Cancel)).size(14))
        .on_press(Message::ConfirmCancel)
        .padding([10, 22])
        .style(theme::style::button::secondary());

    let buttons: Element<'a, Message> = match action {
        ConfirmAction::DeleteTask(gid) => {
            let remove_record_btn = button(text(fluent.get(Tr::RemoveRecord)).size(14))
                .on_press(Message::RemoveTask(gid.clone()))
                .padding([10, 22])
                .style(theme::style::button::secondary());
            let delete_files_btn = button(text(fluent.get(Tr::DeleteFiles)).size(14))
                .on_press(Message::DeleteTask(gid.clone()))
                .padding([10, 22])
                .style(theme::style::button::danger());

            row![cancel_btn, remove_record_btn, delete_files_btn]
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
        }
        ConfirmAction::DeleteAll => {
            let remove_all_records_btn = button(text(fluent.get(Tr::RemoveAllRecords)).size(14))
                .on_press(Message::RemoveAllRecords)
                .padding([10, 22])
                .style(theme::style::button::secondary());
            let delete_all_files_btn = button(text(fluent.get(Tr::DeleteAllFiles)).size(14))
                .on_press(Message::DeleteAll)
                .padding([10, 22])
                .style(theme::style::button::danger());

            row![cancel_btn, remove_all_records_btn, delete_all_files_btn]
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
        }
        ConfirmAction::ClearCompleted => {
            let confirm_btn = button(text(fluent.get(Tr::Confirm)).size(14))
                .on_press(Message::ClearCompleted)
                .padding([10, 22])
                .style(theme::style::button::danger());

            row![cancel_btn, confirm_btn]
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
        }
        ConfirmAction::LeaveSettings { .. } => {
            let discard_btn = button(text(fluent.get(Tr::Discard)).size(14))
                .on_press(Message::DiscardAndLeaveSettings)
                .padding([10, 22])
                .style(theme::style::button::danger());
            let apply_btn = button(text(fluent.get(Tr::Apply)).size(14))
                .on_press(Message::ApplyAndLeaveSettings)
                .padding([10, 22])
                .style(theme::style::button::primary());

            row![cancel_btn, discard_btn, apply_btn]
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
        }
    };

    overlay(
        Dialog::new()
            .title(fluent.get(title_key))
            .with_close(Message::ConfirmCancel)
            .body(body)
            .footer(buttons)
            .build(),
    )
}
