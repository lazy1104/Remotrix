use iced::widget::{column, container, row, text};
use iced::{Element, Length, Padding};

use crate::message::Message;
use crate::task::format_speed;
use crate::ui::icon;
use crate::ui::theme;

pub fn view<'a>(
    theme: &'a iced::Theme,
    active: bool,
    download: u64,
    upload: u64,
) -> Element<'a, Message> {
    let palette = theme.extended_palette();
    let strong = palette.background.strong.color;
    let primary = palette.primary.base.color;

    if !active && download == 0 && upload == 0 {
        container(icon::download().size(18).color(strong))
            .center_x(Length::Fixed(44.0))
            .center_y(Length::Fixed(44.0))
            .style(theme::style::speed_hud_background)
            .into()
    } else {
        let icon_col =
            container(icon::download().size(18).color(strong)).center_x(Length::Fixed(44.0));

        let up_row = row![
            icon::arrow_up().size(12).color(strong),
            text(format_speed(upload)).size(12).color(strong),
        ]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center);

        let down_row = row![
            icon::download_arrow().size(12).color(primary),
            text(format_speed(download)).size(12).color(primary),
        ]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center);

        let block = column![up_row, down_row].spacing(2);

        let content = row![icon_col, block]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center);

        container(content)
            .padding(Padding {
                top: 8.0,
                right: 12.0,
                bottom: 8.0,
                left: 12.0,
            })
            .style(theme::style::speed_hud_background)
            .into()
    }
}
