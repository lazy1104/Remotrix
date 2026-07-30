# Dropdown Item Hover Style

## Problem
History path dropdown items in `path_picker.rs` use `button::text()` style which gives only a subtle white tint on hover (8% white). The user wants hover to look like a typical `<select>` dropdown — highlighted with the accent/primary color.

## Changes

### 1. `src/ui/theme.rs` — Add `button::picker_item()` style
Insert after `button::text()` (line 254) and before `button::toolbar_icon()` (line 256):

```rust
pub fn picker_item<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
    move |t: &iced::Theme, status: Status| -> Style {
        let accent = t.extended_palette().primary.base.color;
        let base_text = t.extended_palette().background.base.text;
        Style {
            background: match status {
                Status::Hovered => {
                    Some(Color::from_rgba(accent.r, accent.g, accent.b, 0.12).into())
                }
                Status::Pressed => {
                    Some(Color::from_rgba(accent.r, accent.g, accent.b, 0.20).into())
                }
                _ => None,
            },
            text_color: match status {
                Status::Disabled => scale_alpha(base_text, 0.5),
                _ => base_text,
            },
            border: iced::border::rounded(super::super::RADIUS_BUTTON),
            shadow: Shadow::default(),
            ..Default::default()
        }
    }
}
```

### 2. `src/ui/components/path_picker.rs` — Use new style
Line 241: Change `.style(theme::style::button::text())` to `.style(theme::style::button::picker_item())`

## Behavior
| Status | Background |
|---|---|
| Normal | transparent |
| Hovered | accent color @ 12% opacity |
| Pressed | accent color @ 20% opacity |

Matches typical dropdown select highlight behavior.
