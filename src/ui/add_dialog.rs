use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::{button, column, row, rule, text, text_editor, text_input};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{AddField, AddMsg, AddTab, Message, PathPickerId};
use crate::task::format_size;
use crate::ui::components::dialog::{overlay, Dialog};
use crate::ui::components::expand::expand;
use crate::ui::components::file_tree::{self, FileTreeNode};
use crate::ui::components::number_stepper::number_stepper;
use crate::ui::components::path_picker::PathPicker;
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::components::torrent_upload::{TorrentUpload, TorrentUploadEvent};
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

#[derive(Debug, Clone)]
pub struct TorrentFileEntry {
    pub index: u64,
    pub length: u64,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub struct AddDialogState {
    pub visible: bool,
    pub active_tab: AddTab,
    pub url_editor: text_editor::Content,
    pub save_picker: PathPicker,
    pub split: u16,
    pub torrent_upload: TorrentUpload,
    pub torrent_files: Vec<TorrentFileEntry>,
    pub torrent_tree: Vec<FileTreeNode>,
    pub torrent_expanded: std::collections::HashSet<String>,
    pub torrent_parse_failed: bool,
    pub torrent_scroll_offset: f32,
    pub torrent_panel_collapsed: bool,
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
            torrent_files: Vec::new(),
            torrent_tree: Vec::new(),
            torrent_expanded: std::collections::HashSet::new(),
            torrent_parse_failed: false,
            torrent_scroll_offset: 0.0,
            torrent_panel_collapsed: false,
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
        self.torrent_files.clear();
        self.torrent_tree.clear();
        self.torrent_expanded.clear();
        self.torrent_parse_failed = false;
        self.torrent_scroll_offset = 0.0;
        self.torrent_panel_collapsed = false;
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
        self.apply_payload(payload);
    }

