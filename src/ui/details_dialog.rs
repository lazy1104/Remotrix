use std::collections::{HashMap, HashSet};

use iced::widget::{button, column, container, progress_bar, row, text};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{DetailsTab, Message, NavMsg, TaskMsg};
use crate::task::{
    completed_pieces, format_add_time, format_size, format_speed, DownloadTask, TaskDetails,
};
use crate::ui::components::dialog::overlay;
use crate::ui::components::file_tree;
use crate::ui::components::key_value_list::key_value_list;
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
    pub files_expanded: HashSet<String>,
    pub files_tree: Vec<file_tree::FileTreeNode>,
    pub files_scroll_offset: f32,
    pub pending_select: Option<(String, Vec<u64>)>,
    pub select_gen: u64,
}

impl DetailsDialogState {
    pub fn new() -> Self {
        Self {
            visible: false,
            gid: None,
            active_tab: DetailsTab::Summary,
            details: None,
            loading: false,
            files_expanded: HashSet::new(),
            files_tree: Vec::new(),
            files_scroll_offset: 0.0,
            pending_select: None,
            select_gen: 0,
        }
    }

    pub fn open(&mut self, gid: String) {
        self.visible = true;
        self.gid = Some(gid);
        self.active_tab = DetailsTab::Summary;
        self.details = None;
        self.loading = true;
        self.files_expanded.clear();
        self.files_scroll_offset = 0.0;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.gid = None;
        self.details = None;
        self.loading = false;
        self.files_expanded.clear();
        self.files_tree.clear();
        self.files_scroll_offset = 0.0;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    task: Option<&'a DownloadTask>,
    state: &'a DetailsDialogState,
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
            DetailsTab::Summary => summary_tab(fluent, theme, task),
            DetailsTab::Activity => activity_tab(fluent, theme, task, state),
            DetailsTab::Files => files_tab(fluent, theme, task, state),
        },
    };

    let panel = container(
        column![]
            .push(header)
            .push(tab_bar)
            .push(iced::widget::rule::horizontal(1))
            .push(body)
            .spacing(SPACE_LG)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fixed(DETAILS_WIDTH))
    .height(Length::Fixed(480.0))
    .padding(PADDING_DETAILS)
    .style(theme::style::card);

    overlay(panel)
}

fn summary_tab<'a>(
    fluent: &'a Fluent,
    _theme: &'a iced::Theme,
    task: &'a DownloadTask,
) -> Element<'a, Message> {
    let status_val = match task.status {
        crate::task::TaskStatus::Waiting => fluent.get(Tr::Waiting),
        crate::task::TaskStatus::Active => fluent.get(Tr::Active),
        crate::task::TaskStatus::Paused => fluent.get(Tr::Paused),
        crate::task::TaskStatus::Completed => fluent.get(Tr::Completed),
        crate::task::TaskStatus::Error => fluent.get(Tr::Error),
        crate::task::TaskStatus::Removed => fluent.get(Tr::Removed),
    };

    let rows = [
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

    key_value_list(rows)
}

fn activity_tab<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    task: &'a DownloadTask,
    state: &'a DetailsDialogState,
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

        let pct = task.progress_pct();
        let bar_color = theme::task_bar_color(theme, task.status);
        let bar = progress_bar(0.0..=100.0, pct)
            .girth(Length::Fixed(8.0))
            .style(theme::style::progress::task(bar_color));

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
        let loading_text = if state.loading {
            fluent.get(Tr::Loading)
        } else {
            fluent.get(Tr::TaskGone)
        };
        let empty: Element<'a, Message> =
            container(text(loading_text).size(FONT_BODY).style(text_secondary_fn))
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
) -> Element<'a, Message> {
    let text_secondary_fn = theme::style::text::secondary;

    let pct = task.progress_pct();
    let bar_color = theme::task_bar_color(theme, task.status);
    let overall_bar = progress_bar(0.0..=100.0, pct)
        .girth(Length::Fixed(8.0))
        .style(theme::style::progress::task(bar_color));

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
        container(
            container(
                text(fluent.get(Tr::Loading))
                    .size(FONT_BODY)
                    .style(text_secondary_fn),
            )
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
