use std::path::PathBuf;

use iced::widget::{button, column, container, row, text, text_input, tooltip, Space, Text};
use iced::{Alignment, Element, Length};

use iced_aw::widget::drop_down;

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, PathPickerId};
use crate::ui::icon;
use crate::ui::theme;

fn icon_content<'a>(icon: Text<'a>) -> Element<'a, Message> {
    container(icon.line_height(1.0))
        .center_y(Length::Fill)
        .into()
}

fn separator() -> Element<'static, Message> {
    container(Space::new())
        .width(Length::Fixed(1.0))
        .height(Length::Fill)
        .style(theme::style::separator)
        .into()
}

pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    value: &str,
    id: Option<PathPickerId>,
    show_history: bool,
    history_open: bool,
    history: &'a [String],
) -> Element<'a, Message> {
    let text_secondary = crate::ui::theme::text_secondary(theme);
    let mut row = row![]
        .spacing(0)
        .align_y(Alignment::Center)
        .height(Length::Fill);

    let input = text_input("", value)
        .style(theme::style::input::grouped)
        .width(Length::Fill)
        .padding([0, 10])
        .size(13);
    row = row.push(input);
    row = row.push(separator());

    let copy_btn: Element<'a, Message> = {
        let mut btn = button(icon_content(icon::copy().size(15).color(text_secondary)))
            .style(theme::style::button::grouped_icon(false))
            .height(Length::Fill);
        if !value.is_empty() {
            btn = btn.on_press(Message::CopyPath(value.to_string()));
        }
        tooltip(btn, text(fluent.get(Tr::Copy)), tooltip::Position::Bottom)
            .style(container::rounded_box)
            .into()
    };
    row = row.push(copy_btn);
    row = row.push(separator());

    if let Some(pid) = id {
        let browse_btn: Element<'a, Message> = tooltip(
            button(icon_content(
                icon::folder_open().size(15).color(text_secondary),
            ))
            .on_press(Message::BrowsePath(pid))
            .style(theme::style::button::grouped_icon(false))
            .height(Length::Fill),
            text(fluent.get(Tr::Browse)),
            tooltip::Position::Bottom,
        )
        .style(container::rounded_box)
        .into();
        row = row.push(browse_btn);

        if show_history {
            row = row.push(separator());
            if history.is_empty() {
                let disabled_btn = button(icon_content(
                    icon::folder_clock().size(15).color(text_secondary),
                ))
                .style(theme::style::button::grouped_icon(true))
                .height(Length::Fill);
                row = row.push(disabled_btn);
            } else {
                let trailing_btn = button(icon_content(
                    icon::folder_clock().size(15).color(text_secondary),
                ))
                .on_press(Message::TogglePathHistory(pid))
                .style(theme::style::button::grouped_icon(true))
                .height(Length::Fill);
                row = row.push(trailing_btn);
            }
        }
    }

    let group = container(row)
        .width(Length::Fill)
        .height(Length::Fixed(36.0))
        .padding(1.0)
        .style(theme::style::grouped_frame);

    if let Some(pid) = id {
        if show_history && !history.is_empty() {
            let overlay_items: Vec<Element<'a, Message>> = history
                .iter()
                .map(|p| {
                    button(text(p.as_str()).size(12))
                        .on_press(Message::SelectPathHistory(pid, PathBuf::from(p.clone())))
                        .width(Length::Fill)
                        .padding([6, 8])
                        .style(theme::style::button::text())
                        .into()
                })
                .collect();

            let overlay = container(column(overlay_items).spacing(2).width(Length::Fill))
                .padding(6)
                .style(theme::style::card);

            return drop_down::DropDown::new(group, overlay, history_open)
                .on_dismiss(Message::ClosePathHistory)
                .into();
        }
    }

    group.into()
}
