# Secondary button restyle + add_dialog disabled state

## Goal
1. **add_dialog**: when there is no content (`!can_submit()`), the Download button must be a *real* disabled button (no `on_press`, rendered at `Status::Disabled`), not merely a color swap to `secondary()`.
2. **Global**: restyle `button::secondary()` so it no longer renders as a saturated solid fill (current `filled(secondary palette)`), which competes with `primary()` and looks jarring ("突兀"). Make it a subtle **outline/tinted** button consistent with the existing `text()` button but with a visible 1px border so it still reads as a button.

## Root cause
- `src/ui/theme.rs:364` `button::secondary()` delegates to `filled(p.base.color, p.strong.color, p.base.text, status)` using `t.extended_palette().secondary` — a saturated fill with a drop shadow, visually as heavy as `primary()`.
- `src/ui/add_dialog.rs:121-130` always attaches `.on_press(Message::AddDownload)` and only swaps `.style()` between `primary()` and `secondary()`. A button with `on_press` is never in `Status::Disabled`, so the disabled visual is faked via style swap rather than a true disabled state.

## Design decisions

### New `button::secondary()` style (src/ui/theme.rs:364-369)
Replace the `filled(...)` body with a self-contained outline/tinted style (do **not** change `primary()`/`danger()` which still use `filled`):

| Status | background | border | text_color | shadow |
|---|---|---|---|---|
| Active (normal) | `None` (transparent) | `1.0`px `border_color(t)` = `background.strong.color`, radius `RADIUS_BUTTON` | `background.base.text` | `Shadow::default()` |
| Hovered | `Color::from_rgba(1,1,1,0.08)` | same border | same | none |
| Pressed | `Color::from_rgba(1,1,1,0.14)` | same border | same | none |
| Disabled | `None` | `scale_alpha(border_color, 0.5)` | `scale_alpha(background.base.text, 0.5)` | none |

Use the existing helpers `super::super::border_color(t)` (theme.rs:92) and `scale_alpha` (theme.rs:237). The hover/pressed tint values `0.08`/`0.14` match the existing `text()` button (theme.rs:249-250) for consistency.

This makes `secondary()` a quiet outlined button; `primary()`/`danger()` remain the only saturated fills, restoring clear visual hierarchy (primary = action, danger = destructive, secondary = dismiss/secondary).

### add_dialog Download button (src/ui/add_dialog.rs:121-131)
Replace the current always-`on_press` + style-swap block with:
- Always apply `.style(theme::style::button::primary())` and `.padding([8, 18])`.
- Attach `.on_press(Message::AddDownload)` **only when** `state.can_submit()`.
- When `!can_submit()`, omit `on_press` entirely → iced renders the button in `Status::Disabled`, and `primary()`'s `filled` helper already dims to 0.5 alpha (theme.rs:334-350). No `secondary()` swap.
- Remove the now-unused `if state.can_submit() { ... } else { ... }` style branch.

The Cancel button (add_dialog.rs:116-119) keeps `button::secondary()` and picks up the new outline style automatically.

## Affected call sites (auto-pickup, no per-file edits)
All `button::secondary()` usages inherit the new style:
- src/ui/add_dialog.rs:119 (Cancel)
- src/ui/confirm_dialog.rs:30, 37, 57
- src/ui/close_dialog.rs:26
- src/ui/about_dialog.rs:45
- src/ui/settings_page.rs:624, 631
- src/ui/details_dialog.rs:106

No changes needed at these sites; only `src/ui/theme.rs` and `src/ui/add_dialog.rs` are edited.

## Out of scope
- `button::text()`, `primary()`, `danger()`, `grouped_icon()`, `sidebar_*`, `toolbar_icon`, `window_control`, `picker_item` styles — unchanged.
- The unused `button::new_download()` pill style (theme.rs:481) — not touched.
- `text::secondary` text style (theme.rs:632) — unrelated, unchanged.

## Risks / edge cases
- Light theme: the `rgba(1,1,1,...)` hover tint assumes a dark UI background. Confirm against opaline light themes — if a light theme is active, white tint on light bg is invisible. **Mitigation**: verify with at least one light builtin theme; if needed, derive hover tint from `background.base.text` at low alpha instead of hardcoded white. (Check during validation; adjust only if a light theme is actually selectable in-app.)
- Outline border uses `background.strong.color` (same as card/separator border) — consistent with existing border usage (`grouped_frame_state` unfocused, theme.rs:152).
- Disabled Download button still occupies space and shows the label at 0.5 alpha — intended.

## Validation
1. `cargo fmt --check`
2. `cargo clippy --workspace` (no warnings)
3. `cargo build`
4. Run app, open New Download dialog:
   - Empty inputs → Download button dimmed (0.5 alpha), not clickable; Cancel is a quiet outlined button.
   - Type a URL + pick save dir → Download button becomes full primary color, clickable.
   - Torrent path only (no URL) → Download enabled (matches `can_submit()`).
5. Open a confirm dialog (e.g. delete task) and Settings → verify Cancel/secondary buttons now appear as subtle outlined buttons, no longer saturated fills competing with the primary/danger action.

## Open questions
None — design confirmed with user ("整体secondary按钮都改").
