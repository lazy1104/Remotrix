use std::cell::RefCell;
use std::rc::Rc;

use iced::advanced::{
    layout::{Limits, Node},
    mouse, overlay, renderer, text,
    widget::{tree, Operation, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::widget::{text_input, TextInput};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

use crate::ui::components::ctx_menu::CtxCursor;

pub struct CtxInput<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: text_input::Catalog,
    Renderer: text::Renderer,
{
    inner: TextInput<'a, Message, Theme, Renderer>,
    value: String,
    cursor: Rc<RefCell<CtxCursor>>,
}

impl<'a, Message, Theme, Renderer> CtxInput<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: text_input::Catalog,
    Renderer: text::Renderer,
{
    pub fn new(placeholder: &str, value: &str, cursor: Rc<RefCell<CtxCursor>>) -> Self {
        Self {
            inner: TextInput::new(placeholder, value),
            value: value.to_string(),
            cursor,
        }
    }

    pub fn on_input(mut self, on_input: impl Fn(String) -> Message + 'a) -> Self {
        self.inner = self.inner.on_input(on_input);
        self
    }

    pub fn on_input_maybe(mut self, on_input: Option<fn(String) -> Message>) -> Self {
        self.inner = self.inner.on_input_maybe(on_input);
        self
    }

    pub fn on_submit(mut self, on_submit: Message) -> Self {
        self.inner = self.inner.on_submit(on_submit);
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.inner = self.inner.width(width);
        self
    }

    pub fn padding<P: Into<iced::Padding>>(mut self, padding: P) -> Self {
        self.inner = self.inner.padding(padding);
        self
    }

    pub fn size(mut self, size: impl Into<iced::Pixels>) -> Self {
        self.inner = self.inner.size(size);
        self
    }

    pub fn font(mut self, font: Renderer::Font) -> Self {
        self.inner = self.inner.font(font);
        self
    }

    pub fn secure(mut self, is_secure: bool) -> Self {
        self.inner = self.inner.secure(is_secure);
        self
    }

    pub fn style(
        mut self,
        style: impl Fn(&Theme, text_input::Status) -> text_input::Style + 'a,
    ) -> Self
    where
        Theme::Class<'a>: From<text_input::StyleFn<'a, Theme>>,
    {
        self.inner = self.inner.style(style);
        self
    }

    fn apply_pending_caret(&self, tree: &mut Tree) {
        let len = text_input::Value::new(&self.value).len();
        let mut cursor = self.cursor.borrow_mut();
        if let Some((caret, expected_len)) = cursor.pending_caret {
            if len == expected_len {
                let state = tree
                    .state
                    .downcast_mut::<text_input::State<Renderer::Paragraph>>();
                state.move_cursor_to(caret);
                cursor.pending_caret = None;
            }
        }
    }

    fn sync(&self, tree: &Tree) {
        let state = tree
            .state
            .downcast_ref::<text_input::State<Renderer::Paragraph>>();
        let value = text_input::Value::new(&self.value);
        let pending = self.cursor.borrow().pending_caret;
        *self.cursor.borrow_mut() = match state.cursor().state(&value) {
            text_input::cursor::State::Index(index) => CtxCursor {
                selection: None,
                caret: index,
                pending_caret: pending,
            },
            text_input::cursor::State::Selection { start, end } => CtxCursor {
                selection: Some((start.min(end), start.max(end))),
                caret: start.max(end),
                pending_caret: pending,
            },
        };
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for CtxInput<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: text_input::Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<text_input::State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(text_input::State::<Renderer::Paragraph>::new())
    }

    fn diff(&self, tree: &mut Tree) {
        Widget::diff(&self.inner, tree);
        self.sync(tree);
    }

    fn size(&self) -> Size<Length> {
        Widget::size(&self.inner)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let node = Widget::layout(&mut self.inner, tree, renderer, limits);
        self.apply_pending_caret(tree);
        self.sync(tree);
        node
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        Widget::draw(
            &self.inner,
            tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        Widget::operate(&mut self.inner, tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        Widget::update(
            &mut self.inner,
            tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        self.sync(tree);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        Widget::mouse_interaction(&self.inner, tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        Widget::overlay(
            &mut self.inner,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<CtxInput<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: text_input::Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(input: CtxInput<'a, Message, Theme, Renderer>) -> Self {
        Element::new(input)
    }
}
