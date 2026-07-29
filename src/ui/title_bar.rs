use iced::widget::{button, container, mouse_area, row, text};
use iced::{Alignment, Element, Length};

use crate::message::{Message, WindowCmd};
use crate::ui::icon;
use crate::ui::icons::{CATEGORY_W, SIDEBAR_W};
use crate::ui::theme;

pub const BAR_HEIGHT: f32 = 38.0;

pub fn view<'a>(_theme: &iced::Theme, maximized: bool) -> Element<'a, Message> {
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
    .style(theme::style::sidebar_background);

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
    .style(theme::style::category_background);

    let min_btn = button(
        container(icon::minus().size(15).line_height(1.0))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::WindowAction(WindowCmd::Minimize))
    .padding(0)
    .width(Length::Fixed(46.0))
    .height(Length::Fill)
    .style(theme::style::button::window_control(false));

    let max_icon = if maximized {
        icon::copy()
    } else {
        icon::square()
    };
    let max_btn = button(
        container(max_icon.size(15).line_height(1.0))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::WindowAction(WindowCmd::ToggleMaximize))
    .padding(0)
    .width(Length::Fixed(46.0))
    .height(Length::Fill)
    .style(theme::style::button::window_control(false));

    let close_btn = button(
        container(icon::x().size(15).line_height(1.0))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::CloseRequested)
    .padding(0)
    .width(Length::Fixed(46.0))
    .height(Length::Fill)
    .style(theme::style::button::window_control(true));

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
    .style(theme::style::base_background);

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
