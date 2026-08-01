use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::{button, column, row, rule, text, text_editor, text_input};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{AddField, AddTab, Message, PathPickerId};
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::components::number_stepper::number_stepper;
use crate::ui::components::path_picker::PathPicker;
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::components::torrent_upload::TorrentUpload;
use crate::ui::dims::*;
use crate::ui::theme;

#[derive(Debug, Clone)]
pub struct AddDialogState {
    pub visible: bool,
    pub active_tab: AddTab,
    pub url_editor: text_editor::Content,
    pub save_picker: PathPicker,
    pub split: u16,
    pub torrent_upload: TorrentUpload,
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
            active_tab: AddTab::Url,
            url_editor: text_editor::Content::new(),
            save_picker: PathPicker::folder(default_dir.to_string_lossy(), true),
            split: 16,
            torrent_upload: TorrentUpload::new(),
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
        self.active_tab = AddTab::Url;
        self.url_editor = text_editor::Content::new();
        self.save_picker.set_value(default_dir.to_string_lossy());
        self.split = default_split;
        self.torrent_upload.clear();
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

    pub fn set_urls(&mut self, urls: Vec<String>) {
        self.url_editor = text_editor::Content::with_text(&urls.join("\n"));
    }

    pub fn open_with(
        &mut self,
        default_dir: PathBuf,
        default_split: u16,
        payload: crate::clipboard_watch::ClipboardPayload,
    ) {
        self.save_picker.close_history();
        self.open(default_dir, default_split);
        match payload {
            crate::clipboard_watch::ClipboardPayload::Urls(urls) => self.set_urls(urls),
            crate::clipboard_watch::ClipboardPayload::Torrent(path) => {
                self.torrent_upload.set_path(path.to_string_lossy());
                self.active_tab = AddTab::Torrent;
            }
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn can_submit(&self) -> bool {
        let save_dir_ok = !self.save_picker.value().is_empty();
        match self.active_tab {
            AddTab::Url => !self.url_editor.text().trim().is_empty() && save_dir_ok,
            AddTab::Torrent => !self.torrent_upload.is_empty() && save_dir_ok,
        }
    }

    pub fn url_count(&self) -> usize {
        self.url_editor
            .text()
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .count()
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
        .padding(PADDING_EDITOR)
        .size(FONT_BODY)
        .style(theme::style::text_editor::standard);

    let hist_save: &[String] = path_history
        .get("save_dir")
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let save_row = column![]
        .spacing(SPACE_SM)
        .push(
            text(fluent.get(Tr::SaveTo))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .push(state.save_picker.view(fluent, theme, hist_save, |e| {
            Message::PathPicker(PathPickerId::SaveDir, e)
        }));

    let split_input = row![]
        .push(
            text(fluent.get(Tr::SplitConnections))
                .size(FONT_SMALL)
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
                .size(FONT_SMALL)
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
                        .size(FONT_TINY)
                        .style(theme::style::text::secondary),
                )
                .spacing(SPACE_MD)
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

    let tab_bar = {
        let tabs = [(AddTab::Url, Tr::TabUrl), (AddTab::Torrent, Tr::TabTorrent)];
        let mut bar = row![].spacing(SPACE_SM);
        for (tab, tr) in tabs {
            let active = state.active_tab == tab;
            let btn = button(text(fluent.get(tr)).size(FONT_MEDIUM))
                .on_press(Message::SelectAddTab(tab))
                .padding(PADDING_TAB)
                .style(theme::style::button::sidebar_icon(active));
            bar = bar.push(btn);
        }
        bar
    };

    let mut body_items: Vec<Element<'a, Message>> = Vec::new();
    match state.active_tab {
        AddTab::Url => {
            body_items.push(url_input.into());
            body_items.push(rename_row.into());
        }
        AddTab::Torrent => {
            body_items.push(
                state
                    .torrent_upload
                    .view(fluent, theme, Message::TorrentUpload),
            );
        }
    }
    body_items.push(save_row.into());
    body_items.push(split_input.into());
    body_items.push(advanced_checkbox.into());
    if state.advanced_open {
        body_items.push(advanced_form(fluent, theme, state));
    }

    let body = slim_scrollable(column(body_items).spacing(SPACE_3XL).width(Length::Fill))
        .height(Length::Fixed(400.0));

    let content = column![]
        .push(tab_bar)
        .push(rule::horizontal(1))
        .push(body)
        .spacing(SPACE_LG)
        .width(Length::Fill);

    let buttons = row![]
        .push(
            button(text(fluent.get(Tr::Cancel)).size(FONT_BODY))
                .on_press(Message::CancelAdd)
                .padding(PADDING_BUTTON_MD)
                .style(theme::style::button::secondary()),
        )
        .push({
            let mut btn = button(text(fluent.get(Tr::Download)).size(FONT_BODY))
                .padding(PADDING_BUTTON_MD)
                .style(theme::style::button::primary());
            if state.can_submit() {
                btn = btn.on_press(Message::AddDownload);
            }
            btn
        })
        .spacing(SPACE_XL)
        .align_y(Alignment::Center);

    overlay(
        Dialog::new()
            .width(520.0)
            .spacing(SPACE_3XL)
            .title(fluent.get(Tr::NewDownload))
            .with_close(Message::CancelAdd)
            .body(content)
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
            .size(FONT_SMALL)
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
            .size(FONT_TITLE)
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
    .spacing(SPACE_XL)
    .width(Length::Fill)
    .into()
}
