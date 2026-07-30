# Plan: Scrollbar Style Fix & Right Padding

## Problem
1. **Right-side masking**: Scrollbar overlays content on right (settings_page)
2. **Scrollbar appearance**: Default scrollbar is too wide, no rounded corners

## Solution: Reusable `slim_scrollable` component

Extract the scrollbar style + config into a single helper component so callers don't repeat the same config.

### Files created

#### `src/ui/components/slim_scrollable.rs` — New component
```rust
// Exports a single function:
pub fn slim_scrollable<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> iced::widget::Scrollable<'a, Message, iced::Theme>
```
- Direction: `Vertical` with `Scrollbar::new().width(6.0).scroller_width(6.0).margin(4.0).anchor(Anchor::End)`
- Style: calls `theme::style::scrollable::standard()` style closure

### Files modified

#### `src/ui/components/mod.rs`
- Add `pub mod slim_scrollable;`

#### `src/ui/theme.rs` — Add scrollable style module
Add `pub mod scrollable` inside `pub mod style` with a `standard()` function.
- `vertical_rail.background`: `None` (transparent track)
- `vertical_rail.border`: default
- `vertical_rail.scroller.background`: palette primary base color
- `vertical_rail.scroller.border.radius`: `super::super::RADIUS_BUTTON` (rounded 6px)
- Same for `horizontal_rail`
- `gap`: `None`
- `auto_scroll`: palette background base color

#### `src/ui/settings_page.rs` — Use slim_scrollable + right padding
```
- scrollable(col).height(Length::Fill)
+ slim_scrollable(col).height(Length::Fill)
```
- Also increase container right padding: `[24, 28]` → `[24, 36]`

#### `src/ui/task_list.rs` — Use slim_scrollable
```
- scrollable(column![].spacing(10).push(list)).height(Length::Fill)
+ slim_scrollable(column![].spacing(10).push(list)).height(Length::Fill)
```

#### `src/ui/details_dialog.rs` — Use slim_scrollable
```
- scrollable(column![].push(col).spacing(6)).height(Length::Fill).into()
+ slim_scrollable(column![].push(col).spacing(6)).height(Length::Fill).into()
```

### Files not changed
- `path_picker.rs` — already uses `Scrollbar::hidden()`, keep as-is

## Imports needed
- `theme.rs`: `use iced::widget::scrollable::{self, Rail, Scroller}`; `use iced::Border`
- Callers: remove `scrollable` from iced::widget import (replaced by `slim_scrollable`)
