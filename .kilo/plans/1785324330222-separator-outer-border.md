# Separator Lines + Outer Border (revert per-button borders)

## Goal
Revert the per-icon-button borders (which caused 2px internal dividers). Instead, place 1px separator lines between every adjacent element (input↔copy, copy↔browse, browse↔history) and rely on the existing outer `grouped_frame` border to outline the whole component. Buttons return to borderless.

This is essentially a revert of the prior "Icon-Button Borders" change PLUS one new separator (input↔copy) that the original never had. Caller signatures (`path_picker::view`) are unchanged, so `add_dialog.rs` and `settings_page.rs` need no edits.

## Root context (verified)
- iced 0.14 `Border` has a single uniform `width: f32` — no per-side borders (`iced_core-0.14.0/src/border.rs:5-15`). Per-button borders therefore can't avoid doubling at internal edges; separators avoid this entirely (one 1px widget per gap).
- `grouped_frame` (`theme.rs`) already draws a 1px `background.strong` border with `RADIUS_BUTTON` — this IS "the whole component border". Keep it unchanged.
- Separator color = `border_color(t)` = `background.strong.color` — same as the outer frame border, so separators and the outer border form one cohesive grid (matching the original design).
- Separator fills `Length::Fill` within the 36px frame; iced draws the container border over the frame edge, so separator ends are cleanly bounded by the frame's top/bottom border (no doubling).

## Files & exact edits

### 1. `src/ui/theme.rs` — `style` module
**a) Re-add `pub fn separator`** (removed earlier). Insert it between `card` and `grouped_frame`:
```rust
    pub fn separator(t: &iced::Theme) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(super::border_color(t).into()),
            ..Default::default()
        }
    }
```

**b) Revert `grouped_icon`** (`style::button::grouped_icon`) to borderless:
- Delete the line `let border_color = t.extended_palette().background.strong.color;`
- Change the `border` field back to:
```rust
                    border: iced::Border {
                        color: iced::Color::TRANSPARENT,
                        width: 0.0,
                        radius,
                    },
```
- Leave `weak_bg`, the hover/pressed `lighten(weak_bg, …)` background, `text_color`, and the `trailing` radius logic unchanged.

### 2. `src/ui/path_picker.rs`
**a) Re-add `Space` to the import** (line 3):
```rust
use iced::widget::{button, column, container, row, text, text_input, tooltip, Text, Space};
```

**b) Re-add the `separator()` helper** before `fn icon_content`:
```rust
fn separator() -> Element<'static, Message> {
    container(Space::new())
        .width(1.0)
        .height(Length::Fill)
        .style(theme::style::separator)
        .into()
}
```

**c) Insert three separators** in `view` (row uses `.spacing(0)`, so separators sit flush between elements):
1. **input ↔ copy** — after `row = row.push(input);` and before the `let copy_btn` block:
   ```rust
   row = row.push(input);
   row = row.push(separator());
   ```
2. **copy ↔ browse** — inside `if let Some(pid) = id`, before `row = row.push(browse_btn);`:
   ```rust
   row = row.push(separator());
   row = row.push(browse_btn);
   ```
3. **browse ↔ history** — inside `if show_history {`, as the first statement before the `if history.is_empty()` branch:
   ```rust
   if show_history {
       row = row.push(separator());
       if history.is_empty() {
   ```

Resulting element order: `input | sep | copy | sep | browse | sep | history` (when history present); `input | sep | copy` when no `id`; `input | sep | copy | sep | browse` when `id` but no history.

## Keep unchanged
- `grouped_frame` (outer component border — already satisfies "整个组件给一个边框").
- `style::input::grouped` (transparent input), `icon_content` centering helper, `trailing` radius logic.
- Callers `add_dialog.rs` / `settings_page.rs` (no signature change).

## Known artifact (pre-existing, out of scope)
Non-trailing buttons use square `Radius::default()` while the frame uses `RADIUS_BUTTON`, so in the 1- and 2-button layouts the last (square) button's fill corner may not perfectly follow the frame's rounded corner. This behaviour is identical to the original code and is not introduced by this change; do not adjust radius logic here.

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` (confirm re-added `Space`/`separator` produce no unused/dead-code warnings; `style::separator` is `pub` so no dead-code warning)
- `cargo build`
- Visual: in `add_dialog` and `settings_page` download-folder row — 1px strong separators between input↔copy and between every icon pair; outer rounded border around the whole picker; no 2px doubling anywhere.