# Plan: Scoped Pointer in Add Dialog + Dashed Torrent Drop Zone with Hover

## Goal
Two UI adjustments to the New Download dialog:

1. **Pointer cursor scope** — the pointer cursor should appear ONLY over the torrent upload component (drop zone), not over blank areas of the dialog.
2. **Drop zone styling** — the torrent upload component gets a **dashed border** and a **hover effect**: on hover the border color turns `primary`.

## Verified Constraints (from iced 0.14.2 / 0.14.0 sources)
- `container::Border` has **no dashed style** (only `color`, `width`, `radius`). `iced_core-0.14.0/src/border.rs`.
- **Canvas supports dashes**: `iced::widget::canvas` re-exports `Stroke` with `line_dash: LineDash { segments: &[f32], offset }` (`iced_graphics-0.14.0/src/geometry/stroke.rs`), plus `Path::rounded_rectangle(top_left, size, border::Radius)`, `Frame::new(renderer, size)`, `frame.stroke(&path, stroke)`, `frame.into_geometry()`. All confirmed available.
- `mouse_area(...).on_press(...)` in iced 0.14.2 does **NOT** set the pointer cursor (`on_press` only stores a message; cursor only changes via `.interaction(mouse::Interaction::Pointer)`). `Interaction::None` maps back to the default arrow cursor. So the only deterministic way to scope the pointer to the component is `.interaction(...)`.
- A canvas whose `Program::update` returns `None` does not capture events (`iced_widget-0.14.2/src/canvas.rs` `update`), so a canvas layer on top of the drop zone will not block the Browse click or the replace/clear buttons inside the stack.
- Codebase already uses `iced::widget::canvas` (`src/ui/components/piece_map.rs`) — canvas feature is enabled.

## Task 1 — Pointer scoping (`src/ui/components/torrent_upload.rs`)
- Add `.interaction(mouse::Interaction::Pointer)` to the empty-state drop zone `mouse_area` (import `iced::mouse`). This guarantees the pointer appears only over the 120px drop zone box.
- No other change needed for cursor scoping; no full-dialog interaction exists in the source. If the observed blank-area pointer persists after this change at runtime, it is an environment/cursor quirk — verify by running (see Validation), do not chase in code.

## Task 2 — Dashed border + hover effect

### State (`torrent_upload.rs`)
- Add field `hovered: bool` to `TorrentUpload`; init `false` in `new()`.
- Reset `hovered = false` in `clear()` and `set_path()` (alongside the existing `dragging` resets). `open()` already calls `clear()`.
- Add events to `TorrentUploadEvent`: `Entered`, `Exited`.
- `update()`: `Entered` → `self.hovered = true; None`; `Exited` → `self.hovered = false; None`. `Browse`/`Clear` unchanged.

### Canvas border program (`torrent_upload.rs`, private)
```rust
struct DashedBorder { color: Color, radius: f32, width: f32 }
impl canvas::Program<Message> for DashedBorder {
    type State = ();
    fn draw(&self, _s, renderer, _t, bounds, _c) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let inset = self.width / 2.0;
        let path = canvas::Path::rounded_rectangle(
            Point::new(inset, inset),
            Size::new(bounds.width - self.width, bounds.height - self.width),
            self.radius.into(),
        );
        let stroke = canvas::Stroke {
            style: canvas::Style::Solid(self.color),
            width: self.width,
            line_dash: canvas::LineDash { segments: &[4.0, 4.0], offset: 0 },
            ..Default::default()
        };
        frame.stroke(&path, stroke);
        vec![frame.into_geometry()]
    }
    fn update(&self, ..) -> Option<canvas::Action<M>> { None }
}
```
- Constant `DASH_SEGMENTS: [f32; 2] = [4.0, 4.0]` (dash/gap), `BORDER_WIDTH: f32 = 1.0`.
- In `view()`, compute `let border_color = if self.hovered || self.dragging { theme::accent(theme) } else { theme::border_color(theme) };`.

### Empty state view
Replace the single container with a `stack` wrapped in `mouse_area`:
```
stack![
    container(content).width(Fill).height(Fixed(120)).align_x(Center).align_y(Center)
        .style(theme::style::drop_zone(self.dragging)),
    canvas::Canvas::new(DashedBorder { color: border_color, radius: RADIUS_BUTTON, width: BORDER_WIDTH })
        .width(Length::Fill).height(Length::Fixed(120.0)),
].width(Fill).height(Fixed(120))
```
wrapped in:
```
mouse_area(...)
    .on_press(map(TorrentUploadEvent::Browse))
    .on_enter(map(TorrentUploadEvent::Entered))
    .on_exit(map(TorrentUploadEvent::Exited))
    .interaction(mouse::Interaction::Pointer)
```

### Filled state view
Same `stack` structure (content = filename/path + replace/clear buttons), wrapped in:
```
mouse_area(...)
    .on_enter(map(TorrentUploadEvent::Entered))
    .on_exit(map(TorrentUploadEvent::Exited))
```
(no `on_press`, no `interaction` — buttons inside remain clickable because the canvas layer does not capture events).

### Theme (`src/ui/theme.rs`)
- Change `drop_zone(active)` to a **background-only** style: keep the existing background tint + `text_color` logic, but set `border: iced::Border::default()` (width 0). The dashed border is now drawn by the canvas layer. Both empty and filled states use this style.

## Task 3 — Message plumbing (`src/app.rs`, `src/message.rs`)
- `Message::TorrentUpload` arm in `app.rs` already calls `state.add_dialog.torrent_upload.update(event)` and handles `Some(TorrentUploadAction::Browse)` → `pick_path`. `Entered`/`Exited`/`Clear` return `None` → no app.rs change required.
- `message.rs`: no change (`Entered`/`Exited` ride on the existing `TorrentUpload` message).
- Verify at build time that `Entered`/`Exited` are fully matched in `update()` (no wildcard needed; exhaustiveness).

## Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings allowed)
- `cargo fmt --check`
- Manual: open New Download → Torrent tab:
  - Pointer cursor appears only over the drop zone box; blank dialog areas show the default arrow.
  - Dashed border visible; hover → border turns `primary`; drag-over (file hovered) also highlights.
  - Click drop zone still opens the native file picker; drop a `.torrent` fills the path; clear/replace buttons in the filled state still work.
  - URL tab unaffected.

## Risks / Notes
- The reported "pointer over blank dialog areas" could not be reproduced from source (on_press does not set the cursor). Scoping via `.interaction(Pointer)` is the deterministic fix; if the stray pointer persists at runtime, treat it as an environment/cursor quirk and investigate separately (possibly a stale binary).
- Canvas is redrawn each frame (simple border) — negligible cost.
- Wayland drag-drop limitation unchanged (click-to-browse remains the fallback).
