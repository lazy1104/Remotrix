# Borderless Window Edge-Drag Resize

## Goal
Allow the borderless window (`decorations: false`) to be resized by dragging its edges/corners, and enforce a minimum window size of **800 x 560**.

## Feasibility (confirmed)
- iced 0.14 → winit 0.30.13. Native OS resize borders are removed by `decorations: false`, so OS-level edge hit-testing is **not** available.
- iced exposes `iced::window::drag_resize(id: Id, direction: Direction) -> Task<T>` (`iced_runtime::window::drag_resize`), which calls winit's `Window::drag_resize_window(ResizeDirection)`. This programmatically starts a system resize drag and **works on borderless windows** (required on Wayland; works on Windows/macOS/X11).
- `Direction` (`iced::window::Direction`): `North, South, East, West, NorthEast, NorthWest, SouthEast, SouthWest`.
- Minimum size: set via `iced::window::Settings::min_size: Option<Size>` at startup (no runtime command needed). `iced::window::set_min_size(id, size)` also exists if needed later.
- `mouse_area` widget supports `.on_press(msg)`, `.on_enter/.on_exit`, and `.interaction(mouse::Interaction)` for resize cursors.
- `mouse::Interaction` resize variants: `ResizingHorizontally`, `ResizingVertically`, `ResizingDiagonallyUp` (→ `NeswResize`, for NE/SW corners), `ResizingDiagonallyDown` (→ `NwseResize`, for NW/SE corners).
- `Stack` hit-testing: a non-interactive center cell (`container` with no `mouse_area`) does **not** capture pointer events, so clicks in the center pass through to the content layer below (verified in `iced_widget-0.14.2/src/stack.rs` `update`, lines 262-274).

## Design
Add an invisible 8-strip "resize frame" overlay on top of the main content (but below modal dialogs). Each strip is a `mouse_area` whose `on_press` emits `Message::ResizeWindow(direction)`; the handler calls `iced::window::drag_resize(id, direction)`. Strips also set the matching resize cursor via `.interaction(...)`.

Frame = 3x3 grid (column of rows). Center cell is a plain transparent `container` (pass-through). `BORDER = 6.0` px hit zone; corners are `BORDER x BORDER`.

```
row![ NW(ResizingDiagonallyDown) , N(ResizingVertically, Fill x BORDER) , NE(ResizingDiagonallyUp) ]   // height BORDER
row![ W(ResizingHorizontally, BORDER x Fill) , center(transparent Fill x Fill) , E(ResizingHorizontally, BORDER x Fill) ]  // height Fill
row![ SW(ResizingDiagonallyUp) , S(ResizingVertically, Fill x BORDER) , SE(ResizingDiagonallyDown) ]  // height BORDER
```

Direction → cursor mapping:
- N, S → `ResizingVertically`
- E, W → `ResizingHorizontally`
- NW, SE → `ResizingDiagonallyDown`
- NE, SW → `ResizingDiagonallyUp`

### Layering / interactions
- Overlay sits **above** `base` (content+titlebar) and **below** modal dialogs (add/about/close/details). So dialogs still capture input and block resize while open.
- The top 6px strip overlaps the title bar: top edge → North resize; the rest of the title bar (38px) still drags via its existing `mouse_area`/`DragWindow`. This is standard borderless-app behavior.
- Edge strips overlap 6px of sidebar/category/content edges — harmless (those are background edges).
- When `state.maximized` is true, **skip rendering the frame** (can't/shouldn't resize a maximized window). Note: `maximized` is currently only toggled by the title-bar button and may not reflect OS snap; acceptable for now — out of scope to add a `Resized`/maximize subscription.

## Tasks

### 1. `src/message.rs`
- Add variant `ResizeWindow(iced::window::Direction)` to `Message` (next to `DragWindow`).
  - `Direction` is `Copy`, so `Message` (already `Clone`) stays fine; `#[derive(Debug, Clone)]` continues to apply.

### 2. New module `src/ui/resize_frame.rs`
- `pub const BORDER: f32 = 6.0;`
- `pub fn view<'a>() -> Element<'a, Message>` building the 3x3 grid described above using `iced::widget::{row, column, container, mouse_area}` and `iced::Length`.
- Helper `fn strip(direction, interaction, width, height) -> Element` → `mouse_area(container(...).width(width).height(height)).on_press(Message::ResizeWindow(direction)).interaction(interaction)`.
- Corners: `Length::Fixed(BORDER)` x `Length::Fixed(BORDER)`.
- N/S: `Length::Fill` x `Length::Fixed(BORDER)`. E/W: `Length::Fixed(BORDER)` x `Length::Fill`.
- Center: plain `container(text("").size(1))` (or empty container) `Fill x Fill` — **no `mouse_area`**, non-interactive so events pass through.
- Root: `column![top_row, mid_row, bottom_row].width(Length::Fill).height(Length::Fill).into()`. Transparent (no style) so it draws nothing.
- Imports per conventions: `iced` widgets → `crate::message::Message`.

### 3. `src/ui/mod.rs`
- Add `pub mod resize_frame;`.

### 4. `src/app.rs` — `view`
- Wrap `base` with the resize frame as a stack layer above it, before dialogs:
  ```rust
  let framed = stack![
      iced::widget::opaque(base),
      crate::ui::resize_frame::view(),
  ]
  .width(Length::Fill)
  .height(Length::Fill);
  ```
  Then use `framed` in place of `iced::widget::opaque(base)` as the bottom layer the dialogs stack onto (i.e., `let mut stacked = framed;`).
- When `state.maximized` is true, omit the overlay layer (just use `iced::widget::opaque(base)`).

### 5. `src/app.rs` — `update`
- Add arm:
  ```rust
  Message::ResizeWindow(direction) => {
      if let Some(id) = state.window_id {
          return iced::window::drag_resize::<Message>(id, direction);
      }
  }
  ```
  (Mirrors existing `Message::DragWindow` handler.)

### 6. `src/main.rs` — window settings
- In `iced::window::Settings { .. }`, add:
  ```rust
  min_size: Some(iced::Size::new(800.0, 560.0)),
  ```
- Keep `decorations: false`, `resizable: true` (default), `exit_on_close_request: false`.

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` (no warnings)
- `cargo build`
- `cargo run --` and manually:
  - Drag each of the 8 edges/corners → window resizes; cursor shows correct resize icon on hover.
  - Center clicks still reach content (buttons, task list, sidebar, title-bar drag) — no dead zones.
  - Resize down to the 800x560 floor → OS clamps at minimum; cannot shrink below.
  - Maximize → edge strips no longer capture (no resize cursor); restore → works again.
  - Open a modal (Add/About/Close/Details) → edge resize does not trigger (dialog backdrop captures).

## Risks / Notes
- `drag_resize_window` on a maximized window is platform-dependent; mitigated by skipping the overlay when `maximized` is true.
- `state.maximized` is not synced from OS snap/maximize events (only the title-bar button toggles it). Adding a `Resized`/maximize subscription is out of scope here.
- `BORDER` (6px) is a UX knob; adjust if edge grab feels too small/large.
- Min size 800x560 is enforced by the WM at creation; no runtime change needed.
