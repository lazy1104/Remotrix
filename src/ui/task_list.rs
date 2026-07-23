use iced::widget::{
    button, column, combo_box, container, progress_bar, row, scrollable, text, tooltip,
};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, SortField};
use crate::task::{format_duration, format_size, format_speed, DownloadTask, TaskStatus};
use crate::ui::theme;

pub fn view<'a>(
    fluent: &'a Fluent,
    dark: bool,
    tasks: &[DownloadTask],
    sort_combo_state: &'a combo_box::State<SortField>,
) -> Element<'a, Message> {
    let text_primary = if dark {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_PRIMARY_LIGHT
    };
    let text_secondary = if dark {
        theme::TEXT_SECONDARY
    } else {
        theme::TEXT_SECONDARY_LIGHT
    };
    let bg_card = if dark {
        theme::BG_CARD
    } else {
        theme::BG_CARD_LIGHT
    };
    let progress_track = if dark {
        theme::BG_PRIMARY
    } else {
        theme::BORDER_LIGHT
    };

    let combo_w = combo_box::ComboBox::new(
        sort_combo_state,
        "",
        None::<&SortField>,
        Message::SortSelected,
    )
    .size(13.0)
    .width(Length::Fixed(130.0));

    let lucide_font = iced::Font::with_name("lucide");

    let toolbar_btn = |codepoint: char, tip: String, msg: Message| -> Element<'a, Message> {
        let glyph = text(codepoint.to_string()).font(lucide_font).size(15);
        let btn = button(glyph)
            .on_press(msg)
            .padding([6_u16, 8])
            .style(button::text);
        tooltip(btn, text(tip), tooltip::Position::Bottom)
            .style(container::rounded_box)
            .into()
    };

    let toolbar = row![]
        .push(
            row![]
                .push(toolbar_btn(
                    '\u{E145}',
                    fluent.get(Tr::Refresh),
                    Message::Refresh,
                ))
                .push(combo_w)
                .push(toolbar_btn(
                    '\u{E13C}',
                    fluent.get(Tr::StartAll),
                    Message::StartAll,
                ))
                .push(toolbar_btn(
                    '\u{E12E}',
                    fluent.get(Tr::PauseAll),
                    Message::PauseAll,
                ))
                .push(toolbar_btn(
                    '\u{E18E}',
                    fluent.get(Tr::DeleteAll),
                    Message::DeleteAll,
                ))
                .push(toolbar_btn(
                    '\u{E28F}',
                    fluent.get(Tr::ClearList),
                    Message::ClearCompleted,
                ))
                .align_y(Alignment::Center)
                .spacing(4),
        )
        .push(iced::widget::Space::new().width(Length::Fill))
        .width(Length::Fill)
        .padding(iced::Padding::new(0.0).bottom(12.0));

    if tasks.is_empty() {
        let empty = container(
            column![]
                .spacing(8)
                .push(text(fluent.get(Tr::NoTasks)).size(18).color(text_secondary))
                .push(
                    text(fluent.get(Tr::NoTasksHint))
                        .size(13)
                        .color(text_secondary),
                ),
        )
        .center_x(Length::Fill)
        .width(Length::Fill)
        .padding([80_u16, 0]);

        return container(column![].push(toolbar).push(empty))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([24_u16, 28])
            .into();
    }

    let mut list = column![].spacing(10);

    for t in tasks {
        list = list.push(task_card(
            fluent,
            t,
            text_primary,
            text_secondary,
            bg_card,
            progress_track,
        ));
    }

    let body = scrollable(column![].spacing(10).push(list)).height(Length::Fill);

    container(column![].push(toolbar).push(body))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([24_u16, 28])
        .into()
}

