use crate::ui::components::drop_down;
use iced::widget::{
    button, column, container, mouse_area, progress_bar, row, text, text_input, tooltip,
};
use iced::{mouse, Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{ConfirmAction, Message, SortField, SortOrder};
use crate::task::{format_duration, format_size, format_speed, DownloadTask, TaskStatus};
use crate::ui::components::slim_scrollable::slim_scrollable;
use crate::ui::components::tooltip as tip;
use crate::ui::components::truncated_text::truncated_text;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &iced::Theme,
    tasks: &[DownloadTask],
    sort_field: SortField,
    sort_order: SortOrder,
    sort_menu_open: bool,
    search_query: &str,
) -> Element<'a, Message> {
    let lucide_font = iced::Font::with_name("lucide");

    let toolbar_btn =
        |codepoint: char, tip: String, msg: Message, active: bool| -> Element<'a, Message> {
            let glyph = text(codepoint.to_string())
                .font(lucide_font)
                .size(FONT_ICON);
            let glyph = if active {
                glyph.color(theme::accent(theme))
            } else {
                glyph
            };
            let btn = button(glyph)
                .on_press(msg)
                .padding(PADDING_BUTTON_XS)
                .style(theme::style::button::toolbar_icon(active));
            tip::standard(btn, text(tip).size(FONT_SMALL), tooltip::Position::Bottom)
        };

    let sort_active = sort_menu_open || sort_field != SortField::AddedTime;

    let sort_underlay = {
        let glyph = text('\u{E37D}'.to_string())
            .font(lucide_font)
            .size(FONT_ICON);
        let glyph = if sort_active {
            glyph.color(theme::accent(theme))
        } else {
            glyph
        };
        button(glyph)
            .on_press(Message::ToggleSortMenu)
            .padding(PADDING_BUTTON_XS)
            .style(theme::style::button::toolbar_icon(sort_active))
    };

    let sort_overlay: Element<'a, Message> = {
        let asc_desc_label = fluent.get(if sort_order == SortOrder::Desc {
            Tr::SortDesc
        } else {
            Tr::SortAsc
        });
        let asc_desc_btn = button(text(asc_desc_label).size(FONT_MEDIUM))
            .on_press(Message::ToggleSortOrder)
            .width(Length::Fill)
            .padding(PADDING_BUTTON_XS)
            .style(theme::style::button::text());

        let mut col = column![asc_desc_btn].spacing(SPACE_XS).width(Length::Fill);
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
            let btn = button(text(fluent.get(tr)).size(FONT_MEDIUM))
                .on_press(Message::SortSelected(field))
                .width(Length::Fill)
                .padding(PADDING_BUTTON_XS)
                .style(theme::style::button::sidebar_icon(selected));
            col = col.push(btn);
        }

        container(col)
            .padding(PADDING_DROPDOWN)
            .style(theme::style::card)
            .into()
    };

    let sort_field_label = match sort_field {
        SortField::AddedTime => Tr::SortByAdded,
        SortField::Name => Tr::SortByName,
        SortField::Size => Tr::SortBySize,
        SortField::Progress => Tr::SortByProgress,
        SortField::Status => Tr::SortByStatus,
    };
    let sort_order_label = match sort_order {
        SortOrder::Asc => Tr::SortAsc,
        SortOrder::Desc => Tr::SortDesc,
    };
    let sort_tip = format!(
        "{}: {} · {}",
        fluent.get(Tr::Sort),
        fluent.get(sort_field_label),
        fluent.get(sort_order_label)
    );

    let sort_dropdown = drop_down::DropDown::new(
        tip::standard(
            sort_underlay,
            text(sort_tip).size(FONT_SMALL),
            tooltip::Position::Bottom,
        ),
        sort_overlay,
        sort_menu_open,
    )
    .on_dismiss(Message::CloseSortMenu)
    .width(Length::Fixed(170.0));

    let new_btn: Element<'a, Message> = {
        let glyph = text('\u{E13D}'.to_string())
            .font(lucide_font)
            .size(FONT_ICON);
        let btn = button(glyph)
            .on_press(Message::OpenAddDialog)
            .padding(PADDING_BUTTON_XS)
            .style(theme::style::button::toolbar_icon(true));
        tip::standard(
            btn,
            text(fluent.get(Tr::NewDownload)).size(FONT_SMALL),
            tooltip::Position::Bottom,
        )
    };

    let has_query = !search_query.trim().is_empty();

    let search_input = theme::input_layout(
        text_input(&fluent.get(Tr::Search), search_query)
            .on_input(Message::SearchChanged)
            .width(Length::Fixed(220.0))
            .style(theme::style::input::standard),
    );

    let mut search_group = row![search_input]
        .spacing(SPACE_SM)
        .align_y(Alignment::Center);
    if has_query {
        search_group = search_group.push(
            button(icon::x().size(FONT_ICON))
                .on_press(Message::SearchChanged(String::new()))
                .padding(PADDING_BUTTON_XS)
                .style(theme::style::button::toolbar_icon(false)),
        );
    }

    let toolbar = row![]
        .push(search_group)
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
                    Message::RequestConfirm(ConfirmAction::DeleteAll),
                    false,
                ))
                .push(toolbar_btn(
                    '\u{E28F}',
                    fluent.get(Tr::ClearList),
                    Message::RequestConfirm(ConfirmAction::ClearCompleted),
                    false,
                ))
                .align_y(Alignment::Center)
                .spacing(SPACE_SM),
        )
        .align_y(Alignment::Center)
        .spacing(SPACE_SM)
        .width(Length::Fill)
        .padding(PADDING_BOTTOM_TOOLBAR);

    if tasks.is_empty() {
        let mut empty_col = column![].spacing(SPACE_LG).push(
            text(fluent.get(if has_query {
                Tr::NoResults
            } else {
                Tr::NoTasks
            }))
            .size(FONT_HERO)
            .style(theme::style::text::secondary),
        );
        if !has_query {
            empty_col = empty_col.push(
                text(fluent.get(Tr::NoTasksHint))
                    .size(FONT_MEDIUM)
                    .style(theme::style::text::secondary),
            );
        }
        let empty = container(empty_col)
            .center_x(Length::Fill)
            .width(Length::Fill)
            .padding(PADDING_EMPTY_STATE);

        return container(column![].push(toolbar).push(empty))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(PADDING_PAGE)
            .into();
    }

    let mut list = column![].spacing(SPACE_XL);

    for t in tasks {
        list = list.push(task_card(fluent, theme, t));
    }

    let body = slim_scrollable(column![].spacing(SPACE_XL).push(list)).height(Length::Fill);

    container(column![].push(toolbar).push(body))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(PADDING_PAGE)
        .into()
}

