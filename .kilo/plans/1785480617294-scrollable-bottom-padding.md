# Plan: Add 5px bottom padding to `slim_scrollable` content

## Problem
Bottom components inside `slim_scrollable` pages (task list, settings, details dialog, add dialog) have their borders clipped/overlapping with the scrollable's bottom edge. The user wants 5px of bottom spacing so the last row is not cut off.

## Feasibility
`iced::widget::Scrollable` (iced 0.14 / `iced_widget-0.14.2/src/scrollable.rs`) has **no `padding` method**, so we cannot set padding directly on the Scrollable builder. However, we can add the padding inside the component by wrapping the content in a `iced::widget::container` with bottom padding. This fixes all 4 call sites with one consistent change (preferred over editing each usage site).

## Change
File: `src/ui/components/slim_scrollable.rs`

Update imports and wrap content in a full-width container with 5px bottom padding:

```rust
use iced::widget::container;
use iced::widget::scrollable::{self, Scrollbar};
use iced::{Element, Length};

use crate::ui::theme;

pub fn slim_scrollable<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> iced::widget::Scrollable<'a, Message> {
    iced::widget::scrollable(
        container(content).width(Length::Fill).padding([0, 0, 5, 0]),
    )
    .direction(scrollable::Direction::Vertical(
        Scrollbar::new().width(6.0).scroller_width(6.0),
    ))
    .spacing(3)
    .style(theme::style::scrollable::standard)
}
```

Notes:
- `[0, 0, 5, 0]` is `[top, right, bottom, left]` → only bottom gets 5px.
- `width(Length::Fill)` preserves the full-width content layout that all 4 callers rely on.
- `container` default style is transparent (no background/border), so no visual side effects.
- Callers keep setting `.height(...)` on the returned `Scrollable` — unaffected.

## Validation
- `cargo clippy --workspace` — no warnings.
- `cargo fmt --check`.
- Manual: open task list, settings, details dialog, and add dialog; verify the last row is no longer clipped at the scroll bottom.