#[allow(clippy::too_many_arguments)]
fn task_card<'a>(
    fluent: &'a Fluent,
    t: &DownloadTask,
    text_primary: iced::Color,
    text_secondary: iced::Color,
    bg_card: iced::Color,
    progress_track: iced::Color,
) -> Element<'a, Message> {
    let pct = t.progress_pct();
    let name = text(t.name.clone()).size(15).color(text_primary);

    let meta_left = format!(
        "{} / {}",
        format_size(t.downloaded),
        if t.total == 0 {
            "—".to_string()
        } else {
            format_size(t.total)
        }
    );
    let speed_text = if t.speed > 0 {
        format_speed(t.speed)
    } else {
        "—".to_string()
    };
    let eta_text = match t.eta_secs() {
        Some(s) => format_duration(s),
        None => "—".to_string(),
    };
    let status_str = match t.status {
        TaskStatus::Waiting => fluent.get(Tr::Waiting),
        TaskStatus::Active => fluent.get(Tr::Active),
        TaskStatus::Paused => fluent.get(Tr::Paused),
        TaskStatus::Completed => fluent.get(Tr::Completed),
        TaskStatus::Error => fluent.get(Tr::Error),
        TaskStatus::Removed => fluent.get(Tr::Removed),
    }
    .to_string();

    let status_color = match t.status {
        TaskStatus::Active => theme::PROGRESS,
        TaskStatus::Paused => theme::PAUSED,
        TaskStatus::Completed => theme::PROGRESS,
        TaskStatus::Error => theme::ERROR,
        _ => text_secondary,
    };

    let sep1 = text("  ·  ").size(12).color(text_secondary);
    let sep2 = text("  ·  ").size(12).color(text_secondary);

    let meta = row![]
        .push(text(meta_left).size(12).color(text_secondary))
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(text(speed_text).size(12).color(theme::SPEED))
        .push(sep1)
        .push(text(eta_text).size(12).color(text_secondary))
        .push(sep2)
        .push(text(status_str).size(12).color(status_color))
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let bar_color = match t.status {
        TaskStatus::Paused => theme::PAUSED,
        TaskStatus::Error => theme::ERROR,
        _ => theme::PROGRESS,
    };
    let bar = progress_bar(0.0..=100.0, pct)
        .girth(Length::Fixed(8.0))
        .style(move |_theme| iced::widget::progress_bar::Style {
            background: iced::Background::Color(progress_track),
            bar: iced::Background::Color(bar_color),
            border: iced::border::rounded(4),
        });

    let mut actions = row![].spacing(8);
    match t.status {
        TaskStatus::Active | TaskStatus::Waiting => {
            actions = actions.push(
                button(text(fluent.get(Tr::Pause)).size(12))
                    .on_press(Message::PauseTask(t.gid.clone()))
                    .padding([6, 12])
                    .style(button::secondary),
            );
        }
        TaskStatus::Paused => {
            actions = actions.push(
                button(text(fluent.get(Tr::Resume)).size(12))
                    .on_press(Message::ResumeTask(t.gid.clone()))
                    .padding([6, 12])
                    .style(button::secondary),
            );
        }
        _ => {}
    }
    if !matches!(t.status, TaskStatus::Removed) {
        actions = actions.push(
            button(text(fluent.get(Tr::Remove)).size(12))
                .on_press(Message::RemoveTask(t.gid.clone()))
                .padding([6, 12])
                .style(button::danger),
        );
    }
    let pct_text = format!("{:.1}%", pct);
    actions = actions.push(iced::widget::Space::new().width(Length::Fill));
    actions = actions.push(text(pct_text).size(12).color(text_secondary));

    let content = column![]
        .spacing(8)
        .push(row![name, iced::widget::Space::new().width(Length::Fill)].align_y(Alignment::Center))
        .push(bar)
        .push(meta)
        .push(actions.align_y(Alignment::Center))
        .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .padding(16)
        .style(move |_theme| container::Style {
            background: Some(bg_card.into()),
            border: iced::border::rounded(8),
            ..Default::default()
        })
        .into()
}
