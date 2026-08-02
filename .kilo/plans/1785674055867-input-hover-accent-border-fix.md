# Fix: input hover border should be primary accent (preview of focus)

## Problem
The previous change (`1785673814465-input-hover-border-color.md`) set the hover border to the
**idle** gray (`background.strong`) on both `input::standard` and `grouped_frame_state`. Because the
idle and hovered borders became identical, hovering gives **no visual feedback** at all — the user
reported "现在直接没有hover效果了" (no hover effect anymore).

Root cause: "match active" was interpreted as the idle `Active` state instead of the focused/active
(accent) state. The intended behavior is: hovering previews the focused border.

## Decision (user-confirmed)
Hover border becomes the primary accent color, **identical to that widget's focused border**:

- `input::standard`: hover border = `primary.strong.color` (iced default Focused border for text_input
  is `palette.primary.strong.color` — iced_widget-0.14.2/src/text_input.rs:1785).
- `grouped_frame_state`: hover border = `primary.base.color` (its existing focused border color,
  theme.rs:294).

Focused state itself is untouched. Idle border stays `background.strong.color` (`border_color`).

## Changes

### 1. `src/ui/theme.rs` — `style::input::standard` (lines 818-823)
Replace the current `background.strong` override with the accent color:

```rust
pub fn standard(t: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let mut s = text_input::default(t, status);
    s.border.radius = super::super::RADIUS_BUTTON.into();
    if matches!(status, text_input::Status::Hovered) {
        s.border.color = t.extended_palette().primary.strong.color;
    }
    s
}
```

### 2. `src/ui/theme.rs` — `grouped_frame_state` (lines 286-305)
Restore the `hovered` param name and fold hover into the focused branch (both render `primary.base`):

```rust
pub fn grouped_frame_state(
    focused: bool,
    hovered: bool,
) -> impl Fn(&iced::Theme) -> iced::widget::container::Style {
    move |t| iced::widget::container::Style {
        background: Some(t.extended_palette().background.base.color.into()),
        border: iced::Border {
            color: if focused || hovered {
                t.extended_palette().primary.base.color
            } else {
                super::border_color(t)
            },
            width: 1.0,
            radius: super::RADIUS_BUTTON.into(),
        },
        ..Default::default()
    }
}
```

## No other changes needed
- Call sites `path_picker.rs:226` and `number_stepper.rs:367` pass `(focused, hovered)` — signature
  unchanged, so both keep working. Hover tracking mechanisms are untouched.
- `text_editor::standard` / `pick_list::standard` remain out of scope (same pattern could be applied
  later if requested).
- `Focused { is_hovered: true }` needs no special case: iced default already renders
  `primary.strong` for any `Focused` variant.

## Validation
1. `cargo clippy --workspace` — no warnings.
2. `cargo fmt --check`.
3. `cargo build`.
4. Manual:
   - Hover a search/settings text input → border turns primary accent (same as when focused).
   - Hover `path_picker` / `number_stepper` frame → border turns primary accent.
   - Focus the widgets → border remains primary accent (no visible change from hover, by design).
   - Idle state → gray `border_color`, unchanged.
