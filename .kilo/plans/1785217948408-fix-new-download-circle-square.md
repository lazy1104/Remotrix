# Plan: Fix New-Download button rendering as an ellipse

## Problem
After the prior restyle, the New-Download toolbar button renders as an **ellipse (taller than wide)** instead of a perfect circle. Root cause: the lucide `plus` glyph's line-box height (font ascent+descent) exceeds its advance width, so symmetric `padding([6, 6])` + a size-15 glyph produces a non-square button (~27 wide × ~30 tall). iced's `button::layout` uses `layout::padded` which places content at the padding offset with **no auto-centering**, and `border::rounded(40)` only clamps to a circle when the button quad is square — hence the ellipse.

## Decision
Apply the fallback already documented in the prior plan: **enforce the square with a fixed-size centered container**, and give the button `padding(0)` so the button bounds equal the container bounds (28×28). The existing `new_download` style fn (`rounded(40)`) then clamps to radius 14 = 28/2 → perfect circle. No style change needed.

Verified against iced source:
- `iced_widget-0.14.2/src/container.rs:145-165` — `center_x`/`center_y`/`center(impl Into<Length>)` set the container's width+height **and** set `align_x/align_y` to Center.
- `container.rs:407-432` (`layout`) — uses `layout::positioned` with a positioner that calls `content.align(HCenter, VCenter, size)`, i.e. it genuinely centers the child within the container's fixed size (unlike button's identity positioner).
- `container.rs:161-165` — `center(length)` = `center_x(length).center_y(length)`, so one call sets both dimensions.

## Scope
- `src/ui/task_list.rs` only — the `new_btn` block (currently lines 92-105).
- `src/ui/theme.rs` `new_download` style fn: **unchanged**.
- No imports needed: `container` (line 1) and `Length` (line 2) are already in scope; `Length::Fixed` already used (e.g. line 90).
- Position/tooltip/message/i18n: unchanged.

## File change

### `src/ui/task_list.rs` — `new_btn` block
Replace the current block:
```rust
let new_btn: Element<'a, Message> = {
    let glyph = text('\u{E13D}'.to_string()).font(lucide_font).size(15);
    let btn = button(glyph)
        .on_press(Message::OpenAddDialog)
        .padding([6, 6])
        .style(theme::style::button::new_download());
    tooltip(
        btn,
        text(fluent.get(Tr::NewDownload)),
        tooltip::Position::Bottom,
    )
    .style(container::rounded_box)
    .into()
};
```
with:
```rust
let new_btn: Element<'a, Message> = {
    let glyph = text('\u{E13D}'.to_string()).font(lucide_font).size(15);
    let inner = container(glyph).center(Length::Fixed(28.0));
    let btn = button(inner)
        .on_press(Message::OpenAddDialog)
        .padding(0)
        .style(theme::style::button::new_download());
    tooltip(
        btn,
        text(fluent.get(Tr::NewDownload)),
        tooltip::Position::Bottom,
    )
    .style(container::rounded_box)
    .into()
};
```
Net diff:
- Add `let inner = container(glyph).center(Length::Fixed(28.0));` — 28×28 square, glyph centered.
- `button(glyph)` → `button(inner)`.
- `.padding([6, 6])` → `.padding(0)` — button bounds == container bounds == 28×28.

## Why 28
Glyph at size 15 ≈ 15px advance; 28px circle leaves ~6.5px margin around the plus — visually prominent and balanced against the neighboring toolbar buttons (which are ~27 wide × ~30 tall via `padding([6,8])`). Matches the diameter the prior plan's fallback specified.

## Risks / notes
- The outer `toolbar` row has no explicit `align_y`; the New button height changes from ~30 to 28. Negligible — circle reads as same size. Do not add `align_y` (out of scope; keep position/alignment unchanged).
- If the circle still looks slightly off-center, the cause would be the glyph's optical center vs metrics center — not expected for a symmetric `plus` glyph; do not over-correct.
- `rounded(40)` is intentionally oversized so it clamps to half the min dimension; on a 28×28 quad that's 14. Keep it (do not switch to `rounded(14)`) so future size tweaks stay circular automatically.

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` — must stay warning-free aside from the pre-existing unrelated `unused variable: page` in `src/ui/sidebar.rs`.
- `cargo build` — offline build must succeed.
- Manual `cargo run --`: New-Download plus button is a **solid blue circle** (not ellipse) with white plus, centered glyph, in its current right-side position; opens Add dialog on click; tooltip on hover; lightens on hover/press. Verify in both dark and light themes.
