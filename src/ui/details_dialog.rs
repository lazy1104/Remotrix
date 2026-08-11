use std::collections::{HashMap, HashSet};

use iced::widget::{button, column, container, mouse_area, progress_bar, row, rule, text};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{AddField, CtxTarget, DetailsTab, Message, NavMsg, TaskMsg};
use crate::task::{
    completed_pieces, format_add_time, format_size, format_speed, DownloadTask,
    TaskAdvancedOptions, TaskDetails,
};
use crate::ui::animation::{animation, Animated};
use crate::ui::components::ctx_input;
use crate::ui::components::ctx_menu::CtxMirrors;
use crate::ui::components::expand::expand_pinned;
use crate::ui::components::file_tree;
use crate::ui::components::key_value_list::key_value_list;
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

const DETAILS_WIDTH: f32 = 640.0;

pub struct DetailsDialogState {
    pub visible: bool,
    pub gid: Option<String>,
    pub active_tab: DetailsTab,
    pub details: Option<TaskDetails>,
    pub loading: bool,
    pub fetch_failed: bool,
    pub files_expanded: HashSet<String>,
    pub files_tree: Vec<file_tree::FileTreeNode>,
    pub files_scroll_offset: f32,
    pub pending_select: Option<(String, Vec<u64>)>,
    pub select_gen: u64,
    pub user_agent: String,
    pub http_user: String,
    pub http_passwd: String,
    pub referer: String,
    pub cookie: String,
    pub proxy_server: String,
    pub proxy_username: String,
    pub proxy_password: String,
    pub advanced_loaded: bool,
    pub advanced_saving: bool,
    pub advanced_dirty: bool,
}

impl DetailsDialogState {
    pub fn new() -> Self {
        Self {
            visible: false,
            gid: None,
            active_tab: DetailsTab::Summary,
            details: None,
            loading: false,
            fetch_failed: false,
            files_expanded: HashSet::new(),
            files_tree: Vec::new(),
            files_scroll_offset: 0.0,
            pending_select: None,
            select_gen: 0,
            user_agent: String::new(),
            http_user: String::new(),
            http_passwd: String::new(),
            referer: String::new(),
            cookie: String::new(),
            proxy_server: String::new(),
            proxy_username: String::new(),
            proxy_password: String::new(),
            advanced_loaded: false,
            advanced_saving: false,
            advanced_dirty: false,
        }
    }

