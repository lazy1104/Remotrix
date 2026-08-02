use iced::widget::{button, column, container, image, text, Text};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, Page};
use crate::ui::components::tooltip;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

pub fn view<'a>(
    fluent: &'a Fluent,
    _theme: &iced::Theme,
    page: Page,
    logo_handle: &'a iced::widget::image::Handle,
) -> Element<'a, Message> {
    let logo = container(
        image(logo_handle.clone())
            .width(Length::Fixed(28.0))
            .height(Length::Fixed(28.0)),
    )
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
        Message::NavigatePage(Page::Tasks),
        page == Page::Tasks,
    );
    let new_area = icon_btn(
        icon::plus().size(FONT_DIALOG_TITLE).line_height(1.0),
        fluent.get(Tr::New),
        Message::OpenAddDialog,
        false,
    );
    let about_area = icon_btn(
        icon::circle_help().size(FONT_DIALOG_TITLE).line_height(1.0),
        fluent.get(Tr::About),
        Message::OpenAbout,
        false,
    );
    let sett_area = icon_btn(
        icon::settings().size(FONT_DIALOG_TITLE).line_height(1.0),
        fluent.get(Tr::Settings),
        Message::NavigatePage(Page::Settings),
        page == Page::Settings,
    );

    let col = column![]
        .spacing(SPACE_SM)
        .align_x(Alignment::Center)
        .push(logo)
        .push(iced::widget::Space::new().height(Length::Fixed(20.0)))
        .push(list_area)
        .push(new_area)
        .push(iced::widget::Space::new().height(Length::Fill))
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
