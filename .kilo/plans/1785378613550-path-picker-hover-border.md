# Plan: PathPicker hover border — fix hover color to match input box

## Status
The hover feature is **already structurally implemented** in the working tree:
- `PathPickerEvent::Entered` variant, `hovered: bool` field (init `false` in all 3 ctors), `on_enter`/`on_exit` on the `mouse_area` wrap, and `grouped_frame_state(focused, hovered)` signature all exist.
- `update` already sets `hovered=true` on `Entered` and clears both `hovered`+`focused` on `Exited`.

Only **one defect remains**: the border color in `grouped_frame_state` collapses hover into the focus color (accent). Fix it to a 3-tier scheme so hover matches a standard `iced::widget::text_input`.

## Color reference (verified — iced_widget-0.14.2/src/text_input.rs:1761-1789)
Standard iced `text_input::default` borders by `Status`:
- **Active** (idle): `palette.background.strong.color` (= project `border_color`, `theme.rs:83`)
- **Hovered**: `palette.background.base.text` (= project `text_secondary`, `theme.rs:79`)
- **Focused**: `palette.primary.strong.color`

Project precedent: existing focus flash uses `primary.base.color` (`accent`, `theme.rs:63`). **Keep focus on `accent`** (`primary.base.color`) — user only flagged hover as wrong; do not change the press-flash color.

## Goal
3-state border, **focus takes precedence over hover** (a press is a stronger signal than a hover):
| state            | border color                       | source                              |
|------------------|------------------------------------|-------------------------------------|
| `focused`        | `primary.base.color` (accent)      | `t.extended_palette().primary.base.color` |
| `hovered` only   | `background.base.text`             | `super::text_secondary(t)`          |
| idle             | `background.strong.color`          | `super::border_color(t)`            |

When both flags are true → accent (focus wins). Reads: idle → neutral hover → accent press → (leave) idle.

## Implementation — exactly ONE file changes

### `src/ui/theme.rs` (`style::grouped_frame_state`, currently lines 131-147)
Replace the border `color` expression.

Current (broken):
```rust
border: iced::Border {
    color: if focused || hovered {
        t.extended_palette().primary.base.color
    } else {
        super::border_color(t)
    },
    width: 1.0,
    radius: super::RADIUS_BUTTON.into(),
},
```

New:
```rust
border: iced::Border {
    color: if focused {
        t.extended_palette().primary.base.color
    } else if hovered {
        super::text_secondary(t)
    } else {
        super::border_color(t)
    },
    width: 1.0,
    radius: super::RADIUS_BUTTON.into(),
},
```
- Signature `grouped_frame_state(focused: bool, hovered: bool)` stays the same (already updated in `path_picker.rs` call site).
- Background (`background.base.color`), width `1.0`, radius `super::RADIUS_BUTTON` — unchanged.
- `super::text_secondary(t)` is the correct path: `text_secondary` lives at the `theme` module level (`theme.rs:79`), and `style` is its direct child module, so `super::` resolves there. (Compare existing `super::border_color(t)` at line ~135 inside the same fn — same depth.)

### No other files change
- `src/ui/components/path_picker.rs` — **already correct**, do NOT touch (Entered/hovered/on_enter all present).
- `app.rs` / `message.rs` / `settings_page.rs` / `add_dialog.rs` — none needed.

## Edge cases (accepted, no extra code)
- Hover then press Copy/Browse: hovered=true → neutral border; press sets focused=true → accent (focus wins); leave → on_exit clears both → idle.
- Dropdown overlay: cursor leaves group bounds → on_exit reverts to idle briefly (accepted flicker); selecting item sets focused=true → accent; move out → idle.
- Browse opens rfd: if pointer stays in window, border stays accent (focused) until on_exit; accepted.
- Read-only pickers: never wrapped by `mouse_area`; `hovered`/`focused` stay `false` → idle border throughout. ✓

## Validation
- `cargo build`; `cargo clippy --workspace`; `cargo fmt --check`.
- Manual:
  - Hover a folder picker → border = neutral text color (`background.base.text`); move out → reverts to idle.
  - While hovering, press Copy → border switches to accent; move out → idle.
  - Hover then press Browse → accent during rfd; close dialog + mouse-out → idle.
  - Hover then click folder_clock → dropdown opens + neutral→accent on click; select item → accent; move out → idle.
  - Read-only engine paths (Advanced) → border stays idle (expected — no mouse_area wrap).