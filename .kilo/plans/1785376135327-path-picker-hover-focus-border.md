# Plan: PathPicker hover + focus border color states

## Goal
Make the active `PathPicker` fields (DownloadDir / SaveDir / Torrent) react like iced's native text input:
- **Hover** (cursor over the grouped frame) → border color shifts.
- **Focus** (after pressing any icon button inside the picker) → border shifts to a focus color, and clears when the user interacts elsewhere (`DismissHistory`, another picker, another input field, navigation/dialog open).

Read-only pickers (engine data dir / session file / log file in `labeled_readonly`) are **out of scope**: they are reconstructed each frame with no per-instance state and bypass `picker.update`, so they keep the current static border.

## Decisions (confirmed)
- Hover via `iced::widget::mouse_area` wrapping the grouped frame, `.on_enter`/`.on_exit` emit `PathPickerEvent::Hovered(bool)`. Inner buttons keep working (mouse_area defers to children first; verified in `iced_widget-0.14.2/src/mouse_area.rs:250-255`).
- Focus is **synthetic** (display text_input stays disabled; no `on_input`). Pressing any icon button (Browse / SelectHistory / Copy / ToggleHistory) sets the owning picker `focused = true`.
- **Reference study — `iced_aw::NumberInput` (confirmed rationale, approach unchanged):**
  - `iced_aw-0.14.1/src/widget/number_input.rs` gets its input-frame hover/focus border from the inner `text_input`'s **native** `text_input::Status` (`Hovered` / `Focused { is_hovered }`), able to do so because its input has `on_input` and is genuinely focusable. The +/- buttons are drawn via a **custom `Widget`** that manually hit-tests `layout.bounds().contains(cursor)` and tracks `ModifierState { increase_pressed, decrease_pressed }` in `Tree::state`, painting button backgrounds in `draw` via `renderer.fill_quad` (`iced_aw-0.14.1/src/style/number_input.rs:11-16` defines `Style { button_background, icon_color }`, mapped over `Status { Active, Hovered, Pressed, Disabled }`).
  - We **cannot** copy that directly: our `PathPicker` display input has no `on_input`, so `iced_widget-0.14.2:460-471` keeps its `text_input::Status` permanently `Disabled` → never reports native `Hovered`/`Focused`. Hence hover/focus must be **synthesized** via `mouse_area` (composition) rather than read from the input's own status machine.
  - We also do **not** adopt the custom-`Widget`+`draw`+`Tree::state` route: that's heavyweight and our `PathPicker` is a plain function `view` returning composed elements, not a custom `Widget`. The `mouse_area` + struct-held state model fits the existing composition pattern.
- Focus clears (`focused = false`) on:
  1. `DismissHistory` — iced_aw `on_dismiss` fires on outside-click or Escape (verified `iced_aw-0.14.1/src/widget/drop_down.rs:413-432`), i.e. "click elsewhere" while the history dropdown is open.
  2. An activating event on a *different* picker → that picker becomes focused, others lose focus (single global focus among the 3 active pickers).
  3. Finger-keyboard interactions of OTHER (non-picker) inputs: `UrlEditor`, `SplitChanged`, `SettingChanged`, `UaEditor`, `HeadersEditor` (app clears all picker focus).
  4. Navigation/dialog events that already call `close_history()` (`NavigatePage`, `SetSettingsCategory`, `OpenAddDialog`, `CancelAdd`) also clear all picker focus.
- `PathPicked` (rfd result) **keeps** the picker focused (user just acted on that field). Listed as a deliberate decision; revisit if UX feels sticky.
- **Limitation (noted honestly):** iced 0.14 has no broadcast focus event; we cannot clear picker focus on arbitrary blank-area clicks outside any control. The approximation covers the common cases (history dismiss, sibling picker, other inputs, navigation). Roaming clicks on empty window chrome are not covered.
- `container::style()` accepts `impl Fn(&Theme) -> Style + 'a` (verified `iced_widget-0.14.2/src/container.rs:214`) — a stateful closure capturing `hovered`/`focused` (both `Copy`) is `'static`-friendly and type-correct.

## Palette (all verified present via existing usage)
- idle border: `t.extended_palette().background.strong.color` (= current `border_color`, `theme.rs:84`)
- hover border: `t.extended_palette().primary.weak.color` (used at `theme.rs:483`)
- focus border: `t.extended_palette().primary.base.color` (= `accent`, `theme.rs:63`)
- background + radius + width: unchanged from current `grouped_frame` (`background.base.color`, `RADIUS_BUTTON`, width `1.0`).

## Implementation steps

### 1. `src/ui/components/path_picker.rs`
- Add fields `hovered: bool` and `focused: bool` to `PathPicker` (`Default::default()` = false in `folder`/`file`/`read_only`).
- Add `PathPickerEvent::Hovered(bool)` variant.
- `update` rules (augment existing match):
  - `Hovered(b)` → `self.hovered = b; None`
  - `Browse` / `SelectHistory(_)` / `Copy(s)` / `ToggleHistory` → set `self.focused = true` before existing logic (Copy still returns `None` when empty, but empty Copy never fires since the button is disabled — keep guard).
  - `DismissHistory` → `self.history_open = false; self.focused = false; None` (add focus clear).
