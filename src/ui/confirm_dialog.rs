use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{ConfirmAction, Message};
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

    let title = text(fluent.get(title_key)).size(20);
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

            row![]
                .push(cancel_btn)
                .push(remove_record_btn)
                .push(delete_files_btn)
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

            row![]
                .push(cancel_btn)
                .push(remove_all_records_btn)
                .push(delete_all_files_btn)
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
        }
        ConfirmAction::ClearCompleted => {
            let confirm_btn = button(text(fluent.get(Tr::Confirm)).size(14))
                .on_press(Message::ClearCompleted)
                .padding([10, 22])
                .style(theme::style::button::danger());

            row![]
                .push(cancel_btn)
                .push(confirm_btn)
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

            row![]
                .push(cancel_btn)
                .push(discard_btn)
                .push(apply_btn)
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
        }
    };

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
