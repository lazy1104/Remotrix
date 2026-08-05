use iced::widget::{button, row, text};
use iced::{Alignment, Element};

use crate::i18n::{Fluent, Tr};
use crate::message::{ConfirmAction, DialogMsg, EngineMsg, Message, SettingsMsg, TaskMsg};
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::dims::*;
use crate::ui::theme;

pub fn view<'a>(
    fluent: &'a Fluent,
    _theme: &iced::Theme,
    action: &'a ConfirmAction,
) -> Element<'a, Message> {
    let (title_key, body_key) = match action {
        ConfirmAction::DeleteTask(_) => (Tr::ConfirmDeleteTitle, Tr::ConfirmDeleteBody),
        ConfirmAction::RemoveMissingFileTask(_) => {
            (Tr::ConfirmMissingFileTitle, Tr::ConfirmMissingFileBody)
        }
        ConfirmAction::DeleteAll => (Tr::ConfirmDeleteAllTitle, Tr::ConfirmDeleteAllBody),
        ConfirmAction::ClearCompleted => (Tr::ConfirmClearTitle, Tr::ConfirmClearBody),
        ConfirmAction::LeaveSettings { .. } => {
            (Tr::ConfirmUnappliedTitle, Tr::ConfirmUnappliedBody)
        }
        ConfirmAction::UnsavedOnClose => (Tr::ConfirmUnappliedTitle, Tr::ConfirmUnappliedBody),
        ConfirmAction::RestartEngine { has_active } => {
            if *has_active {
                (
                    Tr::ConfirmRestartEngineActiveTitle,
                    Tr::ConfirmRestartEngineActiveBody,
                )
            } else {
                (Tr::ConfirmRestartEngineTitle, Tr::ConfirmRestartEngineBody)
            }
        }
    };

    let body = text(fluent.get(body_key))
        .size(FONT_MEDIUM)
        .style(theme::style::text::secondary);

    let cancel_btn = button(text(fluent.get(Tr::Cancel)).size(FONT_BODY))
        .on_press(Message::Dialog(DialogMsg::ConfirmCancel))
        .padding(PADDING_BUTTON_LG)
        .style(theme::style::button::secondary());

    let buttons: Element<'a, Message> = match action {
        ConfirmAction::DeleteTask(gid) => {
            let remove_record_btn = button(text(fluent.get(Tr::RemoveRecord)).size(FONT_BODY))
                .on_press(Message::Task(TaskMsg::RemoveTask(gid.clone())))
                .padding(PADDING_BUTTON_LG)
                .style(theme::style::button::secondary());
            let delete_files_btn = button(text(fluent.get(Tr::DeleteFiles)).size(FONT_BODY))
                .on_press(Message::Task(TaskMsg::DeleteTask(gid.clone())))
                .padding(PADDING_BUTTON_LG)
                .style(theme::style::button::danger());

            row![cancel_btn, remove_record_btn, delete_files_btn]
                .spacing(SPACE_XL)
                .align_y(Alignment::Center)
                .into()
        }
        ConfirmAction::RemoveMissingFileTask(gid) => {
            let remove_btn = button(text(fluent.get(Tr::RemoveRecord)).size(FONT_BODY))
                .on_press(Message::Task(TaskMsg::RemoveTask(gid.clone())))
                .padding(PADDING_BUTTON_LG)
                .style(theme::style::button::danger());

            row![cancel_btn, remove_btn]
                .spacing(SPACE_XL)
                .align_y(Alignment::Center)
                .into()
        }
        ConfirmAction::DeleteAll => {
            let remove_all_records_btn =
                button(text(fluent.get(Tr::RemoveAllRecords)).size(FONT_BODY))
                    .on_press(Message::Task(TaskMsg::RemoveAllRecords))
                    .padding(PADDING_BUTTON_LG)
                    .style(theme::style::button::secondary());
            let delete_all_files_btn = button(text(fluent.get(Tr::DeleteAllFiles)).size(FONT_BODY))
                .on_press(Message::Task(TaskMsg::DeleteAll))
                .padding(PADDING_BUTTON_LG)
                .style(theme::style::button::danger());

            row![cancel_btn, remove_all_records_btn, delete_all_files_btn]
                .spacing(SPACE_XL)
                .align_y(Alignment::Center)
                .into()
        }
        ConfirmAction::ClearCompleted => {
            let confirm_btn = button(text(fluent.get(Tr::Confirm)).size(FONT_BODY))
                .on_press(Message::Task(TaskMsg::ClearCompleted))
                .padding(PADDING_BUTTON_LG)
                .style(theme::style::button::danger());

            row![cancel_btn, confirm_btn]
                .spacing(SPACE_XL)
                .align_y(Alignment::Center)
                .into()
        }
        ConfirmAction::LeaveSettings { .. } => {
            let discard_btn = button(text(fluent.get(Tr::Discard)).size(FONT_BODY))
                .on_press(Message::Settings(SettingsMsg::DiscardAndLeaveSettings))
                .padding(PADDING_BUTTON_LG)
                .style(theme::style::button::danger());
            let apply_btn = button(text(fluent.get(Tr::Apply)).size(FONT_BODY))
                .on_press(Message::Settings(SettingsMsg::ApplyAndLeaveSettings))
                .padding(PADDING_BUTTON_LG)
                .style(theme::style::button::primary());

            row![cancel_btn, discard_btn, apply_btn]
                .spacing(SPACE_XL)
                .align_y(Alignment::Center)
                .into()
        }
        ConfirmAction::UnsavedOnClose => {
            let discard_btn = button(text(fluent.get(Tr::Discard)).size(FONT_BODY))
                .on_press(Message::Settings(SettingsMsg::DiscardAndClose))
                .padding(PADDING_BUTTON_LG)
                .style(theme::style::button::danger());
            let apply_btn = button(text(fluent.get(Tr::Apply)).size(FONT_BODY))
                .on_press(Message::Settings(SettingsMsg::ApplyAndClose))
                .padding(PADDING_BUTTON_LG)
                .style(theme::style::button::primary());

            row![cancel_btn, discard_btn, apply_btn]
                .spacing(SPACE_XL)
                .align_y(Alignment::Center)
                .into()
        }
        ConfirmAction::RestartEngine { .. } => {
            let confirm_btn = button(text(fluent.get(Tr::Confirm)).size(FONT_BODY))
                .on_press(Message::Engine(EngineMsg::ConfirmRestartEngine))
                .padding(PADDING_BUTTON_LG)
                .style(theme::style::button::primary());

            row![cancel_btn, confirm_btn]
                .spacing(SPACE_XL)
                .align_y(Alignment::Center)
                .into()
        }
    };

    overlay(
        Dialog::new()
            .title(fluent.get(title_key))
            .with_close(Message::Dialog(DialogMsg::ConfirmCancel))
            .body(body)
            .footer(buttons)
            .build(),
    )
}
