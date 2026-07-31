use std::time::Duration;

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, stack, text};
use iced::{Element, Length, Padding};

use crate::message::Message;
use crate::ui::icon;
use crate::ui::theme;

const CARD_WIDTH: f32 = 320.0;
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
pub enum ToastPosition {
    TopLeft,
    Top,
    TopRight,
    Right,
    #[default]
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
    pub position: ToastPosition,
    pub show_close: bool,
    pub close_after: Option<Duration>,
}

impl Toast {
    pub fn new(kind: ToastKind, message: impl Into<String>) -> Self {
        Self {
            id: 0,
            kind,
            message: message.into(),
            position: ToastPosition::BottomRight,
            show_close: false,
            close_after: Some(Duration::from_secs(3)),
        }
    }

    pub fn position(mut self, position: ToastPosition) -> Self {
        self.position = position;
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
        let mut column_ = column![].spacing(8);
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
    .size(16)
    .color(kind_color(theme, toast.kind));

    let icon_col = container(icon).align_y(Vertical::Top);
    let message_col = text(&toast.message).size(13).width(Length::Fill);

    let mut content = row![icon_col, message_col]
        .spacing(8)
        .align_y(Vertical::Top);

    if toast.show_close {
        let close_btn = button(icon::x().size(14).line_height(1.0))
            .on_press(Message::DismissToast(toast.id))
            .padding(2)
            .style(theme::style::button::text());
        content = content.push(close_btn);
    }

    container(content)
        .width(Length::Fixed(CARD_WIDTH))
        .padding(Padding {
            top: 10.0,
            right: 12.0,
            bottom: 10.0,
            left: 12.0,
        })
        .style(theme::style::toast)
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