    pub fn open(&mut self, gid: String) {
        self.visible = true;
        self.gid = Some(gid);
        self.active_tab = DetailsTab::Summary;
        self.details = None;
        self.loading = true;
        self.fetch_failed = false;
        self.files_expanded.clear();
        self.files_scroll_offset = 0.0;
        self.user_agent.clear();
        self.http_user.clear();
        self.http_passwd.clear();
        self.referer.clear();
        self.cookie.clear();
        self.proxy_server.clear();
        self.proxy_username.clear();
        self.proxy_password.clear();
        self.advanced_loaded = false;
        self.advanced_saving = false;
        self.advanced_dirty = false;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.gid = None;
        self.details = None;
        self.loading = false;
        self.fetch_failed = false;
        self.files_expanded.clear();
        self.files_tree.clear();
        self.files_scroll_offset = 0.0;
        self.user_agent.clear();
        self.http_user.clear();
        self.http_passwd.clear();
        self.referer.clear();
        self.cookie.clear();
        self.proxy_server.clear();
        self.proxy_username.clear();
        self.proxy_password.clear();
        self.advanced_loaded = false;
        self.advanced_saving = false;
        self.advanced_dirty = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn to_advanced(&self) -> TaskAdvancedOptions {
        TaskAdvancedOptions {
            out: String::new(),
            user_agent: self.user_agent.clone(),
            http_user: self.http_user.clone(),
            http_passwd: self.http_passwd.clone(),
            referer: self.referer.clone(),
            cookie: self.cookie.clone(),
            proxy_server: self.proxy_server.clone(),
            proxy_username: self.proxy_username.clone(),
            proxy_password: self.proxy_password.clone(),
        }
    }

    pub fn apply_advanced(&mut self, a: &TaskAdvancedOptions) {
        self.user_agent = a.user_agent.clone();
        self.http_user = a.http_user.clone();
        self.http_passwd = a.http_passwd.clone();
        self.referer = a.referer.clone();
        self.cookie = a.cookie.clone();
        self.proxy_server = a.proxy_server.clone();
        self.proxy_username = a.proxy_username.clone();
        self.proxy_password = a.proxy_password.clone();
    }
}

pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    task: Option<&'a DownloadTask>,
    state: &'a DetailsDialogState,
    progress: f32,
    ctx_mirrors: &CtxMirrors,
    progress_anim: &'a HashMap<String, Animated<f32>>,
) -> Element<'a, Message> {
    let close_btn = button(icon::x().size(FONT_HERO).line_height(1.0))
        .on_press(Message::Task(TaskMsg::CloseTaskDetails))
        .padding(PADDING_DROPDOWN)
        .style(theme::style::button::sidebar_icon(false));

    let title_text = text(fluent.get(Tr::Details)).size(FONT_TITLE);

    let header = row![]
        .push(title_text)
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(close_btn)
        .align_y(Alignment::Center)
        .padding(PADDING_BOTTOM_HEADER);

    let tab_bar = {
        let tabs = [
            (DetailsTab::Summary, Tr::TabSummary),
            (DetailsTab::Activity, Tr::TabActivity),
            (DetailsTab::Files, Tr::TabFiles),
            (DetailsTab::Advanced, Tr::TabAdvanced),
        ];
        let mut bar = row![].spacing(SPACE_SM);
        for (tab, tr) in tabs {
            let active = state.active_tab == tab;
            let btn = button(text(fluent.get(tr)).size(FONT_MEDIUM))
                .on_press(Message::Nav(NavMsg::SelectDetailsTab(tab)))
                .padding(PADDING_TAB)
                .style(theme::style::button::sidebar_icon(active));
            bar = bar.push(btn);
        }
        bar
    };

    let body: Element<'a, Message> = match task {
        None => container(
            column![]
                .spacing(SPACE_4XL)
                .push(
                    text(fluent.get(Tr::TaskGone))
                        .size(FONT_BODY)
                        .style(theme::style::text::secondary),
                )
                .push(
                    button(text(fluent.get(Tr::CloseAbout)).size(FONT_MEDIUM))
                        .on_press(Message::Task(TaskMsg::CloseTaskDetails))
                        .padding(PADDING_TAB)
                        .style(theme::style::button::secondary()),
                )
                .align_x(Alignment::Center),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into(),
        Some(task) => match state.active_tab {
            DetailsTab::Summary => {
                slim_scrollable(summary_tab(fluent, theme, task, state.details.as_ref()))
                    .height(Length::Fill)
                    .into()
            }
            DetailsTab::Activity => activity_tab(fluent, theme, task, state, progress_anim),
            DetailsTab::Files => files_tab(fluent, theme, task, state, progress_anim),
            DetailsTab::Advanced => {
                slim_scrollable(advanced_tab(fluent, theme, task, state, ctx_mirrors))
                    .height(Length::Fill)
                    .into()
            }
        },
    };

    let footer = if let Some(task) = task {
        if state.active_tab == DetailsTab::Advanced {
            let apply_enabled = {
                let task_active = !matches!(
                    task.status,
                    crate::task::TaskStatus::Completed | crate::task::TaskStatus::Removed
                );
                state.advanced_loaded
                    && state.advanced_dirty
                    && !state.advanced_saving
                    && task_active
            };
            let apply_btn = button(text(fluent.get(Tr::Apply)).size(FONT_MEDIUM))
                .padding(PADDING_TAB)
                .style(theme::style::button::primary());
            let apply_btn = if apply_enabled {
                apply_btn.on_press(Message::Task(TaskMsg::DetailsAdvancedSave))
            } else {
                apply_btn
            };
            Some(
                column![
                    iced::widget::rule::horizontal(1),
                    row![iced::widget::Space::new().width(Length::Fill), apply_btn]
                        .align_y(Alignment::Center),
                ]
                .spacing(SPACE_LG)
                .width(Length::Fill),
            )
        } else {
            None
        }
    } else {
        None
    };

    let mut content = column![]
        .push(header)
        .push(tab_bar)
        .push(iced::widget::rule::horizontal(1))
        .push(body);
    if let Some(footer) = footer {
        content = content.push(footer);
    }

    expand_pinned(
        container(
            content
                .spacing(SPACE_LG)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fixed(DETAILS_WIDTH))
        .height(Length::Fixed(480.0))
        .padding(PADDING_DETAILS)
        .style(theme::style::card),
        progress,
    )
}

