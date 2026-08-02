use std::rc::Rc;

use iced::widget::{button, container, row, text};
use iced::{Alignment, Element, Length};

use super::drop_down;
use super::number_stepper::number_stepper;
use super::CONTROL_HEIGHT;
use crate::scheduler::parse_hhmm;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

fn picker_button<'a>() -> impl Fn(&iced::Theme, button::Status) -> button::Style + 'a {
    move |t: &iced::Theme, status: button::Status| button::Style {
        background: Some(t.extended_palette().background.base.color.into()),
        text_color: t.extended_palette().background.base.text,
        border: iced::Border {
            color: match status {
                button::Status::Hovered | button::Status::Pressed => {
                    t.extended_palette().primary.base.color
                }
                _ => theme::border_color(t),
            },
            width: 1.0,
            radius: theme::RADIUS_BUTTON.into(),
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

pub fn time_picker<'a, M>(
    value: &'a str,
    open: bool,
    on_toggle: M,
    on_change: impl Fn(String) -> M + 'a,
    width: Length,
) -> Element<'a, M, iced::Theme, iced::Renderer>
where
    M: 'a + Clone,
{
    let (h, m) = parse_hhmm(value).unwrap_or((0, 0));
    let hour: &'a u8 = Box::leak(Box::new(h));
    let minute: &'a u8 = Box::leak(Box::new(m));
    let on_change: Rc<dyn Fn(String) -> M + 'a> = Rc::new(on_change);

    let underlay: Element<'a, M> = button(
        row![
            text(value).size(FONT_MEDIUM).width(Length::Fill),
            icon::chevron_down().size(FONT_ICON),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_LG),
    )
    .on_press(on_toggle.clone())
    .padding(PADDING_GROUPED)
    .height(Length::Fixed(CONTROL_HEIGHT))
    .width(Length::Fill)
    .style(picker_button())
    .into();

    let overlay: Element<'a, M> = {
        let mut row = row![]
            .spacing(SPACE_XS)
            .align_y(Alignment::Center)
            .padding(PADDING_DROPDOWN);
        row = row.push(number_stepper(
            hour,
            0..=23u8,
            1,
            {
                let on_change = on_change.clone();
                move |v: u8| on_change(format!("{:02}:{:02}", v, *minute))
            },
            Length::Fixed(64.0),
        ));
        row = row.push(
            text(":")
                .size(FONT_MEDIUM)
                .style(theme::style::text::secondary),
        );
        row = row.push(number_stepper(
            minute,
            0..=59u8,
            1,
            {
                let on_change = on_change.clone();
                move |v: u8| on_change(format!("{:02}:{:02}", *hour, v))
            },
            Length::Fixed(64.0),
        ));
        container(row)
            .width(Length::Shrink)
            .style(theme::style::card)
            .into()
    };

    drop_down::DropDown::new(underlay, overlay, open)
        .alignment(drop_down::Alignment::Bottom)
        .offset(drop_down::Offset::from(0.0))
        .on_dismiss(on_toggle)
        .width(width)
        .into()
}
