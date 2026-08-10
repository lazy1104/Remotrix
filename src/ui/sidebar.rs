use iced::widget::{button, column, container, text, Text};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{AddMsg, DialogMsg, Message, NavMsg, Page};
use crate::ui::components::logo;
use crate::ui::components::tooltip;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

pub fn view<'a>(fluent: &'a Fluent, theme: &'a iced::Theme, _page: Page) -> Element<'a, Message> {
    let logo = container(logo::view(theme, SIDEBAR_LOGO_W, SIDEBAR_LOGO_H))
        .center_x(Length::Fill)
        .width(Length::Fill)
        .padding(PADDING_SIDEBAR_LOGO);

    let icon_btn =
        |glyph: Text<'a>, tip: String, msg: Message, active: bool| -> Element<'a, Message> {
            let btn_content = container(glyph)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .width(Length::Fill)
                .height(Length::Fill);
            let btn = button(btn_content)
                .on_press(msg)
                .padding(PADDING_NONE)
                .width(Length::Fixed(40.0))
                .height(Length::Fixed(40.0))
                .style(theme::style::button::sidebar_nav(active));

            tooltip::standard(btn, text(tip), iced::widget::tooltip::Position::Right)
        };

    let list_area = icon_btn(
        icon::list().size(FONT_DIALOG_TITLE).line_height(1.0),
        fluent.get(Tr::Tasks),
        Message::Nav(NavMsg::NavigatePage(Page::Tasks)),
        false,
    );
    let new_area = icon_btn(
        icon::plus().size(FONT_DIALOG_TITLE).line_height(1.0),
        fluent.get(Tr::New),
        Message::Add(AddMsg::OpenAddDialog),
        false,
    );
    let about_area = icon_btn(
        icon::circle_help().size(FONT_DIALOG_TITLE).line_height(1.0),
        fluent.get(Tr::About),
        Message::Dialog(DialogMsg::OpenAbout),
        false,
    );
    let power_area = icon_btn(
        icon::power().size(FONT_DIALOG_TITLE).line_height(1.0),
        fluent.get(Tr::Shutdown),
        Message::Shutdown(crate::message::ShutdownMsg::ToggleCard),
        false,
    );
    let sett_area = icon_btn(
        icon::settings().size(FONT_DIALOG_TITLE).line_height(1.0),
        fluent.get(Tr::Settings),
        Message::Nav(NavMsg::NavigatePage(Page::Settings)),
        false,
    );

    let col = column![]
        .spacing(SPACE_SM)
        .align_x(Alignment::Center)
        .push(logo)
        .push(iced::widget::Space::new().height(Length::Fixed(20.0)))
        .push(list_area)
        .push(new_area)
        .push(iced::widget::Space::new().height(Length::Fill))
        .push(power_area)
        .push(about_area)
        .push(sett_area)
        .height(Length::Fill);

    container(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(PADDING_SIDEBAR)
        .style(theme::style::sidebar_background)
        .into()
}
