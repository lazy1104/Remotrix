# Make settings dropdowns (pick_list) match text input height

## Problem
After the font-size fix, dropdowns in the settings page are visibly **shorter** than
the text inputs (e.g. the BT tracker input). User requirement: pick_list height must
equal the text input height.

## Root cause
- Text inputs use `theme::input_layout` → `.padding(INPUT_PADDING)`, where
  `INPUT_PADDING = iced::Padding::new(8.0)` (8px on all sides).
- `iced_widget` `PickList` defaults to `crate::button::DEFAULT_PADDING`
  (`top: 5, bottom: 5, right: 10, left: 10`).
- Both use 13px text (FONT_MEDIUM) and the same default line height, so heights
  differ only by padding: 8+8=16px vs 5+5=10px → pick_list is ~6px shorter.
- `PickList::Style` has no padding field (padding is a widget builder field), so it
  must be set per widget via `.padding()`.

## Change
In `src/ui/settings_page.rs`, add `.padding(theme::INPUT_PADDING)` to all 5
`pick_list(...)` calls (same 5 locations that already have `.text_size(FONT_MEDIUM)`):

1. `font_family_row` (line ~272) — font family picker
2. `logging_view` (line ~1095) — app log level picker
3. `logging_view` (line ~1107) — engine log level picker
4. `labeled_pick` helper (line ~1272) — theme mode, locale, file allocation
5. `speed_labeled_input` (line ~1360) — KB/s vs MB/s unit picker

`theme::INPUT_PADDING` is `pub` in `src/ui/theme.rs:83` and already imported via
`use crate::ui::theme;`. `padding` accepts `impl Into<Padding>`, and `Padding` is
already the value type, so `.padding(theme::INPUT_PADDING)` compiles directly.

The closed pick_list height then equals `line_height(13px) + 16px`, identical to
`theme::input_layout` text inputs. Menu items inherit the same text size; menu
item padding is unaffected.

## Files
- `src/ui/settings_page.rs` (only)

## Validation
- `cargo check` compiles.
- `cargo clippy --workspace` passes with no warnings.
- `cargo fmt --check` passes.
- Manual: open Settings → the closed dropdowns (font family, theme mode, locale,
  log levels, file allocation, speed unit) are now the same height as the BT
  tracker text input.
