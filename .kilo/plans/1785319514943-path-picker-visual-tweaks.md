# Path-Picker Visual Tweaks

## Goal
Refine the existing `src/ui/path_picker.rs` component and its `theme::style` helpers based on 4 user feedback points:
1. Remove the leading folder icon from the component.
2. Give the icon buttons a distinct background color (different from the input).
3. Add border separators between icon buttons (keep flush, no gaps).
4. Make buttons/input fill the 36px container height (no undersized buttons).

## Files to edit
- `src/ui/path_picker.rs` (component)
- `src/ui/theme.rs` (styles)
- `src/ui/settings_page.rs` (caller: remove `show_path_icon` arg)
- `src/ui/add_dialog.rs` (caller: remove `show_path_icon` arg ×2)

## 1. Remove leading icon (`path_picker.rs`)

- Remove the `show_path_icon: bool` parameter from `view()` signature (now 7 params → drop `#[allow(clippy::too_many_arguments)]`).
- Remove the `if show_path_icon { ... }` block (the `container(icon::folder_open()...)` with `iced::Padding` struct).
- Remove the now-unused `iced::Padding` usage and `icon::folder_open` leading-icon call (the browse button still uses `icon::folder_open` — keep that).
- Update all 3 call sites to drop the 5th arg (`true`):
  - `settings_page.rs:212` — `path_picker::view(fluent, theme, dir_str, Some(...), true, true, open, hist)` → remove the first `true`
  - `add_dialog.rs:85` (torrent) — same
  - `add_dialog.rs:108` (save) — same

## 2. Button background = `background.weak.color` (`theme.rs`)

Modify `button::grouped_icon(trailing)`:
- Default state: `Some(t.extended_palette().background.weak.color.into())` (was `None`).
- Hovered: `Some(super::lighten(weak_bg, 0.08).into())` (blends toward white 8%, was flat white overlay).
- Pressed: `Some(super::lighten(weak_bg, 0.14).into())` (was flat white overlay).
- Compute `let weak_bg = t.extended_palette().background.weak.color;` once at the top.
- `super::lighten` is already accessible from the `button` submodule (same pattern as `super::darken` used in `filled`).
- Input stays transparent (shows `background.base.color`), buttons show `background.weak.color` → visually distinct.

## 3. Border separators between buttons (`path_picker.rs` + `theme.rs`)

**Constraint:** iced's `Border.width` is uniform (no per-side widths), so a left-only border on a button isn't possible. Use 1px separator widgets between buttons instead.

Add a new theme style in `theme.rs` `style` module:
```rust
pub fn separator(t: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(super::border_color(t).into()),
        ..Default::default()
    }
}
```

In `path_picker.rs`, add a helper:
```rust
fn separator() -> Element<'a, Message> {
    container(iced::widget::Space::new())
        .width(1.0)
        .height(Length::Fill)
        .style(theme::style::separator)
        .into()
}
```

Insert separators **between** buttons only (not between input and first button — the bg color difference already separates them):
- `row.push(copy_btn).push(separator()).push(browse_btn)`
- If history button present: `.push(separator()).push(history_btn)`

Read-only mode (id == None, copy only): no separator (single button).

## 4. Buttons fill 36px height (`path_picker.rs`)

- Set the row height to Fill so it fills the container: `row![].spacing(0).align_y(Alignment::Center).height(Length::Fill)`.
- Set each button's height to Fill: `.height(Length::Fill)` on copy, browse, and history buttons.
- `text_input` has no `.height()` method (verified in iced_widget 0.14.2) — leave it at natural height; `align_y(Center)` centers it within the 36px row.
- Separators already use `.height(Length::Fill)`.
- Container stays `.height(Length::Fixed(36.0))`.

## Full `grouped_icon` style after changes

```rust
pub fn grouped_icon<'a>(trailing: bool) -> impl Fn(&iced::Theme, Status) -> Style + 'a {
    move |t, status| {
        let base_text = t.extended_palette().background.base.text;
        let weak_bg = t.extended_palette().background.weak.color;
        let radius = if trailing {
            iced::border::Radius::default().right(super::super::RADIUS_BUTTON)
        } else {
            iced::border::Radius::default()
        };
        Style {
            background: match status {
                Status::Hovered => Some(super::lighten(weak_bg, 0.08).into()),
                Status::Pressed => Some(super::lighten(weak_bg, 0.14).into()),
                _ => Some(weak_bg.into()),
            },
            text_color: base_text,
            border: iced::Border {
                color: iced::Color::TRANSPARENT,
                width: 0.0,
                radius,
            },
            shadow: Shadow::default(),
            ..Default::default()
        }
    }
}
```

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` (no warnings — verify `too_many_arguments` allow can be removed)
- `cargo build`
- Visual checks: no leading icon; buttons have weak.color bg distinct from input; 1px dividers between copy/browse/history; all elements fill 36px height; hover lightens button bg.
