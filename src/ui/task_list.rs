use iced::widget::{button, column, container, progress_bar, row, scrollable, text, tooltip};
use iced::{Alignment, Element, Length};
use iced_aw::widget::drop_down;

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, SortField, SortOrder};
use crate::task::{format_duration, format_size, format_speed, DownloadTask, TaskStatus};
use crate::ui::theme;

pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &iced::Theme,
    tasks: &[DownloadTask],
    sort_field: SortField,
    sort_order: SortOrder,
    sort_menu_open: bool,
) -> Element<'a, Message> {
    let lucide_font = iced::Font::with_name("lucide");

    let toolbar_btn =
        |codepoint: char, tip: String, msg: Message, active: bool| -> Element<'a, Message> {
            let glyph = text(codepoint.to_string()).font(lucide_font).size(15);
            let glyph = if active {
                glyph.color(theme::accent(theme))
            } else {
                glyph
            };
            let btn = button(glyph)
                .on_press(msg)
                .padding([6_u16, 8])
                .style(button::text);
            tooltip(btn, text(tip), tooltip::Position::Bottom)
                .style(container::rounded_box)
                .into()
        };

    let sort_active = sort_menu_open || sort_field != SortField::AddedTime;

    let sort_underlay = {
        let glyph = text('\u{E37D}'.to_string()).font(lucide_font).size(15);
        let glyph = if sort_active {
            glyph.color(theme::accent(theme))
        } else {
            glyph
        };
        button(glyph)
            .on_press(Message::ToggleSortMenu)
            .padding([6_u16, 8])
            .style(button::text)
    };

    let sort_overlay: Element<'a, Message> = {
        let asc_desc_label = fluent.get(if sort_order == SortOrder::Desc {
            Tr::SortDesc
        } else {
            Tr::SortAsc
        });
        let asc_desc_btn = button(text(asc_desc_label).size(13))
            .on_press(Message::ToggleSortOrder)
            .width(Length::Fill)
            .padding([6, 8])
            .style(button::text);

        let mut col = column![asc_desc_btn].spacing(2).width(Length::Fill);
        col = col.push(iced::widget::rule::horizontal(1));

        let fields = [
            (SortField::AddedTime, Tr::SortByAdded),
            (SortField::Name, Tr::SortByName),
            (SortField::Size, Tr::SortBySize),
            (SortField::Progress, Tr::SortByProgress),
            (SortField::Status, Tr::SortByStatus),
        ];

        for (field, tr) in fields {
            let selected = field == sort_field;
            let btn = button(text(fluent.get(tr)).size(13))
                .on_press(Message::SortSelected(field))
                .width(Length::Fill)
                .padding([6, 8])
                .style(theme::style::button::sidebar_icon(selected));
            col = col.push(btn);
        }

        container(col).padding(6).style(theme::style::card).into()
    };

    let sort_dropdown = drop_down::DropDown::new(sort_underlay, sort_overlay, sort_menu_open)
        .on_dismiss(Message::CloseSortMenu)
        .width(Length::Fixed(170.0));

    let new_btn: Element<'a, Message> = {
        let glyph = text('\u{E13D}'.to_string()).font(lucide_font).size(15);
        let inner = container(glyph).center(Length::Fixed(28.0));
        let btn = button(inner)
            .on_press(Message::OpenAddDialog)
            .padding(0)
            .style(theme::style::button::new_download());
        tooltip(
            btn,
            text(fluent.get(Tr::NewDownload)),
            tooltip::Position::Bottom,
        )
        .style(container::rounded_box)
        .into()
    };

    let toolbar = row![]
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(new_btn)
        .push(
            row![]
                .push(toolbar_btn(
                    '\u{E145}',
                    fluent.get(Tr::Refresh),
                    Message::Refresh,
                    false,
                ))
                .push(sort_dropdown)
                .push(toolbar_btn(
                    '\u{E13C}',
                    fluent.get(Tr::StartAll),
                    Message::StartAll,
                    false,
                ))
                .push(toolbar_btn(
                    '\u{E12E}',
                    fluent.get(Tr::PauseAll),
                    Message::PauseAll,
                    false,
                ))
                .push(toolbar_btn(
                    '\u{E18E}',
                    fluent.get(Tr::DeleteAll),
                    Message::DeleteAll,
                    false,
                ))
                .push(toolbar_btn(
                    '\u{E28F}',
                    fluent.get(Tr::ClearList),
                    Message::ClearCompleted,
                    false,
                ))
                .align_y(Alignment::Center)
                .spacing(4),
        )
        .width(Length::Fill)
        .padding(iced::Padding::new(0.0).bottom(12.0));

    if tasks.is_empty() {
        let empty = container(
            column![]
                .spacing(8)
                .push(
                    text(fluent.get(Tr::NoTasks))
                        .size(18)
                        .style(theme::style::text::secondary),
                )
                .push(
                    text(fluent.get(Tr::NoTasksHint))
                        .size(13)
                        .style(theme::style::text::secondary),
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
        list = list.push(task_card(fluent, theme, t));
    }

    let body = scrollable(column![].spacing(10).push(list)).height(Length::Fill);

    container(column![].push(toolbar).push(body))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([24_u16, 28])
        .into()
}

fn task_card<'a>(
    fluent: &'a Fluent,
    theme: &iced::Theme,
    t: &DownloadTask,
) -> Element<'a, Message> {
    let text_secondary = theme::text_secondary(theme);
    let pct = t.progress_pct();
    let name = text(t.name.clone()).size(15);

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
        TaskStatus::Active => theme::success(theme),
        TaskStatus::Paused => theme::warning(theme),
        TaskStatus::Completed => theme::success(theme),
        TaskStatus::Error => theme::danger(theme),
        _ => text_secondary,
    };

    let sep1 = text("  ·  ").size(12).style(theme::style::text::secondary);
    let sep2 = text("  ·  ").size(12).style(theme::style::text::secondary);

    let meta = row![]
        .push(
            text(meta_left)
                .size(12)
                .style(theme::style::text::secondary),
        )
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(text(speed_text).size(12).color(theme::success(theme)))
        .push(sep1)
        .push(text(eta_text).size(12).style(theme::style::text::secondary))
        .push(sep2)
        .push(text(status_str).size(12).color(status_color))
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let bar_color = match t.status {
        TaskStatus::Paused => theme::warning(theme),
        TaskStatus::Error => theme::danger(theme),
        _ => theme::success(theme),
    };
    let bar = progress_bar(0.0..=100.0, pct)
        .girth(Length::Fixed(8.0))
        .style(theme::style::progress::task(bar_color));

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
    actions = actions.push(text(pct_text).size(12).style(theme::style::text::secondary));

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
        .style(theme::style::card)
        .into()
}
