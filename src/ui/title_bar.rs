use iced::widget::{button, container, mouse_area, row, text};
use iced::{Alignment, Color, Element, Length};

use crate::message::{Message, WindowCmd};
use crate::ui::icons::{CATEGORY_W, SIDEBAR_W};
use crate::ui::theme;

pub const BAR_HEIGHT: f32 = 38.0;

pub fn view<'a>(dark: bool, maximized: bool) -> Element<'a, Message> {
    let title_color = if dark {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_PRIMARY_LIGHT
    };
    let hover_color = Color::from_rgba(1.0, 1.0, 1.0, 0.12);
    let close_hover = Color::from_rgba(0.961, 0.263, 0.212, 0.85);

    let bg_sidebar = if dark {
        theme::BG_SIDEBAR
    } else {
        theme::BG_SIDEBAR_LIGHT
    };
    let bg_card = if dark {
        theme::BG_CARD
    } else {
        theme::BG_CARD_LIGHT
    };
    let bg_primary = if dark {
        theme::BG_PRIMARY
    } else {
        theme::BG_PRIMARY_LIGHT
    };

    let left_seg = container(
        mouse_area(
            container(text("").size(1))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::DragWindow),
    )
    .width(Length::Fixed(SIDEBAR_W))
    .height(Length::Fill)
    .style(move |_theme| container::Style {
        background: Some(bg_sidebar.into()),
        ..Default::default()
    });

    let mid_seg = container(
        mouse_area(
            container(text("").size(1))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Message::DragWindow),
    )
    .width(Length::Fixed(CATEGORY_W))
    .height(Length::Fill)
    .style(move |_theme| container::Style {
        background: Some(bg_card.into()),
        ..Default::default()
    });

    let win_btn_color = title_color;

    let min_btn = button(
        container(text("–").size(15).color(win_btn_color))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::WindowAction(WindowCmd::Minimize))
    .padding(0)
    .width(Length::Fixed(46.0))
    .height(Length::Fill)
    .style(move |_theme, status| button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(hover_color.into()),
            _ => None,
        },
        text_color: win_btn_color,
        border: iced::border::rounded(0),
        ..Default::default()
    });

    let max_glyph = if maximized { "❐" } else { "▢" };
    let max_btn = button(
        container(text(max_glyph).size(13).color(win_btn_color))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::WindowAction(WindowCmd::ToggleMaximize))
    .padding(0)
    .width(Length::Fixed(46.0))
    .height(Length::Fill)
    .style(move |_theme, status| button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(hover_color.into()),
            _ => None,
        },
        text_color: win_btn_color,
        border: iced::border::rounded(0),
        ..Default::default()
    });

    let close_btn = button(
        container(text("✕").size(14).color(win_btn_color))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::CloseRequested)
    .padding(0)
    .width(Length::Fixed(46.0))
    .height(Length::Fill)
    .style(move |_theme, status| button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(close_hover.into()),
            _ => None,
        },
        text_color: win_btn_color,
        border: iced::border::rounded(0),
        ..Default::default()
    });

    let right_seg = container(
        row![]
            .push(
                container(
                    mouse_area(
                        container(text("").size(1))
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .on_press(Message::DragWindow),
                )
                .width(Length::Fill)
                .height(Length::Fill),
            )
            .push(min_btn)
            .push(max_btn)
            .push(close_btn)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_theme| container::Style {
        background: Some(bg_primary.into()),
        ..Default::default()
    });

    container(
        row![]
            .push(left_seg)
            .push(mid_seg)
            .push(right_seg)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fixed(BAR_HEIGHT))
    .into()
}
