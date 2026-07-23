use iced::widget::{button, column, container, image, text, tooltip};
use iced::{Color, Element, Length};

use crate::i18n::{Fluent, Tr};
use crate::message::{Message, Page};
use crate::ui::theme;

pub fn view<'a>(fluent: &'a Fluent, dark: bool, page: Page) -> Element<'a, Message> {
    let bg_sidebar = if dark {
        theme::BG_SIDEBAR
    } else {
        theme::BG_SIDEBAR_LIGHT
    };
    let text_primary = if dark {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_PRIMARY_LIGHT
    };

    let icon_bytes: &[u8] = include_bytes!("../../assets/icon.png");
    let logo = container(
        image(image::Handle::from_bytes(icon_bytes))
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
                .size(20);
            let btn_content = container(glyph).center_x(Length::Fill).width(Length::Fill);
            let btn = button(btn_content).on_press(msg);

            #[allow(clippy::type_complexity)]
            let style: Box<dyn Fn(&iced::Theme, button::Status) -> button::Style + 'a> = if active {
                Box::new(
                    move |_theme: &iced::Theme, _status: button::Status| -> button::Style {
                        button::Style {
                            background: Some(Color::from_rgba(0.29, 0.565, 0.851, 0.25).into()),
                            text_color: theme::ACCENT,
                            border: iced::border::rounded(6),
                            ..Default::default()
                        }
                    },
                )
            } else {
                let hover = Color::from_rgba(1.0, 1.0, 1.0, 0.08);
                Box::new(
                    move |_theme: &iced::Theme, status: button::Status| -> button::Style {
                        let bg = match status {
                            button::Status::Hovered | button::Status::Pressed => Some(hover.into()),
                            _ => None,
                        };
                        button::Style {
                            background: bg,
                            text_color: text_primary,
                            border: iced::border::rounded(6),
                            ..Default::default()
                        }
                    },
                )
            };

            let btn = btn.padding([10, 0]).width(Length::Fill).style(style);

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
        .push(logo)
        .push(iced::widget::Space::new().height(Length::Fixed(20.0)))
        .push(list_area)
        .push(new_area)
        .push(about_area)
        .push(iced::widget::Space::new().height(Length::Fill))
        .push(sett_area)
        .height(Length::Fill);

    container(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([12, 0])
        .style(move |_theme| container::Style {
            background: Some(bg_sidebar.into()),
            text_color: Some(text_primary),
            ..Default::default()
        })
        .into()
}
