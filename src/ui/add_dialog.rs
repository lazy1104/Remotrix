use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::{button, column, container, row, text, text_editor, text_input};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, PathPickerId};
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
        .size(14);

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

    let split_str = state.split.to_string();
    let split_input = row![]
        .push(
            text(fluent.get(Tr::SplitConnections))
                .size(12)
                .style(theme::style::text::secondary),
        )
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(
            text_input("16", split_str.as_str())
                .on_input(Message::SplitChanged)
                .width(Length::Fixed(80.0))
                .padding(8)
                .size(14),
        )
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let buttons = row![]
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(
            button(text(fluent.get(Tr::Cancel)).size(14))
                .on_press(Message::CancelAdd)
                .padding([8, 18])
                .style(theme::style::button::secondary()),
        )
        .push({
            let mut btn = button(text(fluent.get(Tr::Download)).size(14))
                .on_press(Message::AddDownload)
                .padding([8, 18]);
            btn = if state.can_submit() {
                btn.style(theme::style::button::primary())
            } else {
                btn.style(theme::style::button::secondary())
            };
            btn
        })
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let panel = container(
        column![]
            .spacing(14)
            .push(text(fluent.get(Tr::NewDownload)).size(20))
            .push(url_input)
            .push(torrent_row)
            .push(save_row)
            .push(split_input)
            .push(buttons),
    )
    .width(Length::Fixed(520.0))
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
