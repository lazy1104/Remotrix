use crate::i18n::{Fluent, Tr};
use crate::ui::components::tooltip;
use crate::ui::components::CONTROL_HEIGHT;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;
use iced::widget::{button, container, row, text, text_input, Space, Text};
use iced::{Alignment, Element, Length};

pub fn secret_input<'a, Message>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    value: &str,
    placeholder: &str,
    on_change: impl Fn(String) -> Message + 'a,
    on_generate: Message,
    on_copy: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let text_secondary = theme::text_secondary(theme);

    let input = theme::grouped_input_layout(
        text_input(placeholder, value)
            .on_input(on_change)
            .style(theme::style::input::grouped)
            .width(Length::Fixed(180.0)),
    );
    let mut row = row![]
        .spacing(SPACE_NONE)
        .align_y(Alignment::Center)
        .height(Length::Fill);
    row = row.push(input);
    row = row.push(separator());

    let copy_btn = tooltip::standard(
        button(icon_content(
            icon::copy().size(FONT_ICON).color(text_secondary),
        ))
        .on_press(on_copy)
        .style(theme::style::button::grouped_icon(false, false))
        .height(Length::Fill),
        text(fluent.get(Tr::Copy)),
        iced::widget::tooltip::Position::Bottom,
    );
    row = row.push(copy_btn);
    row = row.push(separator());

    let generate_btn = tooltip::standard(
        button(icon_content(
            icon::dices().size(FONT_ICON).color(text_secondary),
        ))
        .on_press(on_generate)
        .style(theme::style::button::grouped_icon(false, false))
        .height(Length::Fill),
        text(fluent.get(Tr::GenerateSecret)),
        iced::widget::tooltip::Position::Bottom,
    );
    row = row.push(generate_btn);

    container(row)
        .width(Length::Shrink)
        .height(Length::Fixed(CONTROL_HEIGHT))
        .padding(PADDING_GROUPED)
        .style(theme::style::grouped_frame_state(false, false))
        .into()
}

fn icon_content<'a, Message: 'a>(icon: Text<'a>) -> Element<'a, Message> {
    container(icon.line_height(1.0))
        .center_y(Length::Fill)
        .into()
}

fn separator<'a, Message: 'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fixed(1.0))
        .height(Length::Fill)
        .style(theme::style::separator)
        .into()
}
