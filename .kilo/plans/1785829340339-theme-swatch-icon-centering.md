# Fix: theme color swatch selected icon not centered

## Problem
In `src/ui/settings_page.rs`, `theme_color_swatches` (lines ~290-319) renders each
color swatch as a `button` with fixed `SWATCH_SIZE` (28px) width/height and `padding(0)`,
whose content is either `icon::circle_check().size(FONT_ICON)` (selected) or empty
`text("")`.

Iced 0.14's `Button` does **not** center its content: `button.rs` `layout` calls
`layout::padded` -> `positioned` (`iced_core-0.14.0/src/layout.rs`), which places the
content at `(padding.left, padding.top)` — the top-left corner. Hence the selected
check mark renders in the top-left of the circle instead of the center.

## Root cause
- `iced_widget-0.14.2/src/button.rs` `layout()` uses `layout::padded(..., |limits| content.layout(...))`.
- `iced_core-0.14.0/src/layout.rs` `positioned()`: `content.move_to((padding.left, padding.top))` with no centering.

## Fix
In `theme_color_swatches`, wrap the swatch marker in a full-size centering
`container` so the icon is centered within the 28×28 button.

Change in `src/ui/settings_page.rs` `theme_color_swatches`:
```rust
let swatch = button(
    container(if selected {
        icon::circle_check().size(FONT_ICON)
    } else {
        text("").size(FONT_ICON)
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill),
)
.on_press(Message::Settings(SettingsMsg::ThemeColorChanged(*color)))
.width(Length::Fixed(SWATCH_SIZE))
.height(Length::Fixed(SWATCH_SIZE))
.padding(0)
.style(theme::style::button::swatch(*color, selected));
```

Notes:
- `container`, `Length`, `icon`, `text` are already imported in this file.
- `center_x`/`center_y` exist on `iced_widget::container` (confirmed at `container.rs:145/150`).
- The empty `text("")` (unselected) case also gets wrapped — harmless (empty content centered is a no-op).

## Validation
- `cargo build` (or `cargo run --`) and visually confirm the check mark is centered
  in the selected swatch.
- `cargo clippy --workspace` (no new warnings).
- `cargo fmt --check`.