    pub fn apply_payload(&mut self, payload: crate::clipboard_watch::ClipboardPayload) {
        match payload {
            crate::clipboard_watch::ClipboardPayload::Urls(urls) => {
                self.set_urls(urls);
                self.active_tab = AddTab::Url;
            }
            crate::clipboard_watch::ClipboardPayload::Torrent(path) => {
                self.set_torrent_path(path.to_string_lossy().to_string());
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
            AddTab::Torrent => {
                !self.torrent_upload.is_empty()
                    && save_dir_ok
                    && (self.torrent_files.is_empty()
                        || self.torrent_files.iter().any(|f| f.selected))
            }
        }
    }

    pub fn load_torrent_files(&mut self) {
        let path = self.torrent_upload.path().to_string();
        if path.is_empty() {
            self.torrent_files.clear();
            self.torrent_tree.clear();
            self.torrent_expanded.clear();
            self.torrent_parse_failed = false;
            self.torrent_scroll_offset = 0.0;
            return;
        }
        let bytes = std::fs::read(&path).ok();
        let meta = bytes.and_then(|b| crate::torrent_meta::parse_torrent(&b));
        match meta {
            Some(meta) => {
                self.torrent_files = meta
                    .files
                    .iter()
                    .map(|f| TorrentFileEntry {
                        index: f.index,
                        length: f.length,
                        selected: true,
                    })
                    .collect();
                let tuples: Vec<(u64, String, u64)> = meta
                    .files
                    .iter()
                    .map(|f| (f.index, f.path.clone(), f.length))
                    .collect();
                self.torrent_tree = file_tree::build_tree(&tuples);
                self.torrent_expanded.clear();
                file_tree::collect_dir_paths(&self.torrent_tree, &mut self.torrent_expanded);
                self.torrent_parse_failed = false;
                self.torrent_scroll_offset = 0.0;
            }
            None => {
                self.torrent_files.clear();
                self.torrent_tree.clear();
                self.torrent_expanded.clear();
                self.torrent_parse_failed = true;
                self.torrent_scroll_offset = 0.0;
            }
        }
    }

    pub fn set_torrent_path(&mut self, path: String) {
        self.torrent_upload.set_path(path);
        self.load_torrent_files();
    }

    pub fn handle_torrent_event(
        &mut self,
        event: TorrentUploadEvent,
    ) -> Option<crate::ui::components::torrent_upload::TorrentUploadAction> {
        let action = self.torrent_upload.update(event);
        if event == TorrentUploadEvent::Clear {
            self.torrent_files.clear();
            self.torrent_tree.clear();
            self.torrent_expanded.clear();
            self.torrent_parse_failed = false;
            self.torrent_scroll_offset = 0.0;
        }
        action
    }

    pub fn toggle_torrent_node(&mut self, path: &str) {
        let Some(node) = file_tree::find_node(&self.torrent_tree, path) else {
            return;
        };
        let indices = file_tree::descendant_indices(node);
        let mut pairs: Vec<(u64, bool)> = self
            .torrent_files
            .iter()
            .map(|f| (f.index, f.selected))
            .collect();
        file_tree::flip_with_guard(&mut pairs, &indices);
        for (idx, selected) in pairs {
            if let Some(entry) = self.torrent_files.iter_mut().find(|f| f.index == idx) {
                entry.selected = selected;
            }
        }
    }

    pub fn toggle_torrent_expand(&mut self, path: &str) {
        let path = path.to_string();
        if !self.torrent_expanded.remove(&path) {
            self.torrent_expanded.insert(path);
        }
    }

    pub fn toggle_torrent_panel(&mut self) {
        self.torrent_panel_collapsed = !self.torrent_panel_collapsed;
    }

    pub fn set_all_torrent_files(&mut self, selected: bool) {
        for entry in &mut self.torrent_files {
            entry.selected = selected;
        }
    }

    pub fn selected_file_indices(&self) -> Vec<u64> {
        self.torrent_files
            .iter()
            .filter(|f| f.selected)
            .map(|f| f.index)
            .collect()
    }

    pub fn selected_total(&self) -> u64 {
        self.torrent_files
            .iter()
            .filter(|f| f.selected)
            .map(|f| f.length)
            .sum()
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
    progress: f32,
) -> Element<'a, Message> {
    let placeholder = fluent.get(Tr::UrlPlaceholder);
    let url_input = text_editor(&state.url_editor)
        .placeholder(placeholder)
        .on_action(|a| Message::Add(AddMsg::UrlEditor(a)))
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
            Message::Add(AddMsg::PathPicker(PathPickerId::SaveDir, e))
        }));

    let split_input = row![]
        .push(
            text(fluent.get(Tr::SplitConnections))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(number_stepper(
            state.split,
            1..=128u16,
            1,
            |v| Message::Add(AddMsg::SplitChanged(v.to_string())),
            Length::Fixed(120.0),
        ))
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let rename_input = theme::input_layout(
        text_input("", &state.out)
            .on_input(move |s| Message::Add(AddMsg::AddFieldChanged(AddField::Out, s)))
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
        .on_toggle(|v| Message::Add(AddMsg::ToggleAdvanced(v)));

    let tab_bar = {
        let tabs = [(AddTab::Url, Tr::TabUrl), (AddTab::Torrent, Tr::TabTorrent)];
        let mut bar = row![].spacing(SPACE_SM);
        for (tab, tr) in tabs {
            let active = state.active_tab == tab;
            let btn = button(text(fluent.get(tr)).size(FONT_MEDIUM))
                .on_press(Message::Add(AddMsg::SelectAddTab(tab)))
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
                    .view(fluent, theme, |ev| Message::Add(AddMsg::TorrentUpload(ev))),
            );
            if state.torrent_parse_failed {
                body_items.push(
                    row![
                        icon::triangle_alert()
                            .size(FONT_MEDIUM)
                            .color(theme::warning(theme)),
                        text(fluent.get(Tr::TorrentParseFailed))
                            .size(FONT_SMALL)
                            .style(theme::style::text::secondary),
                    ]
                    .spacing(SPACE_SM)
                    .align_y(Alignment::Center)
                    .width(Length::Fill)
                    .into(),
                );
            } else if !state.torrent_files.is_empty() {
                let total_count = state.torrent_files.len();
                let selected_count = state.torrent_files.iter().filter(|f| f.selected).count();
                let total_size: u64 = state.torrent_files.iter().map(|f| f.length).sum();
                let sel_map: HashMap<u64, bool> = state
                    .torrent_files
                    .iter()
                    .map(|f| (f.index, f.selected))
                    .collect();
                let is_selected = |i: u64| sel_map.get(&i).copied().unwrap_or(false);

                let selected_line = text(format!(
                    "{} / {} · {}",
                    selected_count,
                    total_count,
                    format_size(state.selected_total())
                ))
                .size(FONT_SMALL)
                .style(theme::style::text::secondary);

                body_items.push(crate::ui::components::torrent_file_list::view(
                    fluent,
                    theme,
                    format!("{} ({})", fluent.get(Tr::TorrentFiles), total_count),
                    Some(format_size(total_size)),
                    Length::Fixed(230.0),
                    &state.torrent_tree,
                    &state.torrent_expanded,
                    &is_selected,
                    None::<&fn(u64) -> Option<(u64, u64)>>,
                    true,
                    state.torrent_panel_collapsed,
                    &torrent_tree_toggle,
                    &torrent_tree_expand,
                    Message::Add(AddMsg::TorrentFilesSelectAll),
                    Message::Add(AddMsg::TorrentFilesSelectNone),
                    Some(Message::Add(AddMsg::TorrentFilesTogglePanel)),
                    state.torrent_scroll_offset,
                    &torrent_files_scroll,
                ));
                body_items.push(selected_line.into());
            }
        }
    }
    body_items.push(save_row.into());
    body_items.push(split_input.into());
    body_items.push(advanced_checkbox.into());
    if state.advanced_open {
        body_items.push(advanced_form(fluent, theme, state));
    }

    let body = slim_scrollable(column(body_items).spacing(SPACE_3XL).width(Length::Fill))
        .height(Length::Fixed(350.0));

    let content = column![]
        .push(tab_bar)
        .push(rule::horizontal(1))
        .push(body)
        .spacing(SPACE_LG)
        .width(Length::Fill);

    let buttons = row![]
        .push(
            button(text(fluent.get(Tr::Cancel)).size(FONT_BODY))
                .on_press(Message::Add(AddMsg::CancelAdd))
                .padding(PADDING_BUTTON_MD)
                .style(theme::style::button::secondary()),
        )
        .push({
            let mut btn = button(text(fluent.get(Tr::Download)).size(FONT_BODY))
                .padding(PADDING_BUTTON_MD)
                .style(theme::style::button::primary());
            if state.can_submit() {
                btn = btn.on_press(Message::Add(AddMsg::AddDownload));
            }
            btn
        })
        .spacing(SPACE_XL)
        .align_y(Alignment::Center);

    overlay(expand(
        Dialog::new()
            .width(520.0)
            .spacing(SPACE_3XL)
            .title(fluent.get(Tr::NewDownload))
            .with_close(Message::Add(AddMsg::CancelAdd))
            .body(content)
            .footer(buttons)
            .build(),
        progress,
    ))
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
            .on_input(move |s| Message::Add(AddMsg::AddFieldChanged(field, s)))
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

fn torrent_tree_toggle(path: String) -> Message {
    Message::Add(AddMsg::TorrentTreeToggle(path))
}

fn torrent_tree_expand(path: String) -> Message {
    Message::Add(AddMsg::TorrentTreeExpand(path))
}

fn torrent_files_scroll(y: f32) -> Message {
    Message::Add(AddMsg::TorrentFilesScroll(y))
}
