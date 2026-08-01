use iced::widget::{button, column, container, progress_bar, row, text};
use iced::{Alignment, Element, Length, Padding};

use crate::i18n::{Fluent, Tr};
use crate::message::{DetailsTab, Message};
use crate::task::{
    completed_pieces, format_add_time, format_size, format_speed, DownloadTask, TaskDetails,
};
use crate::ui::components::dialog::overlay;
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::components::truncated_text::truncated_text;
use crate::ui::icon;
use crate::ui::theme;

pub struct DetailsDialogState {
    pub visible: bool,
    pub gid: Option<String>,
    pub active_tab: DetailsTab,
    pub details: Option<TaskDetails>,
    pub loading: bool,
}

impl DetailsDialogState {
    pub fn new() -> Self {
        Self {
            visible: false,
            gid: None,
            active_tab: DetailsTab::Summary,
            details: None,
            loading: false,
        }
    }

    pub fn open(&mut self, gid: String) {
        self.visible = true;
        self.gid = Some(gid);
        self.active_tab = DetailsTab::Summary;
        self.details = None;
        self.loading = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.gid = None;
        self.details = None;
        self.loading = false;
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
    let close_btn = button(icon::x().size(18).line_height(1.0))
        .on_press(Message::CloseTaskDetails)
        .padding(6)
        .style(theme::style::button::sidebar_icon(false));

    let title_text = text(fluent.get(Tr::Details)).size(16);

    let header = row![]
        .push(title_text)
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(close_btn)
        .align_y(Alignment::Center)
        .padding(Padding::new(0.0).bottom(12.0));

    let tab_bar = {
        let tabs = [
            (DetailsTab::Summary, Tr::TabSummary),
            (DetailsTab::Activity, Tr::TabActivity),
            (DetailsTab::Files, Tr::TabFiles),
        ];
        let mut bar = row![].spacing(4);
        for (tab, tr) in tabs {
            let active = state.active_tab == tab;
            let btn = button(text(fluent.get(tr)).size(13))
                .on_press(Message::SelectDetailsTab(tab))
                .padding([6, 14])
                .style(theme::style::button::sidebar_icon(active));
            bar = bar.push(btn);
        }
        bar
    };

    let body: Element<'a, Message> = match task {
        None => container(
            column![]
                .spacing(16)
                .push(
                    text(fluent.get(Tr::TaskGone))
                        .size(14)
                        .style(theme::style::text::secondary),
                )
                .push(
                    button(text(fluent.get(Tr::CloseAbout)).size(13))
                        .on_press(Message::CloseTaskDetails)
                        .padding([6, 14])
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
            .spacing(8)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fixed(640.0))
    .height(Length::Fixed(480.0))
    .padding(20)
    .style(theme::style::card);

    overlay(panel)
}

fn key_value_row(key: String, value: String) -> Element<'static, Message> {
    row![]
        .push(
            text(key)
                .size(13)
                .style(theme::style::text::secondary)
                .width(Length::Fixed(140.0)),
        )
        .push(
            truncated_text(value)
                .size(13)
                .max_lines(2)
                .wrapping(text::Wrapping::Glyph),
        )
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

fn summary_tab<'a>(
    fluent: &'a Fluent,
    _theme: &'a iced::Theme,
    task: &'a DownloadTask,
) -> Element<'a, Message> {
    let gid_val = &task.gid;
    let name_val = &task.name;
    let dir_val = task.save_dir.to_string_lossy();
    let status_val = match task.status {
        crate::task::TaskStatus::Waiting => fluent.get(Tr::Waiting),
        crate::task::TaskStatus::Active => fluent.get(Tr::Active),
        crate::task::TaskStatus::Paused => fluent.get(Tr::Paused),
        crate::task::TaskStatus::Completed => fluent.get(Tr::Completed),
        crate::task::TaskStatus::Error => fluent.get(Tr::Error),
        crate::task::TaskStatus::Removed => fluent.get(Tr::Removed),
    };
    let time_val = format_add_time(task.added_at);