fn summary_tab<'a>(
    fluent: &'a Fluent,
    _theme: &'a iced::Theme,
    task: &'a DownloadTask,
    details: Option<&'a TaskDetails>,
) -> Element<'a, Message> {
    let status_val = match task.status {
        crate::task::TaskStatus::Waiting => fluent.get(Tr::Waiting),
        crate::task::TaskStatus::Active => fluent.get(Tr::Active),
        crate::task::TaskStatus::Paused => fluent.get(Tr::Paused),
        crate::task::TaskStatus::Completed => fluent.get(Tr::Completed),
        crate::task::TaskStatus::Error => fluent.get(Tr::Error),
        crate::task::TaskStatus::Removed => fluent.get(Tr::Removed),
    };

    let mut rows = vec![
        (fluent.get(Tr::FieldGid), task.gid.clone()),
        (fluent.get(Tr::FieldFileName), task.name.clone()),
        (
            fluent.get(Tr::FieldDownloadLocation),
            task.save_dir.to_string_lossy().to_string(),
        ),
        (fluent.get(Tr::FieldTaskStatus), status_val),
        (
            fluent.get(Tr::FieldAddedTime),
            format_add_time(task.added_at),
        ),
    ];

    if task.info_hash.is_some() {
        if let Some(hash) = task.info_hash.as_ref() {
            rows.push((fluent.get(Tr::FieldInfoHash), hash.to_uppercase()));
        }
        if let Some(details) = details {
            if details.num_pieces > 0 {
                rows.push((fluent.get(Tr::PieceSize), format_size(details.piece_length)));
                rows.push((
                    fluent.get(Tr::FieldPieceCount),
                    details.num_pieces.to_string(),
                ));
            }
            if let Some(date) = details.creation_date {
                rows.push((fluent.get(Tr::FieldCreationDate), format_add_time(date)));
            }
            if let Some(mode) = details.mode.as_deref() {
                let mode_val = if mode == "single" {
                    fluent.get(Tr::TorrentModeSingle)
                } else {
                    fluent.get(Tr::TorrentModeMulti)
                };
                rows.push((fluent.get(Tr::FieldTorrentMode), mode_val));
            }
            if let Some(comment) = details.comment.as_deref() {
                if !comment.is_empty() {
                    rows.push((fluent.get(Tr::FieldComment), comment.to_string()));
                }
            }
        }
    }

    key_value_list(rows)
}

