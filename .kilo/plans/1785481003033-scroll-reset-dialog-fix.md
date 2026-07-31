# Fix: Opening dialogs resets task list / settings scroll position

## Problem

Clicking "新建/New" (or any action that opens a dialog) causes the scrollbar in
`ui/task_list.rs` and `ui/settings_page.rs` to jump back to the top.

## Root cause

iced preserves widget state (including `scrollable` scroll offsets) via the widget
`Tree`, diffed each frame. `Tree::diff` (iced_core `widget/tree.rs:57`) resets an
entire subtree whenever the widget **tag** at a given tree position changes.

In `src/app.rs:1399-1464` (`App::view`) the dialog layers are added by *re-nesting*:

```rust
let mut stacked = stack![framed, hud_overlay]...;      // root = Stack[framed, hud]
if add_dialog.is_visible() {
    stacked = stack![stacked, add_dialog::view(...)];  // root = Stack[Stack[framed,hud], dialog]
}
```

This shifts the base content one level deeper and changes the tree shape:

- Dialog closed: root children `[framed, hud_overlay]`  →  child0 = `Opaque` or `Stack[Opaque, resize]`
- Dialog open:   root children `[Stack[framed,hud], dialog]` → child0 = `Stack`, whose child0 = `framed`

At the point of mismatch (e.g. `Opaque` vs `Stack`) the tag differs, so
`*self = Self::new(new)` re-creates the whole base subtree, dropping every
`scrollable::State` (scroll offsets) inside it. Hence the jump to top. Same for
about/close/details/confirm/toast layers.

## Fix

Keep the widget tree **structurally constant**: always build a single `Stack`
with a fixed number of children, where each dialog layer is either the dialog
element or a `Space::new()` placeholder when hidden.

Verified against iced 0.14 source:
- `Stack::diff` → `tree.diff_children` zips children pairwise by index (state kept when tags match at each position).
- `Stack::push` drops only children with `size_hint().is_void()` (i.e. `Length::Fixed(0.0)`); `Space::new()` is `Shrink/Shrink` (non-void) and all dialog overlays are `Fill` (non-void), so all 7 layers are always present.
- `Space` has `mouse_interaction = None`, so invisible placeholder layers do not block events or levitate the cursor.

### Changes (only `src/app.rs`, in `App::view`)

Replace the `let mut stacked = stack![...]` block plus the six conditional
`stack![stacked, ...]` re-wraps (lines ~1399-1464) with fixed layers. Keep the
exact z-order of the current code (later = on top):

1. base      — `stack![framed, hud_overlay].width(Fill).height(Fill)`
2. add       — if `state.add_dialog.is_visible()` → `add_dialog::view(&state.fluent, t, &state.add_dialog, &state.settings.path_history)`, else `Space::new()`
3. about     — if `state.about_dialog_visible` → `about_dialog::view(&state.fluent, t, state.aria2_version.as_deref())`, else `Space::new()`
4. close     — if `state.show_close_dialog` → `close_dialog::view(&state.fluent, t)`, else `Space::new()`
5. details   — if `state.details.is_visible()` → `details_dialog::view(&state.fluent, t, task, &state.details)` (keep existing `task` lookup), else `Space::new()`
6. confirm   — if `let Some(action) = &state.confirm` → `confirm_dialog::view(&state.fluent, t, action)`, else `Space::new()`
7. toasts    — if `!state.toasts.is_empty()` → `components::toast::view(t, &state.toasts)`, else `Space::new()`

Then:

```rust
let stacked: iced::Element<'_, Message> = stack![
    base_layer, add_layer, about_layer, close_layer,
    details_layer, confirm_layer, toast_layer,
]
.width(Length::Fill)
.height(Length::Fill)
.into();

container(stacked)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
```

Notes:
- Each layer needs an explicit type annotation (`iced::Element<'_, Message>`) so the if/else arms (`Element` vs `Space.into()`) unify; use `let layer: iced::Element<'_, Message> = if ... { ... } else { iced::widget::Space::new().into() };`.
- Do not use `iced::widget::Space::new()` with `.width(Fixed(0.0))` — that would be void and be dropped by `Stack::push`, shifting layer indices and re-breaking diff stability.
- No changes needed in `task_list.rs`, `settings_page.rs`, or dialog components.

## Validation

1. `cargo build`
2. `cargo clippy --workspace` — no warnings
3. `cargo fmt --check`
4. Manual (`cargo run --`):
   - Add many tasks, scroll the task list down, click 新建 → scroll position must be preserved.
   - Open settings, scroll down, then click 新建 / 关于 / 删除确认 / toast → settings scroll position must be preserved.
   - Verify dialogs still render full-screen with dimming, and z-order is unchanged (toasts on top, etc.).
   - Verify scroll restore also survives the sidebar 新建 button (same `Message::OpenAddDialog`).

## Out of scope

- Scroll position across page switches (Tasks ↔ Settings) — state is intentionally recreated there.
- The `toast::view` inner `stack` child-count variability — lives above the base subtree and holds no scrollables.
