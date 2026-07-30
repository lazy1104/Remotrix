# Fix: Tooltip transparency

## Problem

`src/ui/theme.rs:170-178` — `style::tooltip()` ignores the theme (`_t`) and sets only `border.radius`. Both `background` and `text_color` are left as `None` (default), making the tooltip container transparent and its text unreadable against arbitrary backgrounds.

## Root cause

The `tooltip()` style function doesn't apply any background color:

```rust
pub fn tooltip(_t: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        border: iced::Border {
            radius: super::RADIUS_BUTTON.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
```

## Fix

Use `t.extended_palette().background.weak` for both `background` and `text_color`, consistent with the card style used elsewhere in the UI.

```rust
pub fn tooltip(t: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(t.extended_palette().background.weak.color.into()),
        text_color: Some(t.extended_palette().background.weak.text),
        border: iced::Border {
            radius: super::RADIUS_BUTTON.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
```

No other files need changes — `tooltip.rs:15` already calls `.style(theme::style::tooltip)` and will pick up the fix automatically.

## Validation

```bash
cargo clippy --workspace
cargo build
```