    column![
        key_value_row(fluent.get(Tr::FieldGid), gid_val.to_string()),
        key_value_row(fluent.get(Tr::FieldFileName), name_val.to_string()),
        key_value_row(fluent.get(Tr::FieldDownloadLocation), dir_val.to_string()),
        key_value_row(fluent.get(Tr::FieldTaskStatus), status_val),
        key_value_row(fluent.get(Tr::FieldAddedTime), time_val),
    ]
    .spacing(6)
    .width(Length::Fill)
    .into()
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
        .size(12)
        .style(text_secondary_fn);

        let piece_map_el = crate::ui::components::piece_map::view(
            details.bitfield.clone(),
            details.num_pieces,
            theme::success(theme),
            theme::text_secondary(theme),
        );

        let pct = task.progress_pct();
        let bar_color = match task.status {
            crate::task::TaskStatus::Paused => theme::warning(theme),
            crate::task::TaskStatus::Error => theme::danger(theme),
            _ => theme::success(theme),
        };
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
            column![piece_info, piece_map_el].spacing(4).into(),
            column![]
                .push(bar)
                .push(text(downloaded_text).size(12).style(text_secondary_fn))
                .spacing(4)
                .into(),
            column![]
                .push(
                    row![]
                        .push(
                            text(fluent.get(Tr::Speed))
                                .size(12)
                                .style(text_secondary_fn),
                        )
                        .push(text(speed_str).size(12).color(theme::success(theme)))
                        .spacing(4)
                        .align_y(Alignment::Center),
                )
                .push(
                    row![]
                        .push(
                            text(fluent.get(Tr::Connections))
                                .size(12)
                                .style(text_secondary_fn),
                        )
                        .push(text(conn_str).size(12))
                        .spacing(4)
                        .align_y(Alignment::Center),
                )
                .spacing(4)
                .into(),
        )
    } else {
        let loading_text = if state.loading {
            fluent.get(Tr::Loading)
        } else {
            fluent.get(Tr::TaskGone)
        };
        let empty: Element<'a, Message> =
            container(text(loading_text).size(14).style(text_secondary_fn))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        return empty;
    };

    column![piece_content, progress_section, info_section]
        .spacing(12)
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
    let bar_color = match task.status {
        crate::task::TaskStatus::Paused => theme::warning(theme),
        crate::task::TaskStatus::Error => theme::danger(theme),
        _ => theme::success(theme),
    };
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
    .size(12)
    .style(text_secondary_fn);

    let file_list: Element<'a, Message> = if let Some(ref details) = state.details {
        let mut col = column![].spacing(6);
        for file in &details.files {
            let basename: String = std::path::Path::new(&file.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&file.path)
                .to_string();

            let file_pct = if file.length == 0 {
                0.0
            } else {
                (file.completed_length as f64 / file.length as f64 * 100.0).min(100.0) as f32
            };
            let file_bar = progress_bar(0.0..=100.0, file_pct)
                .girth(Length::Fixed(6.0))
                .style(theme::style::progress::task(
                    if file.completed_length >= file.length {
                        theme::success(theme)
                    } else {
                        bar_color
                    },
                ));

            let file_row = column![]
                .spacing(2)
                .push(
                    row![]
                        .push(
                            text('\u{E0B4}')
                                .font(iced::Font::with_name("lucide"))
                                .size(13)
                                .style(text_secondary_fn),
                        )
                        .push(text(basename.clone()).size(13))
                        .push(iced::widget::Space::new().width(Length::Fill))
                        .push(
                            text(format_size(file.length))
                                .size(12)
                                .style(text_secondary_fn),
                        )
                        .spacing(4)
                        .align_y(Alignment::Center),
                )
                .push(file_bar)
                .push(
                    text(format!("{:.1}%", file_pct))
                        .size(11)
                        .style(text_secondary_fn),
                )
                .width(Length::Fill);
            col = col.push(file_row);
        }
        slim_scrollable(column![].push(col).spacing(6))
            .height(Length::Fill)
            .into()
    } else {
        container(
            text(fluent.get(Tr::Loading))
                .size(14)
                .style(text_secondary_fn),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    };

    column![]
        .push(overall_bar)
        .push(overall_info)
        .push(iced::widget::rule::horizontal(1))
        .push(file_list)
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
