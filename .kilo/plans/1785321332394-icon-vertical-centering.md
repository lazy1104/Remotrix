# Icon-Button Borders + Remove Separators

## Goal
Fix the path-picker ("component indistinguishable from dialog background" in `add_dialog.rs`) by giving each icon button its own 4-sided border using the separator color. With bordered buttons, the explicit separator widgets between buttons become redundant and are removed.

## Root cause (verified)
- The icon buttons (`style::button::grouped_icon`) currently have a **transparent, width-0 border**. Their background is `background.weak.color` — the same as the dialog (`card`) background — so the buttons (and thus the picker) blend into the dialog.
- The 1px separators between buttons use `style::separator` → `border_color()` = `background.strong.color`. Those are visible; the buttons themselves are not outlined.

## Changes

### 1. Give each icon button a 4-sided border (`src/ui/theme.rs`, `style::button::grouped_icon`)
Change the `border` field from transparent/0 to the separator color, keeping the existing radius logic:
```rust
// add near the top of the closure, alongside weak_bg:
let border = t.extended_palette().background.strong.color;
...
border: iced::Border {
    color: border,       // was iced::Color::TRANSPARENT
    width: 1.0,         // was 0.0
    radius,             // unchanged: trailing -> right(RADIUS_BUTTON); else default
},
```
- Border color = `background.strong.color`, identical to the separator color (per request "套用图标间的边框颜色").
- Keep radius as-is (square for non-trailing copy/browse; right-rounded for trailing history) — minimal change.
- Keep button background `background.weak.color` unchanged; keep hover/pressed `lighten(weak_bg, …)`.
- The border is static across statuses (bg changes on hover, border stays) — standard outlined-button behavior.

### 2. Remove the now-redundant separators (`src/ui/path_picker.rs`)
- Delete the `fn separator()` helper (lines 13–19).
- Remove the two `separator()` pushes:
  - After copy button: `row = row.push(copy_btn).push(separator());` → `row = row.push(copy_btn);`
  - Before history button: delete the `row = row.push(separator());` line entirely.
- Remove now-unused `Space` from the import (line 3): `use iced::widget::{button, column, container, row, text, text_input, tooltip, Text};` — keep `Text` (used by `icon_content`).

### 3. Remove the dead `style::separator` helper (`src/ui/theme.rs`)
- Delete `pub fn separator(t: &iced::Theme) -> iced::widget::container::Style { … }` (currently only referenced by the removed `path_picker::separator()`). It is `pub` so it produces no clippy warning, but it is now unreachable — remove for cleanliness.

## Keep unchanged
- `style::grouped_frame` (picker outer frame) — still outlines the input region and overall picker; its `background.strong` border shares the same color as the new button borders, so outer edges stay a clean single line (no doubling).
- The `text_input` and the `icon_content` centering helper — unaffected.

## Known artifact (acceptable)
With iced's uniform `Border.width`, adjacent bordered buttons each draw their own 1px edge, so internal dividers between buttons (copy↔browse, browse↔history) render as 2px instead of the old 1px separators. The input↔copy divider is 1px (only copy's left border). This is an inherent consequence of per-button borders and replaces the old separators as intended.

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` (no warnings; confirm `Space` and `style::separator` removals introduce no unused-import/dead-code warnings)
- `cargo build`
- Visual: each icon button (copy / browse / history) shows a 4-sided outline; no separator bars between buttons; picker reads as a distinct control against the dialog in `add_dialog.rs` and in `settings_page.rs`.