use std::path::PathBuf;

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::Message;
use crate::ui::theme;

#[derive(Debug, Clone)]
pub struct AddDialogState {
    pub visible: bool,
    pub url: String,
    pub save_dir: PathBuf,
    pub split: u16,
    pub torrent_path: Option<PathBuf>,
}

impl AddDialogState {
    pub fn new(default_dir: PathBuf) -> Self {
        Self {
            visible: false,
            url: String::new(),
            save_dir: default_dir,
            split: 16,
            torrent_path: None,
        }
    }

    pub fn open(&mut self, default_dir: PathBuf, default_split: u16) {
        self.visible = true;
        self.url.clear();
        self.save_dir = default_dir;
        self.split = default_split;
        self.torrent_path = None;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn can_submit(&self) -> bool {
        (!self.url.trim().is_empty() && !self.save_dir.as_os_str().is_empty())
            || self.torrent_path.is_some()
    }
}

pub fn view<'a>(
    fluent: &'a Fluent,
    _theme: &iced::Theme,
    state: &'a AddDialogState,
) -> Element<'a, Message> {
    let placeholder = fluent.get(Tr::UrlPlaceholder);
    let url_input = text_input(&placeholder, state.url.as_str())
        .on_input(Message::AddUrlChanged)
        .padding(10)
        .size(14);

    let torrent_row = row![]
        .push(
            text(fluent.get(Tr::OrTorrent))
                .size(12)
                .style(theme::style::text::secondary),
        )
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(
            button(text(fluent.get(Tr::Browse)).size(12))
                .on_press(Message::BrowseTorrent)
                .padding([6, 12])
                .style(theme::style::button::secondary()),
        )
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let save_dir_str = state.save_dir.to_string_lossy().to_string();

    let save_row = row![]
        .push(
            column![]
                .push(
                    text(fluent.get(Tr::SaveTo))
                        .size(12)
                        .style(theme::style::text::secondary),
                )
                .push(text(save_dir_str.clone()).size(13))
                .spacing(2)
                .width(Length::Fill),
        )
        .push(
            button(text(fluent.get(Tr::Browse)).size(12))
                .on_press(Message::BrowseSaveDir)
                .padding([6, 12])
                .style(theme::style::button::secondary()),
        )
        .align_y(Alignment::Center)
        .width(Length::Fill);

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
