# PathPicker: visible field background (all modes) + distinct icon buttons

## Goal
Give the path text field a visible muted background (`background.weak`) in **all** picker modes (editable folder/file AND read-only), so it reads as a field. Make the icon buttons (copy / reveal / browse / history) use the **page background (`background.base`)** so they visibly stand out from the weak field instead of blending with it.

This is a follow-up to the read-only hover fix. Scope is **path picker only** — `tag_picker` and `number_stepper` keep their current look.

## Current state
- `grouped_frame_state` (theme.rs:328) paints the row background `background.weak` only in `ReadOnly` mode, `background.base` otherwise. Shared by path_picker, tag_picker, number_stepper.
- `grouped_icon` (theme.rs:808) paints all icon buttons `background.weak` regardless of mode. Shared by path_picker and number_stepper.
- Input text styles are already distinct: `input::grouped` (editable, `background.base.text`) and `input::grouped_readonly` (muted, `background.weak.text`). These stay unchanged.

## Design decisions
- Path field background becomes `background.weak` for all modes (both editable and read-only).
- Icon buttons become `background.base` (page bg) on the path picker; hover lightens `base`.
- `number_stepper` remains editable — its field stays `background.base` and its icons stay `background.weak` (via `on_field=false`). `tag_picker` unchanged.
- `base.text` on `background.weak` is readable in both light/dark palettes, so the editable path text needs no change.

## Implementation tasks

### 1. `src/ui/theme.rs`
- **`grouped_frame_state`**: revert to 2-arg `(focused: bool, hovered: bool)`; background always `background.base.color` (remove the `read_only` param and the `background.weak` branch).
- **Add `grouped_field_state(focused: bool, hovered: bool)`** — same border logic as `grouped_frame_state` (primary border when `focused || hovered`, else `border_color`), but background `background.weak.color`. Used by the path picker.
- **`grouped_icon`**: add `on_field: bool` param → `grouped_icon(trailing: bool, on_field: bool)`. Pick the base color as `background.base.color` when `on_field` else `background.weak.color`; hover uses `lighten(base, 0.08)` / `lighten(base, 0.14)`; text stays `background.base.text`.

### 2. `src/ui/components/path_picker.rs`
- Frame (line ~271): use `grouped_field_state(self.focused, self.hovered)` (all modes) instead of `grouped_frame_state(..., self.mode == PickerMode::ReadOnly)`.
- Icon buttons (lines ~195, ~212, ~234, ~247): pass `on_field=true` → `grouped_icon(false, true)` / `grouped_icon(true, true)`.
- Input styles unchanged (`grouped` / `grouped_readonly` selection already mode-aware).

### 3. `src/ui/components/tag_picker.rs`
- Line 74: `grouped_frame_state(false, false)` (revert to 2 args).

### 4. `src/ui/components/number_stepper.rs`
- Line 324: `grouped_frame_state(state.focused, hovered)` (revert to 2 args).
- Lines 160, 177: `grouped_icon(false, false)` / `grouped_icon(true, false)` (`on_field=false`, unchanged behavior).

## Validation
- `cargo build` (no warnings).
- `cargo clippy --workspace` (no warnings).
- `cargo fmt --check`.
- Manual: Settings → Download (path pickers), Advanced (Engine Data Dir / Session file, read-only), Logging (Log Location). All path rows show a muted `background.weak` field; icon buttons are `background.base` and stand out; hover still shows the accent border; read-only rows keep muted path text; copy / reveal still work. Confirm number_stepper and tag_picker look unchanged.

## Risks / notes
- `grouped_icon` and `grouped_frame_state` signatures change; update all callers (path_picker, tag_picker, number_stepper) in the same step or the build fails.
- `grouped_field_state` duplicates the border logic of `grouped_frame_state`; acceptable small duplication for clarity.
- The editable path field remains editable (typing still works) — only the background changes; do not disable editing.