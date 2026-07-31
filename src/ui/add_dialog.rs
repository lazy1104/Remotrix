use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::{button, column, row, rule, text, text_editor, text_input};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{AddField, Message, PathPickerId};
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::components::number_stepper::number_stepper;
use crate::ui::components::path_picker::PathPicker;
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::theme;

#[derive(Debug, Clone)]
pub struct AddDialogState {
    pub visible: bool,
    pub url_editor: text_editor::Content,
    pub save_picker: PathPicker,
    pub split: u16,
    pub torrent_picker: PathPicker,
    pub out: String,
    pub advanced_open: bool,
    pub user_agent: String,
    pub http_user: String,
    pub http_passwd: String,
    pub referer: String,
    pub cookie: String,
    pub proxy_server: String,
    pub proxy_username: String,
    pub proxy_password: String,
}

impl AddDialogState {
    pub fn new(default_dir: PathBuf) -> Self {
        Self {
            visible: false,
            url_editor: text_editor::Content::new(),
            save_picker: PathPicker::folder(default_dir.to_string_lossy(), true),
            split: 16,
            torrent_picker: PathPicker::file(String::new()),
            out: String::new(),
            advanced_open: false,
            user_agent: String::new(),
            http_user: String::new(),
            http_passwd: String::new(),
            referer: String::new(),
            cookie: String::new(),
            proxy_server: String::new(),
            proxy_username: String::new(),
            proxy_password: String::new(),
        }
    }

    pub fn open(&mut self, default_dir: PathBuf, default_split: u16) {
        self.visible = true;
        self.url_editor = text_editor::Content::new();
        self.save_picker.set_value(default_dir.to_string_lossy());
        self.split = default_split;
        self.torrent_picker.set_value("");
        self.out.clear();
        self.advanced_open = false;
        self.user_agent.clear();
        self.http_user.clear();
        self.http_passwd.clear();
        self.referer.clear();
        self.cookie.clear();
        self.proxy_server.clear();
        self.proxy_username.clear();
        self.proxy_password.clear();
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

    pub fn url_count(&self) -> usize {
        self.url_editor
            .text()
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .count()
    }

    pub fn has_torrent(&self) -> bool {
        !self.torrent_picker.value().is_empty()
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

    let rename_input = theme::input_layout(
        text_input("", &state.out)
            .on_input(move |s| Message::AddFieldChanged(AddField::Out, s))
            .width(Length::Fill)
            .style(theme::style::input::standard),
    );
    let rename_row = row![]
        .push(
            text(fluent.get(Tr::RenameFile))
                .size(12)
                .style(theme::style::text::secondary)
                .width(Length::Fixed(140.0)),
        )
        .push(if state.url_count() > 1 {
            let mut input = rename_input;
            input = input.on_input_maybe(Option::<fn(String) -> Message>::None);
            row![]
                .push(input)
                .push(
                    text(fluent.get(Tr::RenameMultiUrlHint))
                        .size(11)
                        .style(theme::style::text::secondary),
                )
                .spacing(6)
                .align_y(Alignment::Center)
                .width(Length::Fill)
        } else {
            row![rename_input].width(Length::Fill)
        })
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let advanced_checkbox = iced::widget::checkbox(state.advanced_open)
        .label(fluent.get(Tr::AdvancedOptions))
        .on_toggle(Message::ToggleAdvanced);

    let mut body_items = vec![
        url_input.into(),
        torrent_row.into(),
        save_row.into(),
        split_input.into(),
    ];
    if !state.has_torrent() {
        body_items.push(rename_row.into());
    }
    body_items.push(advanced_checkbox.into());
    if state.advanced_open {
        body_items.push(advanced_form(fluent, theme, state));
    }

    let body = slim_scrollable(column(body_items).spacing(14).width(Length::Fill))
        .height(Length::Fixed(400.0));

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

fn advanced_field<'a>(
    fluent: &'a Fluent,
    label: Tr,
    value: &'a str,
    field: AddField,
    secure: bool,
) -> Element<'a, Message> {
    let mut input = theme::input_layout(
        text_input("", value)
            .on_input(move |s| Message::AddFieldChanged(field, s))
            .width(Length::Fill)
            .style(theme::style::input::standard),
    );
    if secure {
        input = input.secure(true);
    }
    row![
        text(fluent.get(label))
            .size(12)
            .style(theme::style::text::secondary)
            .width(Length::Fixed(140.0)),
        input,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn advanced_form<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    state: &'a AddDialogState,
) -> Element<'a, Message> {
    column![
        advanced_field(
            fluent,
            Tr::UserAgent,
            &state.user_agent,
            AddField::UserAgent,
            false
        ),
        advanced_field(
            fluent,
            Tr::HttpAuthAccount,
            &state.http_user,
            AddField::HttpUser,
            false
        ),
        advanced_field(
            fluent,
            Tr::HttpAuthPassword,
            &state.http_passwd,
            AddField::HttpPasswd,
            true
        ),
        advanced_field(
            fluent,
            Tr::Referer,
            &state.referer,
            AddField::Referer,
            false
        ),
        advanced_field(fluent, Tr::Cookie, &state.cookie, AddField::Cookie, false),
        rule::horizontal(1),
        text(fluent.get(Tr::Proxy))
            .size(16)
            .color(theme::accent(theme)),
        advanced_field(
            fluent,
            Tr::ProxyAddress,
            &state.proxy_server,
            AddField::ProxyServer,
            false
        ),
        advanced_field(
            fluent,
            Tr::ProxyUsername,
            &state.proxy_username,
            AddField::ProxyUsername,
            false
        ),
        advanced_field(
            fluent,
            Tr::ProxyPassword,
            &state.proxy_password,
            AddField::ProxyPassword,
            true
        ),
    ]
    .spacing(10)
    .width(Length::Fill)
    .into()
}
