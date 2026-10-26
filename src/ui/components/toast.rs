use std::path::PathBuf;
use std::time::Duration;

use iced::advanced::layout::{Limits, Node};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget};
use iced::advanced::{mouse, Clipboard, Layout, Shell};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, mouse_area, row, stack, text, tooltip};
use iced::{Element, Length, Point, Rectangle, Size, Vector};

use crate::message::{Message, ToastMsg};
use crate::ui::animation::{
    ease_out_cubic, scale_factor_from_value, Animated, Easing, Event, ANIM_STANDARD_ENTER_MS,
    ANIM_STANDARD_EXIT_MS,
};
use crate::ui::components::tooltip as tip;
use crate::ui::components::truncated_text::truncated_text;
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

/// Action attached to a toast. Rendered as a labelled button on the
/// trailing edge of the toast card; clicking it dispatches the
/// corresponding engine message and dismisses the toast.
#[derive(Debug, Clone)]
pub enum ToastAction {
    RestartEngine,
    ApplyAppUpdate(crate::app_updater::AppUpdateOutcome),
    /// Open the given file with the system default application.
    OpenFile(PathBuf),
    /// Open the containing directory of the given path in the file manager.
    RevealDir(PathBuf),
}

const CARD_MAX_WIDTH: f32 = 440.0;
const CARD_CONTENT_MAX_WIDTH: f32 = CARD_MAX_WIDTH - PADDING_TOAST.left - PADDING_TOAST.right;
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
    pub actions: Vec<(ToastAction, String)>,
    pub anim: Animated<f32>,
    pub dismissing: bool,
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
            actions: Vec::new(),
            anim: Animated::transition(0.0, toast_enter_easing()),
            dismissing: false,
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

    /// Attach a labelled action button. Calling this more than once
    /// appends additional buttons to the toast's trailing edge.
    pub fn action(mut self, action: ToastAction, label: impl Into<String>) -> Self {
        self.actions.push((action, label.into()));
        self
    }

    /// Start the open transition (scale up from `0.0` to `1.0`). Safe to
    /// call repeatedly; if the toast is currently dismissing, the close
    /// transition is cancelled and a fresh enter easing replaces the
    /// current anim so the visible value snaps back to the bottom of the
    /// curve and re-plays the open.
    pub fn open(&mut self) {
        self.anim = Animated::transition(0.0, toast_enter_easing());
        self.anim.set_target(1.0);
        self.dismissing = false;
    }

    /// Start the exit transition (scale down from current size to `0.0`).
    /// Sets `dismissing` so [`Self::completed_dismiss`] can report
    /// completion once the anim finishes.
    pub fn begin_exit(&mut self) {
        self.anim = Animated::transition(self.anim.value().clamp(0.0, 1.0), toast_exit_easing());
        self.anim.set_target(0.0);
        self.dismissing = true;
    }

    /// The visible scale factor for this frame, in `[0.0, 1.0]`. Returns
    /// `0.0` for an unseen (just spawned or fully dismissed) toast.
    pub fn visible_scale(&self) -> f32 {
        scale_factor_from_value(*self.anim.value(), 0.0)
    }

    /// Forward an `iced_anim` event into the underlying anim.
    pub fn update(&mut self, event: Event<f32>) {
        self.anim.update(event);
    }

    /// `true` once the exit transition has played to completion.
    pub fn completed_dismiss(&mut self) -> bool {
        self.dismissing && !self.anim.is_animating()
    }

    /// `true` while any anim (open or exit) is in flight.
    pub fn is_animating(&self) -> bool {
        self.anim.is_animating()
    }
}

fn toast_enter_easing() -> Easing {
    ease_out_cubic(ANIM_STANDARD_ENTER_MS)
}

fn toast_exit_easing() -> Easing {
    ease_out_cubic(ANIM_STANDARD_EXIT_MS)
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
    let message = tip::standard(
        truncated_text(toast.message.clone())
            .size(FONT_MEDIUM)
            .max_lines(1)
            .width(Length::Fill),
        text(toast.message.clone()).size(FONT_SMALL),
        tooltip::Position::Bottom,
    );

    let mut trailing = row![].spacing(SPACE_LG).align_y(Vertical::Center);
    for (index, (_, label)) in toast.actions.iter().enumerate() {
        let msg = Message::Toast(ToastMsg::ToastActionPressed(toast.id, index));
        let btn = if index == 0 {
            button(text(label.as_str()).size(FONT_MEDIUM))
                .on_press(msg)
                .padding(PADDING_TOOLBAR_CAPSULE)
                .style(theme::style::button::primary())
        } else {
            button(text(label.as_str()).size(FONT_MEDIUM))
                .on_press(msg)
                .padding(PADDING_TOOLBAR_CAPSULE)
                .style(theme::style::button::secondary())
        };
        trailing = trailing.push(btn);
    }

    if toast.show_close {
        let close_btn = button(icon::x().size(FONT_BODY).line_height(1.0))
            .on_press(Message::Toast(ToastMsg::DismissToast(toast.id)))
            .padding(PADDING_TOAST_CLOSE)
            .style(theme::style::button::text());
        trailing = trailing.push(close_btn);
    }

    let body = ToastBody::new(
        icon_col.into(),
        message,
        trailing.into(),
        CARD_CONTENT_MAX_WIDTH,
        SPACE_LG,
    );

    let card = mouse_area(
        container(body)
            .width(Length::Shrink)
            .max_width(CARD_MAX_WIDTH)
            .padding(PADDING_TOAST)
            .style(theme::style::toast),
    )
    .on_enter(Message::Toast(ToastMsg::ToastHovered(toast.id)))
    .on_exit(Message::Toast(ToastMsg::ToastUnhovered(toast.id)));

    let scaled = crate::ui::components::scale::scale(card, toast.visible_scale());
    let id = toast.id;
    crate::ui::animation::animation(&toast.anim, scaled)
        .on_update(move |e| Message::Toast(ToastMsg::ToastAnim(id, e)))
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

/// Width left for the message once the leading icon and the trailing
/// controls took their share of the card.
fn message_budget(content_max: f32, leading: f32, trailing: f32, gaps: f32) -> f32 {
    (content_max - leading - trailing - gaps).max(0.0)
}

/// Toast card body laid out in two passes: the trailing controls are
/// measured first, so the message only receives the space they left over
/// and the whole card stays within `max_width`.
///
/// A `Length::Fill` message cannot be used directly here. The card is a
/// `Length::Shrink` container, which marks the whole subtree as
/// compressed, and iced then measures the row children in order — the
/// message would take the full width and starve the buttons that follow
/// it.
struct ToastBody<'a, Message> {
    /// Leading icon, message and trailing controls, in that order.
    elements: Vec<Element<'a, Message>>,
    max_width: f32,
    spacing: f32,
}

