use iced::advanced::layout::{Limits, Node};
use iced::advanced::renderer;
use iced::advanced::widget::{self, tree, Widget};
use iced::advanced::{mouse, Clipboard, Layout, Renderer, Shell};
use iced::widget::{button, column, container, row, text};
use iced::{Element, Event, Length, Padding, Rectangle, Size};

use crate::message::Message;
use crate::task::format_speed;
use crate::ui::animation::{animation, Animated};
use crate::ui::dims::*;
use crate::ui::icon;
use crate::ui::theme;

const HUD_SIZE: f32 = 44.0;

pub fn view<'a>(
    theme: &'a iced::Theme,
    download: u64,
    upload: u64,
    hud: &'a Animated<f32>,
) -> Element<'a, Message> {
    let palette = theme.extended_palette();
    let primary_weak = palette.primary.weak.color;
    let primary = palette.primary.base.color;

    let icon_col = container(icon::download().size(FONT_HERO).color(primary_weak))
        .center_x(Length::Fixed(HUD_SIZE));

    let up_row = row![
        icon::arrow_up().size(FONT_SMALL).color(primary_weak),
        text(format_speed(upload))
            .size(FONT_SMALL)
            .color(primary_weak),
    ]
    .spacing(SPACE_SM)
    .align_y(iced::alignment::Vertical::Center);

    let down_row = row![
        icon::download_arrow().size(FONT_SMALL).color(primary),
        text(format_speed(download)).size(FONT_SMALL).color(primary),
    ]
    .spacing(SPACE_SM)
    .align_y(iced::alignment::Vertical::Center);

    let block = column![up_row, down_row].spacing(SPACE_XS);

    let content = row![icon_col, block]
        .spacing(SPACE_LG)
        .align_y(iced::alignment::Vertical::Center);

    let hud_content = animation(
        hud,
        Clip {
            content: content.into(),
            progress: *hud.value(),
        },
    )
    .on_update(Message::HudAnim);

    let button = button(hud_content)
        .on_press(Message::Noop)
        .padding(Padding {
            top: PADDING_HUD.top,
            right: 0.0,
            bottom: PADDING_HUD.bottom,
            left: 0.0,
        })
        .height(Length::Fixed(HUD_SIZE))
        .style(theme::style::button::speed_hud());

    button.into()
}

struct Clip<'a, Message> {
    content: Element<'a, Message>,
    progress: f32,
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for Clip<'a, Message> {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<widget::Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &iced::Renderer,
        limits: &Limits,
    ) -> Node {
        let node = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &limits.loose());
        let row_w = node.size().width;
        let t = self.progress.clamp(0.0, 1.0);
        let w_natural = row_w + PADDING_HUD.right;
        let w_vis = (HUD_SIZE + (w_natural - HUD_SIZE) * t).clamp(HUD_SIZE, w_natural);
        let h = limits.max().height;
        let y = ((h - node.size().height) / 2.0).max(0.0);
        Node::with_children(Size::new(w_vis, h), vec![node.move_to((0.0, y))])
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
        let bounds = layout.bounds();
        renderer.with_layer(bounds, |renderer| {
            self.content.as_widget().draw(
                tree,
                renderer,
                theme,
                style,
                layout.children().next().unwrap(),
                cursor,
                viewport,
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if !cursor.is_over(layout.bounds()) {
            return;
        }
        self.content.as_widget_mut().update(
            tree,
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content.as_widget_mut().operate(
            tree,
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::None;
        }
        self.content.as_widget().mouse_interaction(
            tree,
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }
}

impl<'a, Message: 'a> From<Clip<'a, Message>> for Element<'a, Message> {
    fn from(clip: Clip<'a, Message>) -> Self {
        Element::new(clip)
    }
}
