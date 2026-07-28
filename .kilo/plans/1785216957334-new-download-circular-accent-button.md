# Plan: Circular accent background for the New-Download toolbar button

## Goal
Make the New-Download (plus) toolbar button in `src/ui/task_list.rs` visually prominent by giving it a solid circular accent background with a white plus glyph. The button's current position (right side of the toolbar, after the leading `Space::Fill`, immediately before the right action-button group) is kept unchanged — only its styling changes.

## Confirmed decisions
- Visual treatment: **solid `theme::ACCENT` (#4A90D9) filled circle, white (`theme::TEXT_PRIMARY`) plus icon** — works in both dark and light themes (ACCENT is the same blue in both; white icon has good contrast on blue).
- Hover/Pressed: lighten the accent (concrete lighter shade below) to give feedback.
- Shape: perfect circle. Achieved via **symmetric padding `[6, 6]`** (keeps the square lucide glyph centered, since iced's `button::layout` uses `layout::padded` which places content at the padding offset — symmetric padding ⇒ centered) **+ a large `border::rounded(40)`** (the wgpu quad renderer clamps corner radius to half the min dimension, so a ~27px square button renders as a circle).
- Keep the existing tooltip (`Tr::NewDownload`, `tooltip::Position::Bottom`, `container::rounded_box` style) — matches the other toolbar buttons.
- The shared `toolbar_btn` closure returns a `button::text`-styled element and cannot produce the custom circle style, so the New-Download button is built **separately** (same pattern already used for `sort_underlay`), then wrapped in a tooltip.
- `toolbar_btn` stays in use by Refresh / StartAll / PauseAll / DeleteAll / ClearCompleted — no dead code.

## File changes

### 1. `src/ui/theme.rs` — add `new_download` button style
Inside `pub mod button` (alongside `sidebar_icon` / `window_control`), add a new style fn. The module already has `use iced::Color;`; `Color` converts to `Background` via `.into()` (used by `sidebar_icon`), so no new imports are needed.

```rust
pub fn new_download<'a>(
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'a {
    move |_t: &iced::Theme,
          status: iced::widget::button::Status|
          -> iced::widget::button::Style {
        let background = match status {
            iced::widget::button::Status::Hovered
            | iced::widget::button::Status::Pressed => Color::from_rgb(0.353, 0.627, 0.902),
            _ => super::super::ACCENT,
        };
        iced::widget::button::Style {
            background: Some(background.into()),
            text_color: super::super::TEXT_PRIMARY,
            border: iced::border::rounded(40),
            ..Default::default()
        }
    }
}
```
Notes for the implementer:
- `super::super::ACCENT` and `super::super::TEXT_PRIMARY` resolve from `style::button` → `style` → `theme` (same path style `sidebar_icon` already uses for `super::super::ACCENT`).
- `text_color: TEXT_PRIMARY` forces the plus glyph white in both themes (do **not** also set `.color()` on the glyph — let the style drive it, matching `sidebar_icon`).

### 2. `src/ui/task_list.rs` — build the New-Download button separately
- Add a `new_btn` binding (place it near the other standalone button defs, e.g. right after the `sort_dropdown` definition, before the `toolbar` row). It uses the existing `lucide_font`, `button`, `text`, `tooltip`, `container` already in scope:
  ```rust
  let new_btn: Element<'a, Message> = {
      let glyph = text('\u{E13D}'.to_string()).font(lucide_font).size(15);
      let btn = button(glyph)
          .on_press(Message::OpenAddDialog)
          .padding([6, 6])
          .style(theme::style::button::new_download);
      tooltip(btn, text(fluent.get(Tr::NewDownload)), tooltip::Position::Bottom)
          .style(container::rounded_box)
          .into()
  };
  ```
  - `padding([6, 6])` (symmetric) is intentional — see "Shape" decision above. Do **not** use the `[6, 8]` used by `toolbar_btn`.
  - Codepoint `'\u{E13D}'` is the lucide `plus` glyph (same as today).
- In the `toolbar` row, replace the current `.push(toolbar_btn('\u{E13D}', fluent.get(Tr::NewDownload), Message::OpenAddDialog, false))` (currently at `task_list.rs:94-99`) with `.push(new_btn)`. Keep the leading `Space::Fill` and the trailing right-group row exactly as-is — position must not change.

## Risks / notes
- **Squareness assumption**: the perfect-circle result depends on the button being square. Lucide glyphs have advance width = em, so size-15 glyph + `[6,6]` padding ≈ 27×27 (square) ⇒ `rounded(40)` clamps to a circle. If visual inspection shows an ellipse (non-square glyph metrics), fallback: wrap the glyph in `container(glyph).width(Length::Fixed(28.0)).height(Length::Fixed(28.0))` with centered alignment and give the **button** `padding(0)` + the same `rounded(40)` style so the container (not padding) enforces the square. Prefer the simple symmetric-padding approach first.
- The outer `toolbar` row has no explicit `align_y`; the New-Download button already renders acceptably next to the right group today, so vertical alignment is unchanged by this restyle.
- No i18n / message / Cargo changes needed — purely a style + view tweak.

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` — must be warning-free (the `toolbar_btn` closure remains used; `new_btn` is used in the toolbar row).
- `cargo build` — offline build must succeed.
- Manual run (`cargo run --`): the New-Download plus button shows as a solid blue circle with a white plus, sits in its current right-side position, opens the Add dialog on click, shows its tooltip on hover, and lightens on hover/press. Verify it looks correct in both dark and light themes.
