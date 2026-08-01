use iced::widget::{column, container, row, text};
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
    let strong = palette.background.strong.color;
    let primary = palette.primary.base.color;

    if !active && download == 0 && upload == 0 {
        container(icon::download().size(FONT_HERO).color(strong))
            .center_x(Length::Fixed(44.0))
            .center_y(Length::Fixed(44.0))
            .style(theme::style::speed_hud_background)
            .into()
    } else {
        let icon_col =
            container(icon::download().size(FONT_HERO).color(strong)).center_x(Length::Fixed(44.0));

        let up_row = row![
            icon::arrow_up().size(FONT_SMALL).color(strong),
            text(format_speed(upload)).size(FONT_SMALL).color(strong),
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

        container(content)
            .padding(PADDING_HUD)
            .style(theme::style::speed_hud_background)
            .into()
    }
}
