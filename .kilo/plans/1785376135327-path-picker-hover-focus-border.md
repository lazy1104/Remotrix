# Plan: PathPicker transient focus border flash (simplified)

## Goal
Add a brief **focus** border color flash on the active `PathPicker` grouped frame when the user presses an icon button (Copy / Browse / SelectHistory / ToggleHistory). The border reverts to idle when the pointer leaves the picker.

**Hover stays as-is = no hover border implementation.** The current `grouped_frame` border is static; it is NOT changed for hover. Only focus drives the border (2 states: idle, focused).

Read-only pickers (engine data dir / session / log via `labeled_readonly`) are out of scope — their Copy routes straight to `Message::CopyPath` (bypasses `picker.update`), so `focused` is never set; they keep the idle border.

## Decisions (confirmed)
- **Revert mechanism = `mouse_area::on_exit`** (user choice), **no timer**, no cross-picker/other-input clearing, no new `Message` variants. `Exited` is a component-level `PathPickerEvent` carried by the existing `Message::PathPicker(PathPickerId, PathPickerEvent)` — zero `message.rs` change.
- Verified `iced_widget-0.14.2/src/mouse_area.rs:310-340`: `on_exit` needs no attached `on_enter` because `state.is_hovered` is tracked internally every update; fires when `!is_hovered && was_hovered`. Inner disabled `text_input` never captures mouse-move → exit fires reliably. Inner buttons only capture press (not move), so `on_exit` on subsequent move-out still fires.
- `container::style()` accepts `impl Fn(&Theme) -> Style + 'a` (`iced_widget-0.14.2/src/container.rs:214`); a closure capturing `focused: bool` (`Copy`) is `'static` and type-correct.
- No `iced_aw::NumberInput`-style native-status approach: our display input has no `on_input` → its `text_input::Status` is permanently `Disabled` (`iced_widget-0.14.2:460-471`), so we synthesize via `mouse_area` + struct state (not a custom `Widget`/`draw`). Reference study recorded; approach unchanged.
- `PathPicked` (rfd result) is irrelevant to focus now (no persistence). No app-side focus logic.

## Palette (verified present via existing usage)
- idle border (focused=false): `t.extended_palette().background.strong.color` (= current `border_color`, `theme.rs:84`).
- focus border (focused=true): `t.extended_palette().primary.base.color` (= `accent`, `theme.rs:63`).
- background: `t.extended_palette().background.base.color` (current `grouped_frame`).
- radius `super::RADIUS_BUTTON`, border width `1.0` (current `grouped_frame`).

## Acceptable, noted edge (do NOT add code to fix)
- When the history dropdown is open, moving the cursor from the group onto the overlay leaves the group bounds → `on_exit` reverts focus briefly; selecting an item (`SelectHistory`) re-sets `focused=true`; then moving away reverts again. This is a minor cosmetic flicker, accepted for simplicity. Windows where the OS dialog (rfd) opens without cursor movement will keep focus until the cursor leaves — accepted (user choice).

## Implementation steps

### 1. `src/ui/components/path_picker.rs`
- Import `iced::widget::mouse_area` (add to the existing `iced::widget::{...}` import line).
- Add field `focused: bool` to `PathPicker`; init `false` in `folder`/`file`/`read_only`.
- Add variant `PathPickerEvent::Exited` (unit variant; enum already derives `Clone`).
- `update`:
  - `PathPickerEvent::Exited => { self.focused = false; None }`
  - Set `self.focused = true` at the top of each activating arm (before existing logic):
    - `ToggleHistory` (guarded `mode != ReadOnly`) → `focused=true; history_open = !history_open; None`
    - `SelectHistory(p)` → `focused=true; history_open=false; Some(Select(p))`
    - `Browse` → `focused=true; Some(Browse)`
    - `Copy(s)` → `focused=true`; keep existing empty-guard (returns `None` if empty, else `Some(Copy(s))`). Note: empty Copy never arrives (button disabled), guard retained defensively.
  - `DismissHistory` → unchanged (`history_open=false; None`); do NOT touch `focused` here (the cursor-out `Exited` handles revert).
- In `view`:
  - Build `inner: Element<'a, M>`:
    - group = `container(row)...style(theme::style::grouped_frame_state(self.focused))` (replace `grouped_frame`).
    - history path → `drop_down::DropDown::new(group, overlay, self.history_open).on_dismiss(...)`.into()
    - else → `group`.into()
  - Wrap conditionally (skip readonly to avoid emitting `Noop` on their mouse-leave):
    ```rust
    if self.mode != PickerMode::ReadOnly {
        mouse_area(inner)
            .on_exit(map(PathPickerEvent::Exited))
            .into()
    } else {
        inner
    }
    ```
    This returns the final element (the current early `return` for the dropdown path must be folded into building `inner` first, then the single `mouse_area` wrap).

### 2. `src/ui/theme.rs` (`style` module)
- Replace `pub fn grouped_frame(t) -> container::Style` with:
  ```rust
  pub fn grouped_frame_state(focused: bool) -> impl Fn(&iced::Theme) -> iced::widget::container::Style {
      move |t| iced::widget::container::Style {
          background: Some(t.extended_palette().background.base.color.into()),
          border: iced::Border {
              color: if focused { t.extended_palette().primary.base.color }
                     else { super::border_color(t) },
              width: 1.0,
              radius: super::RADIUS_BUTTON.into(),
          },
          ..Default::default()
      }
  }
  ```
  `grouped_frame` had no other callers (grep: only `path_picker.rs:194`), so removal is safe.

### 3. `src/app.rs` & `src/message.rs`
- **No changes.** Verify by reading: `Message::PathPicker(id, event)` handler at `app.rs:234-245` calls `picker_mut(state, id).update(event)` and dispatches returned actions; `Exited` returns `None` → handled transparently. No new message needed.

## Affected files
- `src/ui/components/path_picker.rs` (focused field, Exited event, update rules, mouse_area wrap, style fn swap)
- `src/ui/theme.rs` (replace `grouped_frame` with `grouped_frame_state`)

## Validation
- `cargo build`; `cargo clippy --workspace` (confirm `grouped_frame` removal leaves no unused-fn warning); `cargo fmt --check`.
- Manual:
  - Press Copy on an active picker → border flashes primary base color; move mouse out → reverts to idle.
  - Press Browse → border flashes; rfd/OS dialog appears; on return + mouse-out → reverts.
  - Press folder_clock → border flashes + dropdown opens; click an item → field updates + border flashes; move out → reverts.
  - Open dropdown, click outside → closes (DismissHistory); cursor-out reverts focus.
  - Read-only engine-path pickers (Advanced) → border stays idle throughout (expected).
  - No `hover` border change appears (expected — hover out of scope).