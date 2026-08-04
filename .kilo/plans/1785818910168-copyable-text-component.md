# Plan: Refine "Copyable Text" Component (revert details_dialog + number_stepper styling)

## Goal
Adjust the previously-added copyable-text component per user feedback:
1. **Do NOT delete the component** — keep it as a reusable, generic component.
2. **Restore `src/ui/details_dialog.rs` to its original pre-component state** (remove the wiring there).
3. **Align the component's background & border with the number stepper** (`src/ui/components/number_stepper.rs` → `theme::style::grouped_frame_state`).

## Decisions
- The component stays a generic `copyable_text(text, on_copy)` builder (`src/ui/components/copyable_text.rs`), unchanged in its widget structure.
- Styling lives entirely in the `theme::style::button::copyable()` style; updating it to mirror `grouped_frame_state` (theme.rs:328) changes the visual without touching the widget.
- After restoring details_dialog, the component becomes unused. The project requires a warning-free build, so mark the reusable items `#[allow(dead_code)]` until they get a real usage site.
- The `Message::CopyText(String)` variant and its app.rs handler were only added for the details_dialog usage; they are now dead and must be removed.

## Tasks

### 1. Revert `src/ui/details_dialog.rs`
Restore to original committed state:
- Remove import on line 11: `use crate::ui::components::copyable_text::copyable_text;`
- Restore `key_value_row(key: String, value: String)` to accept a `String` value and render it inline with `truncated_text(value).size(FONT_MEDIUM).max_lines(2).wrapping(text::Wrapping::Glyph)` (drop the `Element<'static, Message>` signature).
- Delete the `text_value(value: String)` helper (lines 171–177).
- In `summary_tab`, restore the GID row to `key_value_row(fluent.get(Tr::FieldGid), gid_val.to_string()),` and the other rows to the `key_value_row(key, <string>)` form.
- Remove the `let gid = gid_val.clone();` local.

### 2. Remove dead message plumbing
- `src/message.rs`: delete `CopyText(String),` (line 70).
- `src/app.rs`: delete the arm `Message::CopyText(s) => return iced::clipboard::write::<Message>(s),` (line 2686).

### 3. Align component style with number_stepper
In `src/ui/theme.rs`, update `theme::style::button::copyable()` to mirror `grouped_frame_state` (theme.rs:328):
- background: `Some(p.background.base.color.into())` (was `p.background.weak`)
- border color: `p.primary.base.color` on `Status::Hovered | Status::Pressed`, else `super::super::border_color(t)` (was `Color::TRANSPARENT` / `p.primary.weak`)
- keep `width: 1.0`, `radius: iced::border::rounded(super::super::RADIUS_BUTTON).radius`

Net visual: subtle `border_color` border at rest → primary highlight on hover, surface-base background — identical to the number stepper's grouped frame.

### 4. Keep the build warning-free
- Add `#[allow(dead_code)]` at the top of `src/ui/components/copyable_text.rs` (covers `copyable_text`, `CopyableText`, and the `From` impl).
- Add `#[allow(dead_code)]` to `theme::style::button::copyable()` in `src/ui/theme.rs`.
- `src/ui/components/mod.rs` keeps `pub mod copyable_text;` (component stays registered for future use).

## Validation
- `cargo build` — clean, no warnings.
- `cargo clippy --workspace` — no warnings.
- `cargo fmt --check` — clean.

## Open question (non-blocking)
- No usage site is specified yet, so the component ships as an `#[allow(dead_code)]` reusable building block. If you have a target location in mind, provide it and we can wire it there instead of suppressing the lint.