impl<'a, Message> ToastBody<'a, Message> {
    const LEADING: usize = 0;
    const MESSAGE: usize = 1;
    const TRAILING: usize = 2;

    fn new(
        leading: Element<'a, Message>,
        message: Element<'a, Message>,
        trailing: Element<'a, Message>,
        max_width: f32,
        spacing: f32,
    ) -> Self {
        Self {
            elements: vec![leading, message, trailing],
            max_width,
            spacing,
        }
    }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for ToastBody<'a, Message> {
    fn children(&self) -> Vec<widget::Tree> {
        self.elements
            .iter()
            .map(widget::Tree::new)
            .collect::<Vec<_>>()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(&self.elements);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &iced::Renderer,
        limits: &Limits,
    ) -> Node {
        let loose = limits.loose();

        let leading = self.elements[Self::LEADING].as_widget_mut().layout(
            &mut tree.children[Self::LEADING],
            renderer,
            &loose,
        );
        let trailing = self.elements[Self::TRAILING].as_widget_mut().layout(
            &mut tree.children[Self::TRAILING],
            renderer,
            &loose,
        );

        let leading_size = leading.size();
        let trailing_size = trailing.size();

        let trailing_gap = if trailing_size.width > 0.0 {
            self.spacing
        } else {
            0.0
        };
        let gaps = self.spacing + trailing_gap;

        let content_max = self.max_width.min(limits.max().width);
        let budget = message_budget(content_max, leading_size.width, trailing_size.width, gaps);

        // `compression.width = true` keeps `Length::Fill` resolving to the
        // intrinsic text width, so a short message hugs its content while a
        // long one is truncated to the budget.
        let message_limits = Limits::with_compression(
            Size::ZERO,
            Size::new(budget, limits.max().height),
            Size::new(true, false),
        );
        let message = self.elements[Self::MESSAGE].as_widget_mut().layout(
            &mut tree.children[Self::MESSAGE],
            renderer,
            &message_limits,
        );
        let message_size = message.size();

        let width = leading_size.width
            + self.spacing
            + message_size.width
            + trailing_gap
            + trailing_size.width;
        let height = leading_size
            .height
            .max(message_size.height)
            .max(trailing_size.height);

        let leading = leading.move_to(Point::new(0.0, (height - leading_size.height) / 2.0));
        let message = message.move_to(Point::new(
            leading_size.width + self.spacing,
            (height - message_size.height) / 2.0,
        ));
        let trailing = trailing.move_to(Point::new(
            leading_size.width + self.spacing + message_size.width + trailing_gap,
            (height - trailing_size.height) / 2.0,
        ));

        Node::with_children(Size::new(width, height), vec![leading, message, trailing])
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        for ((child, child_tree), child_layout) in self
            .elements
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
        {
            child.as_widget().draw(
                child_tree,
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                viewport,
            );
        }
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        for ((child, child_tree), child_layout) in self
            .elements
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            child.as_widget_mut().update(
                child_tree,
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation<()>,
    ) {
        for ((child, child_tree), child_layout) in self
            .elements
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            operation.container(None, child_layout.bounds());
            child
                .as_widget_mut()
                .operate(child_tree, child_layout, renderer, operation);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.elements
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, child_tree), child_layout)| {
                child.as_widget().mouse_interaction(
                    child_tree,
                    child_layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .max()
            .unwrap_or_default()
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        iced::advanced::overlay::from_children(
            &mut self.elements,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a> From<ToastBody<'a, Message>> for Element<'a, Message> {
    fn from(body: ToastBody<'a, Message>) -> Self {
        Element::new(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_leaves_room_for_the_fixed_parts() {
        assert_eq!(message_budget(416.0, 16.0, 0.0, 8.0), 392.0);
        assert_eq!(message_budget(416.0, 16.0, 150.0, 16.0), 234.0);
    }

    #[test]
    fn budget_never_goes_negative() {
        assert_eq!(message_budget(416.0, 16.0, 500.0, 16.0), 0.0);
        assert_eq!(message_budget(10.0, 16.0, 0.0, 8.0), 0.0);
    }

    #[test]
    fn budget_is_bounded_by_the_content_width() {
        assert_eq!(CARD_CONTENT_MAX_WIDTH, 416.0);
        const {
            assert!(CARD_CONTENT_MAX_WIDTH < CARD_MAX_WIDTH);
        }
    }
}
