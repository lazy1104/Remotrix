# Fix: NumberStepper focus border not showing primary color

## Root Cause
`NumberStepper` tracks `state.focused` via `FocusProbe` + `BufferReader` Operations that probe the child text_input's `Focusable::is_focused()`. Due to timing between the runtime's focus Operations (unfocus/focus) and our `update()` call, `state.focused` is never set to `true` reliably. The accent border (primary base color) is never shown.

Additionally, `draw()` computes `hovered` locally via `cursor.is_over()` but ignores `state.hovered` which was already set in `update()`.

## Solution
Replace the Operation-based focus detection with a direct **active-state approach** matching `PathPicker`: track interaction within the widget bounds and set `state.focused` on any relevant mouse/keyboard event within our layout.

### Changes to `src/ui/components/number_stepper.rs`

**State** — keep `StepperState` as-is (buffer, focused, hovered).

**Remove** `FocusProbe` and `BufferReader` Operation structs and their `impl Operation` blocks — unused after the change.

**`update()`** — after computing `state.hovered` and forwarding the event:
1. After the event is forwarded to the child, check if `cursor.is_over(layout.bounds())` AND the event is a mouse press/click. If so, set `state.focused = true`.
2. If the event is not within bounds AND not a mouse press, check if `state.focused` should remain. Use a heuristic: `state.focused = state.focused && state.hovered` — i.e., stay focused only while the mouse is over the widget.
3. On `state.focused` transitioning `true → false` (blur): perform clamp logic and publish via `shell.publish()`.
4. On `state.focused` transitioning `false → true` (focus): sync buffer from external value.

**`diff()`** — keep the same conditional `diff_children` logic based on `state.focused`.

**`draw()`** — use `state.hovered` instead of local `cursor.is_over()` computation for consistency:
```rust
let frame_style = theme::style::grouped_frame_state(state.focused, state.hovered);
```

**Buffer reading** — the text_input's value is still needed for blur-clamp. Instead of `BufferReader`, read it by running a simpler Operation that calls `operation.text_input()` and stores `state.text()`. OR, since the editor buffer is now managed locally (not via BufferReader), just store the last `on_input` value in `StepperState.buffer` (update it in the `on_input` closure via a shared mechanism).

Actually, the cleanest approach: **store buffer in Tree State only**, and in the text_input's `on_input` closure inside `build_row`, capture the tree... no, the closure can't access Tree.

**Simplest working approach**: Track `state.active` (synthetic "focused" like PathPicker) instead of real keyboard focus. Remove FocusProbe entirely. Set `state.active` to `true` on any mouse event within bounds, and `false` on mouse exit. This matches PathPicker exactly.

The `buffer` is still needed for blur-clamp: read the text_input's value via a direct `text_input` Operation (keep `BufferReader` but it's simpler now — we only need it for reading the current text on blur).

## Tasks
1. **`src/ui/components/number_stepper.rs`**:
   - Remove `FocusProbe` struct + `impl Operation`
   - Simplify `BufferReader` (keep only `text_input` handler, no `focusable` needed)
   - In `update()`: set `state.focused = true` on any mouse event within bounds; set `state.focused = false` when `!state.hovered && event.is_mouse() && cursor_just_left`
   - In `update()`: on `state.focused` transition `true→false`, use `BufferReader` to read text_input's current text, parse, clamp, publish
   - In `draw()`: use `state.hovered` (already set in `update()`) instead of local `cursor.is_over()`

2. **Verification**:
   - `cargo build`
   - `cargo clippy --workspace`
   - `cargo fmt --check`
   - Manual: focus border shows primary on click, hover shows text_secondary, idle shows border_color