fn task_card<'a>(
    fluent: &'a Fluent,
    theme: &iced::Theme,
    t: &DownloadTask,
) -> Element<'a, Message> {
    let text_secondary = theme::text_secondary(theme);
    let pct = t.progress_pct();
    let name = mouse_area(tip::standard(
        truncated_text(t.name.clone())
            .size(FONT_ICON)
            .max_lines(2)
            .wrapping(text::Wrapping::Glyph),
        text(t.name.clone()).size(FONT_SMALL),
        tooltip::Position::Bottom,
    ))
    .on_double_click(Message::OpenTaskFile(t.gid.clone()))
    .interaction(mouse::Interaction::Pointer);

    let toolbar_icon = |glyph: iced::widget::Text<'a>,
                        msg: Option<Message>,
                        tip_label: String|
     -> Element<'a, Message> {
        let btn = match msg {
            Some(m) => button(glyph)
                .on_press(m)
                .padding(PADDING_ICON_BTN)
                .style(theme::style::button::toolbar_icon(false)),
            None => button(glyph)
                .padding(PADDING_ICON_BTN)
                .style(theme::style::button::toolbar_icon(false)),
        };
        tip::standard(
            btn,
            text(tip_label).size(FONT_SMALL),
            tooltip::Position::Bottom,
        )
    };

    let open_btn: Element<'a, Message> = {
        let glyph = icon::external_link().size(FONT_ICON).color(text_secondary);
        tip::standard(
            button(glyph)
                .on_press(Message::OpenTaskFile(t.gid.clone()))
                .padding(PADDING_ICON_BTN)
                .style(theme::style::button::toolbar_icon(false)),
            text(fluent.get(Tr::Open)).size(FONT_SMALL),
            tooltip::Position::Bottom,
        )
    };

    let pause_resume_btn: Element<'a, Message> = match t.status {
        TaskStatus::Active | TaskStatus::Waiting => toolbar_icon(
            icon::pause().size(FONT_ICON).color(text_secondary),
            Some(Message::PauseTask(t.gid.clone())),
            fluent.get(Tr::Pause),
        ),
        TaskStatus::Paused => toolbar_icon(
            icon::play().size(FONT_ICON).color(text_secondary),
            Some(Message::ResumeTask(t.gid.clone())),
            fluent.get(Tr::Resume),
        ),
        TaskStatus::Completed => {
            let can_redownload = !t.url.is_empty() || t.info_hash.is_some();
            let btn = button(icon::refresh().size(FONT_ICON).color(text_secondary))
                .padding(PADDING_ICON_BTN)
                .style(theme::style::button::toolbar_icon(false));
            let btn = if can_redownload {
                btn.on_press(Message::RedownloadTask(t.gid.clone()))
            } else {
                btn
            };
            tip::standard(
                btn,
                text(fluent.get(Tr::ReDownload)).size(FONT_SMALL),
                tooltip::Position::Bottom,
            )
        }
        _ => toolbar_icon(
            icon::pause().size(FONT_ICON).color(text_secondary),
            None,
            fluent.get(Tr::Pause),
        ),
    };

    let show_in_folder_btn: Element<'a, Message> = if !t.save_dir.as_os_str().is_empty() {
        let glyph = icon::folder_open().size(FONT_ICON).color(text_secondary);
        tip::standard(
            button(glyph)
                .on_press(Message::OpenTaskFolder(t.gid.clone()))
                .padding(PADDING_ICON_BTN)
                .style(theme::style::button::toolbar_icon(false)),
            text(fluent.get(Tr::ShowInFolder)).size(FONT_SMALL),
            tooltip::Position::Bottom,
        )
    } else {
        let glyph = icon::folder_open().size(FONT_ICON).color(text_secondary);
        tip::standard(
            button(glyph)
                .padding(PADDING_ICON_BTN)
                .style(theme::style::button::toolbar_icon(false)),
            text(fluent.get(Tr::ShowInFolder)).size(FONT_SMALL),
            tooltip::Position::Bottom,
        )
    };

    let copy_link_btn: Element<'a, Message> = if !t.url.is_empty() || t.info_hash.is_some() {
        let glyph = icon::copy().size(FONT_ICON).color(text_secondary);
        tip::standard(
            button(glyph)
                .on_press(Message::CopyTaskLink(t.gid.clone()))
                .padding(PADDING_ICON_BTN)
                .style(theme::style::button::toolbar_icon(false)),
            text(fluent.get(Tr::CopyLink)).size(FONT_SMALL),
            tooltip::Position::Bottom,
        )
    } else {
        let glyph = icon::copy().size(FONT_ICON).color(text_secondary);
        tip::standard(
            button(glyph)
                .padding(PADDING_ICON_BTN)
                .style(theme::style::button::toolbar_icon(false)),
            text(fluent.get(Tr::CopyLink)).size(FONT_SMALL),
            tooltip::Position::Bottom,
        )
    };

    let details_btn: Element<'a, Message> = {
        let glyph = icon::circle_alert().size(FONT_ICON).color(text_secondary);
        tip::standard(
            button(glyph)
                .on_press(Message::OpenTaskDetails(t.gid.clone()))
                .padding(PADDING_ICON_BTN)
                .style(theme::style::button::toolbar_icon(false)),
            text(fluent.get(Tr::Details)).size(FONT_SMALL),
            tooltip::Position::Bottom,
        )
    };

    let delete_btn: Element<'a, Message> = {
        let glyph = icon::trash().size(FONT_ICON).color(text_secondary);
        tip::standard(
            button(glyph)
                .on_press(Message::RequestConfirm(ConfirmAction::DeleteTask(
                    t.gid.clone(),
                )))
                .padding(PADDING_ICON_BTN)
                .style(theme::style::button::toolbar_icon(false)),
            text(fluent.get(Tr::Delete)).size(FONT_SMALL),
            tooltip::Position::Bottom,
        )
    };

    let toolbar = container(
        row![]
            .push(open_btn)
            .push(pause_resume_btn)
            .push(show_in_folder_btn)
            .push(copy_link_btn)
            .push(details_btn)
            .push(delete_btn)
            .spacing(SPACE_SM)
            .align_y(Alignment::Center),
    )
    .padding(PADDING_TOOLBAR_CAPSULE)
    .style(theme::style::toolbar_capsule);

    let bar_color = match t.status {
        TaskStatus::Paused => theme::primary_weak(theme),
        TaskStatus::Error => theme::danger(theme),
        TaskStatus::Completed => theme::success(theme),
        _ => theme::primary(theme),
    };
    let bar = progress_bar(0.0..=100.0, pct)
        .girth(Length::Fixed(8.0))
        .style(theme::style::progress::task(bar_color));

    let downloaded_text = format!(
        "{} / {}",
        format_size(t.downloaded),
        if t.total == 0 {
            "—".to_string()
        } else {
            format_size(t.total)
        }
    );

    let speed_text = if t.is_download_active() || t.speed > 0 {
        format_speed(t.speed)
    } else {
        "—".to_string()
    };
    let eta_text = match t.eta_secs() {
        Some(s) => format_duration(s),
        None => "—".to_string(),
    };
    let conn_text = if t.is_download_active() || t.status == TaskStatus::Completed {
        t.connections.to_string()
    } else {
        "0".to_string()
    };

    let sep = || {
        text("  ·  ")
            .size(FONT_SMALL)
            .style(theme::style::text::secondary)
    };

    let row3 = row![]
        .push(
            text(downloaded_text)
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .push(iced::widget::Space::new().width(Length::Fill))
        .push(
            text(eta_text)
                .size(FONT_SMALL)
                .style(theme::style::text::secondary),
        )
        .push(sep())
        .push(
            text(speed_text)
                .size(FONT_SMALL)
                .color(theme::success(theme)),
        )
        .push(sep())
        .push(icon::connections().size(FONT_SMALL).color(text_secondary))
        .push(text(conn_text).size(FONT_SMALL))
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let name_marker: Element<'a, Message> = if t.status == TaskStatus::Completed {
        row![
            icon::circle_check()
                .size(FONT_ICON)
                .color(theme::success(theme)),
            name,
        ]
        .spacing(SPACE_SM)
        .align_y(Alignment::Center)
        .into()
    } else {
        name.into()
    };

    let content = column![]
        .spacing(SPACE_LG)
        .push(
            row![name_marker, toolbar]
                .align_y(iced::alignment::Vertical::Top)
                .spacing(SPACE_2XL),
        )
        .push(bar)
        .push(row3)
        .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .padding(PADDING_CARD)
        .style(theme::style::card)
        .into()
}