fn activity_tab<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    task: &'a DownloadTask,
    state: &'a DetailsDialogState,
    progress_anim: &'a HashMap<String, Animated<f32>>,
) -> Element<'a, Message> {
    let text_secondary_fn = theme::style::text::secondary;

    let (piece_content, progress_section, info_section): (
        Element<'a, Message>,
        Element<'a, Message>,
        Element<'a, Message>,
    ) = if let Some(ref details) = state.details {
        let (done_pieces, total_pieces) =
            completed_pieces(details.bitfield.as_deref(), details.num_pieces);
        let piece_info = text(format!(
            "{} {}/{}  {} {}",
            fluent.get(Tr::Pieces),
            done_pieces,
            total_pieces,
            fluent.get(Tr::PieceSize),
            format_size(details.piece_length),
        ))
        .size(FONT_SMALL)
        .style(text_secondary_fn);

        let piece_map_el = crate::ui::components::piece_map::view(
            details.bitfield.clone(),
            details.num_pieces,
            DETAILS_WIDTH - 2.0 * PADDING_DETAILS as f32,
        );

        let mut piece_content = column![piece_info].spacing(SPACE_SM);
        if let Some(map) = piece_map_el {
            piece_content = piece_content.push(map);
        }

        let pct = progress_anim
            .get(&task.gid)
            .map(|p| *p.value())
            .unwrap_or_else(|| task.progress_pct());
        let bar_color = theme::task_bar_color(theme, task.status, task.is_seeding);
        let base_bar = progress_bar(0.0..=100.0, pct)
            .girth(Length::Fixed(8.0))
            .style(theme::style::progress::task(bar_color));
        let bar: Element<'a, Message> = if let Some(anim) = progress_anim.get(&task.gid) {
            let gid = task.gid.clone();
            animation(anim, base_bar)
                .on_update(move |e| Message::ProgressAnim(gid.clone(), e))
                .into()
        } else {
            base_bar.into()
        };

        let downloaded_text = format!(
            "{} / {}",
            format_size(task.downloaded),
            if task.total == 0 {
                "—".to_string()
            } else {
                format_size(task.total)
            }
        );

        let speed_str = if task.is_download_active() || task.speed > 0 {
            format_speed(task.speed)
        } else {
            "—".to_string()
        };

        let conn_str = task.connections.to_string();

        (
            piece_content.into(),
            column![]
                .push(bar)
                .push(
                    text(downloaded_text)
                        .size(FONT_SMALL)
                        .style(text_secondary_fn),
                )
                .spacing(SPACE_SM)
                .into(),
            column![]
                .push(
                    row![]
                        .push(
                            text(fluent.get(Tr::Speed))
                                .size(FONT_SMALL)
                                .style(text_secondary_fn),
                        )
                        .push(
                            text(speed_str)
                                .size(FONT_SMALL)
                                .color(theme::primary(theme)),
                        )
                        .spacing(SPACE_SM)
                        .align_y(Alignment::Center),
                )
                .push(
                    row![]
                        .push(
                            text(fluent.get(Tr::Connections))
                                .size(FONT_SMALL)
                                .style(text_secondary_fn),
                        )
                        .push(text(conn_str).size(FONT_SMALL))
                        .spacing(SPACE_SM)
                        .align_y(Alignment::Center),
                )
                .spacing(SPACE_SM)
                .into(),
        )
    } else {
        let fallback_text = if task.is_completed() {
            fluent.get(Tr::TaskCompleted)
        } else if state.loading {
            fluent.get(Tr::Loading)
        } else {
            fluent.get(Tr::TaskGone)
        };
        let empty_widget: Element<'a, Message> = if state.loading {
            row![
                crate::ui::components::spinner::Spinner::refresh(
                    theme::accent(theme),
                    FONT_ICON as f32
                )
                .view(),
                text(fallback_text).size(FONT_BODY).style(text_secondary_fn),
            ]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center)
            .into()
        } else {
            text(fallback_text)
                .size(FONT_BODY)
                .style(text_secondary_fn)
                .into()
        };
        let empty: Element<'a, Message> = container(empty_widget)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        return empty;
    };

    column![piece_content, progress_section, info_section]
        .spacing(SPACE_2XL)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn files_tab<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    task: &'a DownloadTask,
    state: &'a DetailsDialogState,
    progress_anim: &'a HashMap<String, Animated<f32>>,
) -> Element<'a, Message> {
    let text_secondary_fn = theme::style::text::secondary;

    let pct = progress_anim
        .get(&task.gid)
        .map(|p| *p.value())
        .unwrap_or_else(|| task.progress_pct());
    let bar_color = theme::task_bar_color(theme, task.status, task.is_seeding);
    let base_bar = progress_bar(0.0..=100.0, pct)
        .girth(Length::Fixed(8.0))
        .style(theme::style::progress::task(bar_color));
    let overall_bar: Element<'a, Message> = if let Some(anim) = progress_anim.get(&task.gid) {
        let gid = task.gid.clone();
        animation(anim, base_bar)
            .on_update(move |e| Message::ProgressAnim(gid.clone(), e))
            .into()
    } else {
        base_bar.into()
    };

    let overall_info = text(format!(
        "{} / {}  ({:.1}%)",
        format_size(task.downloaded),
        if task.total == 0 {
            "—".to_string()
        } else {
            format_size(task.total)
        },
        pct,
    ))
    .size(FONT_SMALL)
    .style(text_secondary_fn);

    let file_list: Element<'a, Message> = if let Some(ref details) = state.details {
        let files_map: HashMap<u64, (bool, u64, u64)> = details
            .files
            .iter()
            .map(|f| (f.index, (f.selected, f.completed_length, f.length)))
            .collect();
        let is_selected = |i: u64| files_map.get(&i).map(|(s, _, _)| *s).unwrap_or(false);
        let progress = |i: u64| files_map.get(&i).map(|(_, c, l)| (*c, *l));
        let enabled = !matches!(
            task.status,
            crate::task::TaskStatus::Completed | crate::task::TaskStatus::Removed
        );
        crate::ui::components::torrent_file_list::view(
            fluent,
            theme,
            fluent.get(Tr::TorrentFiles),
            None,
            Length::Fill,
            &state.files_tree,
            &state.files_expanded,
            &is_selected,
            Some(&progress),
            enabled,
            false,
            &details_tree_toggle,
            &details_tree_expand,
            Message::Task(TaskMsg::DetailsFilesSelectAll),
            Message::Task(TaskMsg::DetailsFilesSelectNone),
            None,
            state.files_scroll_offset,
            &details_files_scroll,
        )
    } else {
        let fallback_text = if task.is_completed() {
            fluent.get(Tr::TaskCompleted)
        } else if state.loading {
            fluent.get(Tr::Loading)
        } else {
            fluent.get(Tr::TaskGone)
        };
        let fallback_widget: Element<'a, Message> = if state.loading {
            row![
                crate::ui::components::spinner::Spinner::refresh(
                    theme::accent(theme),
                    FONT_ICON as f32
                )
                .view(),
                text(fallback_text).size(FONT_BODY).style(text_secondary_fn),
            ]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center)
            .into()
        } else {
            text(fallback_text)
                .size(FONT_BODY)
                .style(text_secondary_fn)
                .into()
        };
        container(
            container(fallback_widget)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(PADDING_XS)
        .style(theme::style::tree_frame)
        .into()
    };

    column![]
        .push(overall_bar)
        .push(overall_info)
        .push(file_list)
        .spacing(SPACE_LG)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn details_tree_toggle(path: String) -> Message {
    Message::Task(TaskMsg::DetailsTreeToggle(path))
}

