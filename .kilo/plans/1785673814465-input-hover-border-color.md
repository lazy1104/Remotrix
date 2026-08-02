# Make input hover border color match active

## Goal
Make the hover border color identical to the active (idle) border color for:
1. The iced native `text_input` (via `theme::style::input::standard`).
2. The custom `path_picker` and `number_stepper` widgets (shared frame style `theme::style::grouped_frame_state`).

## Background (verified)
- iced default style (`iced_widget-0.14.2/src/text_input.rs:1758-1797`):
  - `Active`: border = `palette.background.strong.color`
  - `Hovered`: border = `palette.background.base.text` (only the border color differs from Active)
  - `Focused`: border = `palette.primary.strong.color`
- Project helpers in `src/ui/theme.rs`:
  - `border_color(t)` = `t.extended_palette().background.strong.color` (theme.rs:226) — same color as native Active border.
  - `text_secondary(t)` = `t.extended_palette().background.base.text` (theme.rs:222) — same color as native Hovered border.
- `grouped_frame_state(focused, hovered)` (theme.rs:286-305) currently uses `text_secondary` on hover; call sites: `path_picker.rs:226`, `number_stepper.rs:367`.
- All native text inputs in the app use `input::standard` (task_list.rs:152, add_dialog.rs:326/502, settings_page.rs:990); grouped components use `input::grouped` (no border, frame provides it).
- Baseline `cargo clippy --workspace` is clean.

## Changes

### 1. `src/ui/theme.rs` — `style::input::standard` (lines 818-822)
Keep the default style and radius override, but force the hover border to the active border color:

```rust
pub fn standard(t: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let mut s = text_input::default(t, status);
    s.border.radius = super::super::RADIUS_BUTTON.into();
    if matches!(status, text_input::Status::Hovered) {
        s.border.color = t.extended_palette().background.strong.color;
    }
    s
}
```

### 2. `src/ui/theme.rs` — `grouped_frame_state` (lines 286-305)
Hover must render the same border color as the idle/active state. Since hovered == idle, collapse to a two-way branch; keep the signature stable (call sites pass `(focused, hovered)`) by renaming the now-unused param to `_hovered` (avoids `unused_variables`):

```rust
pub fn grouped_frame_state(
    focused: bool,
    _hovered: bool,
) -> impl Fn(&iced::Theme) -> iced::widget::container::Style {
    move |t| iced::widget::container::Style {
        background: Some(t.extended_palette().background.base.color.into()),
        border: iced::Border {
            color: if focused {
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

No changes needed in `path_picker.rs` / `number_stepper.rs` — they keep passing `hovered` and the focused → primary border behavior is preserved.

## Out of scope
- `text_editor::standard` / `pick_list::standard` (also delegate to iced `default`) — not requested; can be revisited later.
- Removing the now inert `hovered` field/tracking in `path_picker.rs` and `number_stepper.rs` — left untouched to minimize churn and avoid changing mouse-area behavior (`Exited` also clears `focused` in `PathPicker::update`).

## Validation
1. `cargo clippy --workspace` — no warnings.
2. `cargo fmt --check`.
3. `cargo build`.
4. Manual: hover a search/settings text input → border stays the idle gray (`background.strong`), no brightening; hover `path_picker` / `number_stepper` → frame border stays `border_color`; focusing an input / stepping component still shows the primary border.
