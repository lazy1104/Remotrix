use iced::advanced::layout;
use iced::advanced::renderer;
use iced::advanced::text::{self, Renderer as TextRenderer};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::widget::Widget;
use iced::advanced::{Layout, Shell};
use iced::widget::checkbox::{self, Status};
use iced::{alignment, mouse, touch, window, Element, Event, Length, Pixels, Rectangle, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Checked,
    Partial,
    Unchecked,
}

pub fn tri_checkbox<'a, Message>(state: CheckState) -> TriCheckbox<'a, Message> {
    TriCheckbox::new(state)
}

pub struct TriCheckbox<'a, Message> {
    state: CheckState,
    size: f32,
    on_toggle: Option<Box<dyn Fn() -> Message + 'a>>,
}

impl<'a, Message> TriCheckbox<'a, Message> {
    const DEFAULT_SIZE: f32 = 16.0;

    fn new(state: CheckState) -> Self {
        TriCheckbox {
            state,
            size: Self::DEFAULT_SIZE,
            on_toggle: None,
        }
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into().0;
        self
    }

    pub fn on_toggle<F>(mut self, f: F) -> Self
    where
        F: 'a + Fn() -> Message,
    {
        self.on_toggle = Some(Box::new(f));
        self
    }

    pub fn on_toggle_maybe<F>(mut self, f: Option<F>) -> Self
    where
        F: Fn() -> Message + 'a,
    {
        self.on_toggle = f.map(|f| Box::new(f) as _);
        self
    }
}

struct TriCheckState {
    last_status: Option<Status>,
}

impl<'a, Message, Renderer> Widget<Message, iced::Theme, Renderer> for TriCheckbox<'a, Message>
where
    Renderer: TextRenderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<TriCheckState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(TriCheckState { last_status: None })
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fixed(self.size),
            height: Length::Fixed(self.size),
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::new(self.size, self.size))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let mouse_over = cursor.is_over(layout.bounds());

                if mouse_over {
                    if let Some(on_toggle) = &self.on_toggle {
                        shell.publish((on_toggle)());
                        shell.capture_event();
                    }
                }
            }
            _ => {}
        }

        let current_status = {
            let is_mouse_over = cursor.is_over(layout.bounds());
            let is_disabled = self.on_toggle.is_none();
            let is_checked = self.state != CheckState::Unchecked;

            if is_disabled {
                Status::Disabled { is_checked }
            } else if is_mouse_over {
                Status::Hovered { is_checked }
            } else {
                Status::Active { is_checked }
            }
        };

        if let Event::Window(window::Event::RedrawRequested(_now)) = event {
            tree.state.downcast_mut::<TriCheckState>().last_status = Some(current_status);
        } else {
            let last_status = tree.state.downcast_ref::<TriCheckState>().last_status;
            if last_status.is_some_and(|s| s != current_status) {
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) && self.on_toggle.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &iced::Theme,
        _defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let status = tree
            .state
            .downcast_ref::<TriCheckState>()
            .last_status
            .unwrap_or(Status::Disabled {
                is_checked: self.state != CheckState::Unchecked,
            });

        let style = checkbox::primary(theme, status);
        let bounds = layout.bounds();

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                ..renderer::Quad::default()
            },
            style.background,
        );

        match self.state {
            CheckState::Checked => {
                renderer.fill_text(
                    text::Text {
                        content: Renderer::CHECKMARK_ICON.to_string(),
                        font: Renderer::ICON_FONT,
                        size: Pixels(bounds.height * 0.7),
                        line_height: text::LineHeight::default(),
                        bounds: bounds.size(),
                        align_x: text::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text::Shaping::Basic,
                        wrapping: text::Wrapping::default(),
                    },
                    bounds.center(),
                    style.icon_color,
                    *viewport,
                );
            }
            CheckState::Partial => {
                let bar_width = bounds.width * 0.55;
                let bar_height = bounds.height * 0.15;
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: bounds.center_x() - bar_width / 2.0,
                            y: bounds.center_y() - bar_height / 2.0,
                            width: bar_width,
                            height: bar_height,
                        },
                        border: iced::Border {
                            radius: (bar_height / 2.0).into(),
                            ..Default::default()
                        },
                        ..renderer::Quad::default()
                    },
                    style.icon_color,
                );
            }
            CheckState::Unchecked => {}
        }
    }
}

impl<'a, Message: 'a, Renderer> From<TriCheckbox<'a, Message>>
    for Element<'a, Message, iced::Theme, Renderer>
where
    Renderer: TextRenderer + 'a,
{
    fn from(checkbox: TriCheckbox<'a, Message>) -> Self {
        Element::new(checkbox)
    }
}
