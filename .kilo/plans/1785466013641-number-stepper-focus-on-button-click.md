# Fix: NumberStepper keeps input focused when clicking +/- buttons

## Goal
When the user clicks the `-` / `+` buttons in an editable `NumberStepper`,
the inner `text_input` should remain/come into focus, so the value updates
immediately and the input keeps accepting typing + shows the focus border.

## Root cause
iced 0.14 `button` is **not** focusable (`iced_widget-0.14.2/src/button.rs`
has no `Focusable` impl). Therefore the text_input is the *only* focusable
widget inside the stepper's child row.

Clicking a button publishes `Event::Mouse(ButtonPressed(Left))` at a point
outside the text_input's bounds. The text_input's own `update` then sets its
`is_focused = None` (`iced_widget.../text_input.rs:725-735`). This blur makes
`NumberStepper::update` hit the blur branch (`number_stepper.rs:423-432`),
which re-parses the stale buffer and calls `shell.publish(on_change(..))`.

Result: one press produces **two** messages — the button's stepped value
followed by the blur's clamped (old) value — so `+`/`-` appears to do nothing
when the input was focused, and focus is lost.

## Design decisions
- Use the file's existing pattern: run a focus `Operation` on the child tree
  inside `update` (same as `BufferReader`/`FocusProbe`).
- Define a tiny `FocusInput` operation that calls `state.focus()` on the
  (single) focusable child. No `Id` needed — the input is the only focusable,
  so there is no id-stability / cross-frame issue.
- Detect the press via the `Event` + `cursor.is_over(layout.bounds())`, and
  skip the blur-clamp branch on that same frame to avoid the double-publish.
- Only act in editable mode (`!read_only`).

## Changes — `src/ui/components/number_stepper.rs`

### 1. Add a `FocusInput` operation (near `FocusProbe`, ~line 63)
```rust
struct FocusInput;

impl widget::Operation for FocusInput {
    fn focusable(
        &mut self,
        _id: Option<&iced::widget::Id>,
        _bounds: Rectangle,
        state: &mut dyn iced::advanced::widget::operation::Focusable,
    ) {
        if !state.is_focused() {
            state.focus();
        }
    }

    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn widget::Operation)) {
        operate(self);
    }
}
```

### 2. In `NumberStepper::update` (around lines 421-433)
After the existing `FocusProbe` block, before the blur branch:

```rust
let refocus = !self.read_only
    && matches!(
        event,
        Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
    )
    && cursor.is_over(layout.bounds());

if refocus {
    let mut op = FocusInput;
    if let Some(child_layout) = layout.children().next() {
        self.child
            .as_widget_mut()
            .operate(&mut tree.children[0], child_layout, renderer, &mut op);
    }
}
```

Then guard the blur branch and set final focus:
```rust
if !state.focused && probe.focused {
    state.buffer = self.value.to_string();
} else if state.focused && !probe.focused && !refocus {
    // existing clamp + publish
    ...
}
state.focused = if refocus { true } else { probe.focused };
```

This leaves `build_row`, the `Id`/struct fields, and both constructors
untouched.

## Behavior matrix
- Click `-`/`+` while input focused → button publishes stepped value; refocus
  keeps input focused; blur branch skipped → exactly one message, focus kept.
- Click `-`/`+` while input unfocused → input gains focus; one message.
- Click elsewhere in stepper bounds → input focused (acceptable).
- Read-only stepper → `refocus` is `false`; unchanged.
- Typing → unchanged (`refocus` only fires on left press, blur branch skipped
  only that frame).

## Validation
```bash
cargo build
cargo clippy --workspace   # no new warnings
cargo fmt --check
cargo run --               # settings page + add-dialog:
```
Manual checks:
1. Focus the split stepper in add-dialog / a speed stepper in settings, type a
   number, click `+`/`-` → value changes and input stays focused (cursor +
   accent border visible), no reset to old value.
2. First click on `+`/`-` without focusing first → input becomes focused.
3. Read-only steppers still do nothing on click.