fn details_tree_expand(path: String) -> Message {
    Message::Task(TaskMsg::DetailsTreeExpand(path))
}

fn details_files_scroll(y: f32) -> Message {
    Message::Task(TaskMsg::DetailsFilesScroll(y))
}

fn advanced_field<'a>(
    fluent: &'a Fluent,
    label: Tr,
    placeholder: Tr,
    value: &'a str,
    field: AddField,
    secure: bool,
    ctx_mirrors: &CtxMirrors,
) -> Element<'a, Message> {
    let target = CtxTarget::DetailsAdvanced(field);
    let placeholder = fluent.get(placeholder);
    let mut input = ctx_input::CtxInput::new(
        &placeholder,
        value,
        ctx_mirrors.get(&target).cloned().unwrap_or_default(),
    )
    .on_input(move |s| Message::Task(TaskMsg::DetailsAdvancedFieldChanged(field, s)))
    .padding(theme::INPUT_PADDING)
    .size(FONT_MEDIUM)
    .width(Length::Fill)
    .style(theme::style::input::standard);
    if secure {
        input = input.secure(true);
    }
    let label = if label == Tr::HttpAuthPassword {
        String::new()
    } else {
        fluent.get(label)
    };
    row![
        text(label)
            .size(FONT_SMALL)
            .style(theme::style::text::secondary)
            .width(Length::Fixed(140.0)),
        mouse_area(input).on_right_press(Message::CtxOpen(target)),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn advanced_tab<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    _task: &'a DownloadTask,
    state: &'a DetailsDialogState,
    ctx_mirrors: &CtxMirrors,
) -> Element<'a, Message> {
    column![
        advanced_field(
            fluent,
            Tr::UserAgent,
            Tr::UserAgentPlaceholder,
            &state.user_agent,
            AddField::UserAgent,
            false,
            ctx_mirrors
        ),
        advanced_field(
            fluent,
            Tr::HttpAuthAccount,
            Tr::HttpAuthAccountPlaceholder,
            &state.http_user,
            AddField::HttpUser,
            false,
            ctx_mirrors
        ),
        advanced_field(
            fluent,
            Tr::HttpAuthPassword,
            Tr::HttpAuthPasswordPlaceholder,
            &state.http_passwd,
            AddField::HttpPasswd,
            true,
            ctx_mirrors
        ),
        advanced_field(
            fluent,
            Tr::Referer,
            Tr::RefererPlaceholder,
            &state.referer,
            AddField::Referer,
            false,
            ctx_mirrors
        ),
        advanced_field(
            fluent,
            Tr::Cookie,
            Tr::CookiePlaceholder,
            &state.cookie,
            AddField::Cookie,
            false,
            ctx_mirrors
        ),
        rule::horizontal(1),
        text(fluent.get(Tr::Proxy))
            .size(FONT_TITLE)
            .color(theme::accent(theme)),
        advanced_field(
            fluent,
            Tr::ProxyAddress,
            Tr::ProxyAddressPlaceholder,
            &state.proxy_server,
            AddField::ProxyServer,
            false,
            ctx_mirrors
        ),
        advanced_field(
            fluent,
            Tr::ProxyUsername,
            Tr::ProxyUsernamePlaceholder,
            &state.proxy_username,
            AddField::ProxyUsername,
            false,
            ctx_mirrors
        ),
        advanced_field(
            fluent,
            Tr::ProxyPassword,
            Tr::ProxyPasswordPlaceholder,
            &state.proxy_password,
            AddField::ProxyPassword,
            true,
            ctx_mirrors
        ),
    ]
    .spacing(SPACE_XL)
    .width(Length::Fill)
    .into()
}
