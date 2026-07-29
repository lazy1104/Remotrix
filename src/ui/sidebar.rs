use iced::widget::{button, column, container, image, text, tooltip};
use iced::{Alignment, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, Page};
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
    .padding([8, 0]);

    let icon_btn =
        |codepoint: char, tip: String, msg: Message, active: bool| -> Element<'a, Message> {
            let glyph = text(codepoint.to_string())
                .font(iced::Font::with_name("lucide"))
                .size(20)
                .line_height(1.0);
            let btn_content = container(glyph)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .width(Length::Fill)
                .height(Length::Fill);
            let btn = button(btn_content)
                .on_press(msg)
                .padding(0)
                .width(Length::Fixed(40.0))
                .height(Length::Fixed(40.0))
                .style(theme::style::button::sidebar_nav(active));

            tooltip(btn, text(tip), tooltip::Position::Right)
                .style(container::rounded_box)
                .into()
        };

    let list_area = icon_btn(
        '\u{E106}',
        fluent.get(Tr::Tasks),
        Message::NavigatePage(Page::Tasks),
        page == Page::Tasks,
    );
    let new_area = icon_btn(
        '\u{E13D}',
        fluent.get(Tr::New),
        Message::OpenAddDialog,
        false,
    );
    let about_area = icon_btn('\u{E0F9}', fluent.get(Tr::About), Message::OpenAbout, false);
    let sett_area = icon_btn(
        '\u{E154}',
        fluent.get(Tr::Settings),
        Message::NavigatePage(Page::Settings),
        page == Page::Settings,
    );

    let col = column![]
        .spacing(4)
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
        .padding([12, 0])
        .style(theme::style::sidebar_background)
        .into()
}
