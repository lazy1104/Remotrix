# Add bottom spacer to task list (speed HUD overlap)

## Goal
When the task list is scrolled to the bottom, the floating speed HUD (bottom-right overlay) covers the last task card. Add a blank block at the bottom of the scrollable list content so the last item can always scroll clear of the HUD.

## Context
- The speed HUD is a bottom-right floating overlay: right margin 16px, bottom margin 20px (`src/app.rs:2207-2222`).
- HUD sizes: inactive = 44x44 fixed button; active = PADDING_HUD (8 top/bottom) + two FONT_SMALL rows + SPACE_XS, roughly ~48px tall.
- Task list body is built in `src/ui/task_list.rs:247`:
  ```rust
  let body = slim_scrollable(column![].spacing(SPACE_XL).push(list)).height(Length::Fill);
  ```
- `slim_scrollable` already adds 5px bottom padding (`src/ui/components/slim_scrollable.rs:14`) — insufficient.

## Implementation
In `src/ui/task_list.rs` (line ~247), append a fixed-height spacer to the scrollable content:

```rust
let body = slim_scrollable(
    column![]
        .spacing(SPACE_XL)
        .push(list)
        .push(iced::widget::Space::with_height(Length::Fixed(72.0))),
)
.height(Length::Fill);
```

Spacer height rationale: HUD active height (~48px) + bottom margin (20px) ≈ 68px; 72px adds margin for text line-height rounding and also clears the 44px inactive HUD. `SPACE_XL` list spacing keeps the spacer visually aligned with card gaps.

## Notes / decisions
- Spacer is placed inside the scrollable so it scrolls with content (unlike container padding, which stays pinned to the viewport edge).
- No new dims constant; a single literal matches the codebase's "literal in-place" style for one-off values. (Optional: add `PADDING_HUD_CLEAR`/similar to `dims.rs` if a named constant is preferred.)
- Empty-state branch (`tasks.is_empty()`) is unaffected — spacer only applies to the populated list.
- No changes to HUD, overlay layout, or other pages.

## Validation
1. `cargo clippy --workspace` — no warnings.
2. `cargo fmt --check` — formatting clean.
3. Manual: run app with many tasks, scroll to bottom — last card fully visible above the HUD in both idle (44px) and active (speed display) HUD states.
