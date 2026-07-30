# Plan: PathPicker History Dropdown Scrollable

## Problem
The `PathPicker` history dropdown shows all items in a plain `column`. When many history entries exist, the overlay overflows/clips because content exceeds the DropDown's viewport-constrained height.

## Root Cause
File: `src/ui/components/path_picker.rs:226-242`

The overlay is a `container(column(...))` with no scrollable. The `iced_aw::DropDown` layout caps overlay height via `limits.max_height(height_below)` (viewport space below the underlay), but when content exceeds this cap, it's just clipped.

## Solution

### 1. `src/ui/components/path_picker.rs`

**Add import**: `scrollable` to the `iced::widget` import line.

**Change (around line 226-242)**: Wrap the history items `column` in a `scrollable` with a hidden scrollbar (`Scrollbar::hidden()` sets width=0, scroller_width=0 → invisible but scrollable via mouse wheel).

Current:
```rust
let overlay = container(column(overlay_items).spacing(2).width(Length::Fill))
    .padding(6)
    .style(theme::style::card);
```

New:
```rust
let overlay = container(
    scrollable(column(overlay_items).spacing(2).width(Length::Fill))
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::hidden(),
        )),
)
.padding(6)
.style(theme::style::card);
```

### Why this works
- `scrollable` default height = `Length::Shrink` → scrollable reports `min(content_height, max_height)` where `max_height` is the DropDown's viewport cap
- Small content → scrollable is small (no extra space)
- Large content → scrollable is capped at viewport space, scrolls internally
- `Scrollbar::hidden()` → no visible scrollbar, mouse wheel still works
- No changes needed to `DropDown` height configuration

## Risks
- If `Scrollbar::hidden()` is not re-exported through `iced::widget::scrollable`, may need to import from `iced::widget::scrollable::Scrollbar` directly. Verify compile.
- `Direction` enum path: `iced::widget::scrollable::Direction`
- `Scrollbar` struct path: `iced::widget::scrollable::Scrollbar`
- These are all `pub` in iced_widget 0.14.2, re-exported through iced 0.14.

## Validation
```bash
cargo clippy --workspace
```
