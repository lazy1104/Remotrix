# Plan: Fix Pointer Cursor Leak in Add Dialog (NumberStepper)

## Goal
Eliminate the stray hand/pointer cursor over blank areas of the New Download dialog (and the dimmed overlay behind it) that persists even after the drop-zone pointer scoping from the previous plan.

## Root Cause (verified against iced 0.14.2 / 0.14.0 sources)
- `NumberStepper::mouse_interaction` returns `mouse::Interaction::Pointer` **unconditionally**, with no `cursor.is_over()` check:
  `src/ui/components/number_stepper.rs:495-504`.
- iced aggregates interactions without cursor-gating along the whole chain:
  - `Row::mouse_interaction` / `Column::mouse_interaction` take `.max()` over ALL children (row.rs:274, column.rs:285) — a child reporting Pointer poisons the parent regardless of cursor position.
  - `Scrollable::mouse_interaction` (scrollable.rs:1311) and `Container::mouse_interaction` (container.rs:321) delegate to content unconditionally.
- The stepper sits in `split_input` (add_dialog.rs:155-170), which is pushed into the dialog body for **both** tabs (add_dialog.rs:237). The full-screen `add_layer` overlay (app.rs:1739) therefore reports Pointer everywhere; the root `stack` (app.rs:1785) picks the topmost non-None interaction (stack.rs:278), so the whole window shows the hand cursor while the dialog is open.
- `mouse_area`, `button`, `text_input`, `checkbox` are all correctly cursor-gated in this dependency set — they are NOT the leak. The previous drop-zone `.interaction(mouse::Interaction::Pointer)` scoping is correct and must be kept.
- Same widget is used in `settings_page.rs` (3 call sites), so the fix also repairs those dialogs.

## Task 1 — Gate `NumberStepper::mouse_interaction` (`src/ui/components/number_stepper.rs:495-504`)
Replace the body of the method so it (a) returns `None` when the cursor is not over the widget bounds, and (b) delegates to the inner content so the text input shows the `Text` cursor and the +/− buttons show `Pointer`:

```rust
fn mouse_interaction(
    &self,
    tree: &Tree,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
    renderer: &iced::Renderer,
) -> mouse::Interaction {
    if !cursor.is_over(layout.bounds()) {
        return mouse::Interaction::None;
    }
    if let Some(child_layout) = layout.children().next() {
        self.child.as_widget().mouse_interaction(
            &tree.children[0],
            child_layout,
            cursor,
            viewport,
            renderer,
        )
    } else {
        mouse::Interaction::None
    }
}
```

Notes:
- `cursor.is_over(layout.bounds())` is already used in this file (line 447); `tree.children[0]` / `layout.children().next()` mirror the `update()` method (lines 449-453).
- Unused params `_tree`/`_layout`/`_cursor`/`_viewport`/`_renderer` become used — drop the leading underscores.
- Minimal fallback if delegation is unwanted: keep `Pointer` but gate it with `if cursor.is_over(layout.bounds()) { Pointer } else { None }`. Delegate is preferred (correct `Text` cursor over the editable value field).

## Task 2 — No other source changes
- Keep `src/ui/components/torrent_upload.rs` and `src/ui/theme.rs` as-is (drop-zone dashed border + scoped `interaction(Pointer)` from the previous plan).
- `app.rs`, `message.rs`, `add_dialog.rs`: no changes.

## Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual, with the freshly built binary (do not test against a stale one):
  - Open New Download dialog (both URL and Torrent tabs): hand cursor appears ONLY over the drop zone (empty state), buttons, and stepper +/− buttons; blank dialog areas and the dimmed overlay show the default arrow.
  - Text `|` cursor still shows over the stepper's value input and other text fields.
  - Drop-zone hover highlight (dashed border turns primary) still works; clicking the zone still opens the file picker.
  - Close the dialog: no hand cursor over the main window.

## Risks / Notes
- Do NOT re-add `.interaction(Pointer)` anywhere else; the leak is fully explained by the unconditional stepper interaction.
- If a hand cursor is still seen at runtime after this fix, rebuild clean (stale binary was a suspected factor before) and only then investigate windowing/cursor quirks.
