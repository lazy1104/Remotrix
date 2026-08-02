use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

use crate::message::Message;
use crate::task::format_speed;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

pub fn view<'a>(
    theme: &'a iced::Theme,
    active: bool,
    download: u64,
    upload: u64,
) -> Element<'a, Message> {
    let palette = theme.extended_palette();
    let primary_weak = palette.primary.weak.color;
    let primary = palette.primary.base.color;

    if !active && download == 0 && upload == 0 {
        button(icon::download().size(FONT_HERO).color(primary_weak))
            .on_press(Message::Noop)
            .padding(iced::Padding::ZERO)
            .width(Length::Fixed(44.0))
            .height(Length::Fixed(44.0))
            .style(theme::style::button::speed_hud())
            .into()
    } else {
        let icon_col = container(icon::download().size(FONT_HERO).color(primary_weak))
            .center_x(Length::Fixed(44.0));

        let up_row = row![
            icon::arrow_up().size(FONT_SMALL).color(primary_weak),
            text(format_speed(upload))
                .size(FONT_SMALL)
                .color(primary_weak),
        ]
        .spacing(SPACE_SM)
        .align_y(iced::alignment::Vertical::Center);

        let down_row = row![
            icon::download_arrow().size(FONT_SMALL).color(primary),
            text(format_speed(download)).size(FONT_SMALL).color(primary),
        ]
        .spacing(SPACE_SM)
        .align_y(iced::alignment::Vertical::Center);

        let block = column![up_row, down_row].spacing(SPACE_XS);

        let content = row![icon_col, block]
            .spacing(SPACE_LG)
            .align_y(iced::alignment::Vertical::Center);

        button(content)
            .on_press(Message::Noop)
            .padding(PADDING_HUD)
            .style(theme::style::button::speed_hud())
            .into()
    }
}
