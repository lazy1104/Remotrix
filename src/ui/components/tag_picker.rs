use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use super::drop_down::{self, DropDown};
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

pub fn tag_picker<'a, M, V>(
    options: Vec<(V, String)>,
    selected: &'a [V],
    placeholder: String,
    open: bool,
    on_toggle: impl Fn(V, bool) -> M + 'a,
    on_dismiss: M,
    width: Length,
) -> Element<'a, M, iced::Theme, iced::Renderer>
where
    V: PartialEq + Clone + 'a,
    M: 'a + Clone,
{
    let label_of = |value: &V| {
        options
            .iter()
            .find(|(v, _)| v == value)
            .map(|(_, l)| l.clone())
    };

    let mut tag_items: Vec<Element<'a, M, iced::Theme, iced::Renderer>> = Vec::new();

    if selected.is_empty() {
        tag_items.push(
            text(placeholder)
                .size(FONT_MEDIUM)
                .style(theme::style::text::secondary)
                .into(),
        );
    } else {
        for value in selected {
            let Some(label) = label_of(value) else {
                continue;
            };
            tag_items.push(
                button(
                    row![text(label).size(FONT_MEDIUM), icon::x().size(FONT_SMALL),]
                        .align_y(Alignment::Center)
                        .spacing(SPACE_XS),
                )
                .on_press(on_toggle(value.clone(), false))
                .padding([2, 8])
                .style(theme::style::button::chip())
                .into(),
            );
        }
    }

    tag_items.push(
        button(icon::plus().size(FONT_MEDIUM))
            .on_press(on_dismiss.clone())
            .padding([2, 8])
            .style(theme::style::button::chip())
            .into(),
    );

    let tag_row = row(tag_items)
        .spacing(SPACE_XS)
        .wrap()
        .vertical_spacing(SPACE_XS)
        .align_x(Alignment::Start);

    let underlay = container(tag_row)
        .width(width)
        .padding([6, 8])
        .style(theme::style::grouped_frame_state(false, false));

    let mut overlay = column![].spacing(SPACE_XS).width(Length::Fill);
    for (value, label) in &options {
        let checked = selected.contains(value);
        let toggle = on_toggle(value.clone(), !checked);
        let item: iced::widget::Button<'_, M, iced::Theme, iced::Renderer> = if checked {
            button(text(label.clone()).size(FONT_MEDIUM).width(Length::Fill))
                .on_press(toggle)
                .width(Length::Fill)
                .padding(PADDING_BUTTON_XS)
                .style(theme::style::button::chip())
        } else {
            button(text(label.clone()).size(FONT_MEDIUM).width(Length::Fill))
                .on_press(toggle)
                .width(Length::Fill)
                .padding(PADDING_BUTTON_XS)
                .style(theme::style::button::text())
        };
        overlay = overlay.push(item);
    }
    let overlay = container(overlay)
        .padding(PADDING_DROPDOWN)
        .style(theme::style::card);

    DropDown::new(underlay, overlay, open)
        .on_dismiss(on_dismiss)
        .alignment(drop_down::Alignment::Bottom)
        .width(width)
        .into()
}
