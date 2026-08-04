# Plan: Center toast cards within their column

## Goal
When a short toast (e.g. "已复制") stacks under a long toast in the same position column, the short card renders left-aligned instead of centered. Make each toast card horizontally centered within the column.

## Background (already implemented)
- Toast cards adapt to content width via `Length::Shrink` + `container::max_width(CARD_MAX_WIDTH)` in `src/ui/components/toast.rs:12,170-171`.
- The column holding the cards is `column![].spacing(SPACE_LG)` (toast.rs:123) with default width `Shrink`, so its width equals the widest card. The wrapping container is `width(Fill).height(Fill)` with `align_x(h)`/`align_y(v)` (toast.rs:127-134).

## Root cause (verified)
- `iced::widget::Column` lays out children with `layout::flex::resolve(..., self.align, ...)` (iced_widget-0.14.2/src/column.rs:218-247). `self.align` is the cross-axis (horizontal) alignment.
- Default `align` is `Alignment::Start` (column.rs:86), i.e. each card is left-aligned within the column width.
- Because the column width equals the widest card, a shorter card is placed at the left edge of that width → appears left of center when a long toast is present.

## Implementation
Single edit in `src/ui/components/toast.rs` `view()` (line ~123): set the column's cross-axis alignment to center.

```rust
let mut column_ = column![].spacing(SPACE_LG).align_x(Horizontal::Center);
```

`Horizontal` is already imported (`use iced::alignment::{Horizontal, Vertical};`, line 3). No new imports.

## Notes / Edge cases
- With `align_x(Horizontal::Center)`, each card is centered on the same horizontal axis as the widest card in the column, regardless of message length.
- Cards of equal width are unaffected (they already fill the column).
- The column is still centered as a whole by the outer container; `align_x` only recenters children within the column width.
- No toast-manager logic depends on card alignment; change is isolated to the column widget.

## Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings allowed)
- `cargo fmt --check`
- Manual: trigger a long toast (e.g. a long engine error) and then a short copy toast ("已复制") in the same position; confirm the short toast is centered relative to the long toast, not left-aligned. Also confirm a single short toast is still centered on screen.

## Out of scope
- Toast stack/dedup logic, message content, adaptive-width behavior (already done), and other copy surfaces.