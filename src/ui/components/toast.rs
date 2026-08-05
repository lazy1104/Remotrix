use std::time::Duration;

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, mouse_area, row, stack, text};
use iced::{Element, Length};

use crate::message::{Message, ToastMsg};
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

const CARD_MAX_WIDTH: f32 = 320.0;
const MSG_MAX_WIDTH: f32 = 200.0;
const OVERLAY_PADDING: u16 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastKind {
    #[default]
    Normal,
    Warning,
    Error,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastGroup {
    #[default]
    General,
    Engine,
    Tracker,
    Task,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastPosition {
    TopLeft,
    #[default]
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl ToastPosition {
    const ALL: [ToastPosition; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];

    fn alignment(self) -> (Horizontal, Vertical) {
        match self {
            Self::TopLeft => (Horizontal::Left, Vertical::Top),
            Self::Top => (Horizontal::Center, Vertical::Top),
            Self::TopRight => (Horizontal::Right, Vertical::Top),
            Self::Right => (Horizontal::Right, Vertical::Center),
            Self::BottomRight => (Horizontal::Right, Vertical::Bottom),
            Self::Bottom => (Horizontal::Center, Vertical::Bottom),
            Self::BottomLeft => (Horizontal::Left, Vertical::Bottom),
            Self::Left => (Horizontal::Left, Vertical::Center),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
    pub group: ToastGroup,
    pub position: ToastPosition,
    pub show_close: bool,
    pub close_after: Option<Duration>,
    pub remaining: Option<Duration>,
}

impl Toast {
    pub fn new(kind: ToastKind, message: impl Into<String>) -> Self {
        Self {
            id: 0,
            kind,
            message: message.into(),
            group: ToastGroup::General,
            position: ToastPosition::Top,
            show_close: false,
            close_after: Some(Duration::from_secs(3)),
            remaining: None,
        }
    }

    pub fn group(mut self, group: ToastGroup) -> Self {
        self.group = group;
        self
    }

    pub fn show_close(mut self) -> Self {
        self.show_close = true;
        self
    }

    pub fn close_after(mut self, close_after: Option<Duration>) -> Self {
        self.close_after = close_after;
        self
    }
}

pub fn view<'a>(theme: &'a iced::Theme, toasts: &'a [Toast]) -> Element<'a, Message> {
    let mut layers: Vec<Element<'a, Message>> = Vec::new();
    for pos in ToastPosition::ALL {
        let pos_toasts: Vec<&Toast> = toasts.iter().filter(|t| t.position == pos).collect();
        if pos_toasts.is_empty() {
            continue;
        }
        let (h, v) = pos.alignment();
        let mut column_ = column![].spacing(SPACE_LG).align_x(Horizontal::Center);
        for t in pos_toasts {
            column_ = column_.push(card(theme, t));
        }
        layers.push(
            container(column_)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(h)
                .align_y(v)
                .padding(OVERLAY_PADDING)
                .into(),
        );
    }
    stack(layers)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn card<'a>(theme: &'a iced::Theme, toast: &'a Toast) -> Element<'a, Message> {
    let icon = match toast.kind {
        ToastKind::Normal => icon::info(),
        ToastKind::Warning => icon::triangle_alert(),
        ToastKind::Error => icon::circle_x(),
        ToastKind::Success => icon::circle_check(),
    }
    .size(FONT_TITLE)
    .color(kind_color(theme, toast.kind));

    let icon_col = container(icon).align_y(Vertical::Center);
    let message_col = container(
        text(&toast.message)
            .size(FONT_MEDIUM)
            .wrapping(text::Wrapping::Glyph),
    )
    .width(Length::Fill)
    .max_width(MSG_MAX_WIDTH);

    let mut content = row![icon_col, message_col]
        .spacing(SPACE_LG)
        .align_y(Vertical::Center);

    if toast.show_close {
        let close_btn = button(icon::x().size(FONT_BODY).line_height(1.0))
            .on_press(Message::Toast(ToastMsg::DismissToast(toast.id)))
            .padding(PADDING_TOAST_CLOSE)
            .style(theme::style::button::text());
        content = content.push(close_btn);
    }

    mouse_area(
        container(content)
            .width(Length::Shrink)
            .max_width(CARD_MAX_WIDTH)
            .padding(PADDING_TOAST)
            .style(theme::style::toast),
    )
    .on_enter(Message::Toast(ToastMsg::ToastHovered(toast.id)))
    .on_exit(Message::Toast(ToastMsg::ToastUnhovered(toast.id)))
    .into()
}

fn kind_color(theme: &iced::Theme, kind: ToastKind) -> iced::Color {
    match kind {
        ToastKind::Normal => theme::border_color(theme),
        ToastKind::Warning => theme::warning(theme),
        ToastKind::Error => theme::danger(theme),
        ToastKind::Success => theme::success(theme),
    }
}
