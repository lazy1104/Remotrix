# Fix: Modal overlay does not block clicks to underlying content

## Problem

With any modal open (New Download / About / Close / Task Details / Confirm), the backdrop
(遮罩层) is drawn but clicks still hit the task list underneath. E.g. a pause button under
the backdrop fires while the dialog is open.

## Root cause (verified in iced 0.14 sources)

- `src/ui/components/dialog.rs:8` `overlay()` builds a full-screen `container` with the
  backdrop style. In `iced_widget-0.14.2/src/container.rs:309`, `Container::update` only
  forwards events to its content — it **never** captures them.
- `iced_widget-0.14.2/src/stack.rs:231` `Stack::update` iterates children top-down and only
  stops once `shell.is_event_captured()` is true. Since the overlay never captures, every
  mouse event falls through to `base_layer` (composed in `src/app.rs:2267`).
- `Widget::update` has no return status; capture is done via `Shell::capture_event()`
  (`iced_core-0.14.0/src/widget.rs:112`).

All 5 modals use the shared `overlay()` helper, so a single fix covers them.

## Fix

Add a small custom widget `BlockingOverlay` in `src/ui/components/dialog.rs` that:
1. Lays out / draws exactly like its wrapped content (delegates `layout`, `draw`,
   `operate`, `overlay`, `mouse_interaction` to content).
2. In `update`: first forwards the event to the content (so dialog buttons/text inputs
   still work), then — if the event was not already captured by the content — captures all
   `Event::Mouse(_) | Event::Touch(_)` events whose cursor is over its bounds via
   `shell.capture_event()`. This stops the `Stack` from passing the event to layers below.
3. In `mouse_interaction`: returns the content's interaction when hovering the dialog, and
   `mouse::Interaction::Idle` otherwise (instead of `None`) so the `Stack` treats the
   overlay as the top interactive layer and suppresses hover states under the backdrop
   (stack.rs:317 uses the first non-`None` interaction to dim/disable lower layers).

`overlay()` is reimplemented as:

```rust
pub fn overlay<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    BlockingOverlay::new(
        container(content)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::style::overlay),
    )
    .into()
}
```

### Widget skeleton (mirror imports/style of `src/ui/components/drop_down.rs`)

```rust
use iced::advanced::{
    layout::{Limits, Node},
    mouse, overlay, renderer,
    widget::{self, Operation, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

struct BlockingOverlay<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: renderer::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> BlockingOverlay<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self { content: content.into() }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for BlockingOverlay<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size { width: Length::Fill, height: Length::Fill }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let limits = limits.width(Length::Fill).height(Length::Fill);
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, &limits)
    }

    fn draw(
        &self, tree: &Tree, renderer: &mut Renderer, theme: &Theme,
        style: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content]);
    }

    fn operate<'b>(
        &'b mut self, tree: &'b mut Tree, layout: Layout<'_>, renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self, tree: &mut Tree, event: &Event, layout: Layout<'_>,
        cursor: mouse::Cursor, renderer: &Renderer, clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>, viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0], event, layout, cursor, renderer, clipboard, shell, viewport,
        );

        if shell.is_event_captured() {
            return;
        }

        match event {
            Event::Mouse(_) | Event::Touch(_) if cursor.is_over(layout.bounds()) => {
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self, tree: &Tree, layout: Layout<'_>, cursor: mouse::Cursor,
        viewport: &Rectangle, renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            let interaction = self.content.as_widget().mouse_interaction(
                &tree.children[0], layout, cursor, viewport, renderer,
            );
            if interaction != mouse::Interaction::None {
                interaction
            } else {
                mouse::Interaction::Idle
            }
        } else {
            mouse::Interaction::None
        }
    }

    fn overlay<'b>(
        &'b mut self, tree: &'b mut Tree, layout: Layout<'b>, renderer: &Renderer,
        viewport: &Rectangle, translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0], layout, renderer, viewport, translation,
        )
    }
}
```

## Scope / non-goals

- Only the shared `dialog::overlay()` changes; `speed_hud` and `toast` layers are
  intentionally non-blocking and must stay unchanged.
- No backdrop click-to-dismiss behavior is added (not requested; risk of accidental
  dismiss on confirm dialogs).
- Keyboard events are deliberately not captured so dialog text inputs still receive input.

## Validation

1. `cargo build` and `cargo clippy --workspace` (no warnings allowed), `cargo fmt --check`.
2. Manual: open each dialog (Add Download, About, Close, Task Details, Confirm), verify:
   - clicking task-list buttons / toolbar under the backdrop no longer triggers anything;
   - dialog buttons, text inputs, dropdowns, scrollables still work;
   - scroll wheel over the backdrop does not scroll the task list;
   - hover highlight under the backdrop no longer appears.
3. Regression: HUD capsule and toasts remain click-through (non-modal).
