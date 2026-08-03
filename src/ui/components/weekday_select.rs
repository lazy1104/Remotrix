use std::rc::Rc;

use iced::widget::{button, checkbox, column, row, text};
use iced::{Alignment, Element, Length};

use super::drop_down::{self, DropDown};
use super::{time_picker::picker_button, CONTROL_HEIGHT};
use crate::ui::dims::*;
use crate::ui::icon;

pub fn weekday_select<'a, M>(
    summary: String,
    selected: &'a [u8],
    day_labels: [String; 7],
    open: bool,
    on_toggle: impl Fn(u8, bool) -> M + 'a,
    on_dismiss: M,
) -> Element<'a, M, iced::Theme, iced::Renderer>
where
    M: 'a + Clone,
{
    let underlay: Element<'a, M> = button(
        row![
            text(summary).size(FONT_MEDIUM),
            icon::chevron_down().size(FONT_ICON),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_LG)
        .height(Length::Fill),
    )
    .on_press(on_dismiss.clone())
    .padding(PADDING_GROUPED)
    .height(Length::Fixed(CONTROL_HEIGHT))
    .style(picker_button())
    .into();

    let on_toggle = Rc::new(on_toggle);
    let mut overlay = column![].spacing(SPACE_SM).padding(PADDING_DROPDOWN);
    for (i, label) in day_labels.iter().enumerate() {
        let day = (i + 1) as u8;
        let checked = selected.contains(&day);
        let on_toggle = on_toggle.clone();
        overlay = overlay.push(
            row![
                text(label.clone())
                    .size(FONT_MEDIUM)
                    .width(Length::Fixed(64.0)),
                checkbox(checked).on_toggle(move |b| (*on_toggle)(day, b)),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACE_LG),
        );
    }
    let overlay: Element<'a, M, iced::Theme, iced::Renderer> = overlay.into();

    DropDown::new(underlay, overlay, open)
        .on_dismiss(on_dismiss)
        .alignment(drop_down::Alignment::Bottom)
        .width(Length::Fixed(160.0))
        .into()
}
