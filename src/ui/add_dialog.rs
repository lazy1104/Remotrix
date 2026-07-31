use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::{button, column, row, text, text_editor};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, PathPickerId};
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::components::number_stepper::number_stepper;
use crate::ui::components::path_picker::PathPicker;
use crate::ui::theme;

#[derive(Debug, Clone)]
pub struct AddDialogState {
    pub visible: bool,
    pub url_editor: text_editor::Content,
    pub save_picker: PathPicker,
    pub split: u16,
    pub torrent_picker: PathPicker,
}

impl AddDialogState {
    pub fn new(default_dir: PathBuf) -> Self {
        Self {
            visible: false,
            url_editor: text_editor::Content::new(),
            save_picker: PathPicker::folder(default_dir.to_string_lossy(), true),
            split: 16,
            torrent_picker: PathPicker::file(String::new()),
        }
    }

    pub fn open(&mut self, default_dir: PathBuf, default_split: u16) {
        self.visible = true;
        self.url_editor = text_editor::Content::new();
        self.save_picker.set_value(default_dir.to_string_lossy());
        self.split = default_split;
        self.torrent_picker.set_value("");
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn can_submit(&self) -> bool {
        (!self.url_editor.text().trim().is_empty() && !self.save_picker.value().is_empty())
            || !self.torrent_picker.value().is_empty()
    }
}

pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    state: &'a AddDialogState,
    path_history: &'a HashMap<String, Vec<String>>,
) -> Element<'a, Message> {
    let placeholder = fluent.get(Tr::UrlPlaceholder);
    let url_input = text_editor(&state.url_editor)
        .placeholder(placeholder)
        .on_action(Message::UrlEditor)
        .height(Length::Fixed(120.0))
        .padding(10)
        .size(14)
        .style(theme::style::text_editor::standard);

    let torrent_row = column![]
        .spacing(4)
        .push(
            text(fluent.get(Tr::OrTorrent))
                .size(12)
                .style(theme::style::text::secondary),
        )
        .push(state.torrent_picker.view(fluent, theme, &[], |e| {
            Message::PathPicker(PathPickerId::Torrent, e)
        }));

    let hist_save: &[String] = path_history
        .get("save_dir")
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let save_row = column![]
        .spacing(4)
        .push(
            text(fluent.get(Tr::SaveTo))
                .size(12)
                .style(theme::style::text::secondary),
        )
        .push(state.save_picker.view(fluent, theme, hist_save, |e| {
            Message::PathPicker(PathPickerId::SaveDir, e)
        }));

    let split_input = row![]
        .push(
            text(fluent.get(Tr::SplitConnections))
                .size(12)
                .style(theme::style::text::secondary),
        )
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(number_stepper(
            &state.split,
            1..=128u16,
            1,
            |v| Message::SplitChanged(v.to_string()),
            Length::Fixed(120.0),
        ))
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let body = column![url_input, torrent_row, save_row, split_input]
        .spacing(14)
        .width(Length::Fill);

    let buttons = row![]
        .push(
            button(text(fluent.get(Tr::Cancel)).size(14))
                .on_press(Message::CancelAdd)
                .padding([8, 18])
                .style(theme::style::button::secondary()),
        )
        .push({
            let mut btn = button(text(fluent.get(Tr::Download)).size(14))
                .padding([8, 18])
                .style(theme::style::button::primary());
            if state.can_submit() {
                btn = btn.on_press(Message::AddDownload);
            }
            btn
        })
        .spacing(10)
        .align_y(Alignment::Center);

    overlay(
        Dialog::new()
            .width(520.0)
            .spacing(14.0)
            .title(fluent.get(Tr::NewDownload))
            .with_close(Message::CancelAdd)
            .body(body)
            .footer(buttons)
            .build(),
    )
}