- Add `pub fn set_focused(&mut self, b: bool)`.
- In `view`:
  - Replace `.style(theme::style::grouped_frame)` on the group container with `.style(theme::style::grouped_frame_state(self.hovered, self.focused))`.
  - Build the element so the DropDown case is also wrapped: `inner` = the group container (or `DropDown::new(group, overlay, open).on_dismiss(...)` when history shown). Then wrap the result: `mouse_area(inner).on_enter(map(PathPickerEvent::Hovered(true))).on_exit(map(PathPickerEvent::Hovered(false)))`. Return that. Import `iced::widget::mouse_area`.
  - Keep existing `iced_aw::drop_down::DropDown` import; only the wrapping changes.
- `read_only` pickers: unchanged map closure (`Copy → CopyPath`, else `Noop`); `update` never invoked for them → hovered/focused stay false → idle border. (Intentional, per scope.)

### 2. `src/ui/theme.rs` — `style` module
- Add `pub fn grouped_frame_state(hovered: bool, focused: bool) -> impl Fn(&iced::Theme) -> iced::widget::container::Style + 'a` returning a closure that computes border color:
  - `if focused { primary.base.color } else if hovered { primary.weak.color } else { background.strong.color }`
  - background `background.base.color`, radius `RADIUS_BUTTON`, border width `1.0` (mirror current `grouped_frame`).
- Keep existing `grouped_frame` fn (idle) as the `hovered=false, focused=false` case OR delete it (only caller is path_picker). Prefer deleting after confirming no other callers (grep shows only `path_picker.rs:194`).

### 3. `src/app.rs`
- Add helpers (use existing `picker_mut`):
  - `fn clear_other_pickers_focus(state: &mut Remotrix, except: PathPickerId)` — for each `id != except`, `picker_mut(state, id).set_focused(false)`.
  - `fn clear_all_pickers_focus(state: &mut Remotrix)` — `set_focused(false)` on DownloadDir, SaveDir, Torrent.
- `Message::PathPicker(id, event)` handler (rewrite existing):
  ```rust
  let is_activating = matches!(
      &event,
      PathPickerEvent::Browse
          | PathPickerEvent::SelectHistory(_)
          | PathPickerEvent::Copy(_)
          | PathPickerEvent::ToggleHistory
  );
  let action = picker_mut(state, id).update(event);
  if is_activating { clear_other_pickers_focus(state, id); }
  match action {
      Some(PathPickerAction::Copy(s)) => return iced::clipboard::write::<Message>(s),
      Some(PathPickerAction::Browse) => return pick_path(id),
      Some(PathPickerAction::Select(p)) => apply_path(state, id, p),
      None => {}
  }
  ```
  (`Hovered` and `DismissHistory` are not activating → no cross-picker clear; `DismissHistory` clears this picker's own focus inside `update`.)
- `Message::PathPicked` handler: keep existing apply_path; do NOT clear focus (decision above).
- Add `clear_all_pickers_focus(state)` to handlers: `UrlEditor`, `SplitChanged`, `SettingChanged`, `UaEditor`, `HeadersEditor`, `SetSettingsCategory`, `OpenAddDialog`, `CancelAdd`, `NavigatePage` (next to the existing `close_history()` calls where present).
- No structural/schema changes; `PathPickerEvent::Hovered` rides through `Message::PathPicker(PathPickerId, PathPickerEvent)` — no `message.rs` edit beyond the variant added in step 1.

### 4. `src/message.rs`
- No diff beyond what step 1's new `PathPickerEvent::Hovered` variant implies (enum lives in component; `Message::PathPicker` already carries it). No action required here.

## Affected files
- `src/ui/components/path_picker.rs` (hovered/focused fields, Hovered event, update rules, mouse_area wrap, style fn swap)
- `src/ui/theme.rs` (add `grouped_frame_state`; remove/keep `grouped_frame`)
- `src/app.rs` (focus helpers, PathPicker handler rewrite, clear_all_pickers_focus in ~9 handlers)

## Risks / watch
- **mouse_area + DropDown composition**: verify the mouse_area does not swallow the dropdown overlay's outside-click dismissal (mouse_area only has on_enter/on_exit, no on_press; should be inert to the dismiss click). Test by opening history then clicking outside.
- **on_exit while dropdown open**: cursor leaving the group while the overlay is shown sets `hovered=false` but `focused=true` (from ToggleHistory) stays → border remains focus-colored. Expected; verify rendering.
- **`primary.weak` for light themes**: confirm the opaline builtin themes all define `primary.weak` (line 483 already uses it, so existing build proves it compiles; verify visual contrast in light theme at runtime).
- **Single-focus assumption**: only 3 active pickers exist simultaneously that matter (save/torrent in add dialog; download in settings). When add dialog is closed, save/torrent pickers still hold focus flags — harmless, and `OpenAddDialog`/`CancelAdd` clear them. Verify no stale focus border leaks across dialog open/close.
- **clippy no warnings; `cargo fmt` clean.**

## Validation
- `cargo build`; `cargo clippy --workspace`; `cargo fmt --check`.
- Manual:
  - Hover each active picker (settings download dir, add dialog save dir, add dialog torrent) → border shifts to hover color.
  - Press Browse → border shifts to focus color; rfd opens.
  - Open history dropdown, select an item → border focus color stays then field updates.
  - Open history dropdown, click outside / press Esc → dropdown closes AND border returns to idle.
  - Press Copy → border focus color; clipboard receives the path.
  - Click into the URL editor / split input / a settings number input → previously-focused picker border returns to idle.
  - Switch settings category / open add dialog / cancel add / navigate → all picker borders idle.
  - Read-only engine-path pickers in Advanced → border stays idle on hover (expected per scope).