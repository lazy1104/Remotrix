# Move piece_map & speed_hud into ui/components

## Goal
Relocate `src/ui/piece_map.rs` and `src/ui/speed_hud.rs` into the existing `src/ui/components/` module, updating module declarations and call sites accordingly.

## Context
- `src/ui/components/` already exists with `mod.rs` declaring submodules (`dialog`, `toast`, etc.).
- `src/ui/mod.rs` currently declares `pub mod piece_map;` (line 10) and `pub mod speed_hud;` (line 15).
- Call sites:
  - `src/app.rs:1218` — `crate::ui::speed_hud::view(...)`
  - `src/ui/details_dialog.rs:207` — `crate::ui::piece_map::view(...)`
- `speed_hud.rs` references `crate::ui::theme` and `crate::ui::icon` — these absolute `crate::ui::` paths remain valid after the move (no need to switch to `super::`). Note: there is a local variable named `theme` in `speed_hud.rs` colliding with the module path `theme::style`; the current code uses `theme::style::...` which resolves to the parameter `theme` (a `&iced::Theme`), not the module. This already compiles, so keep behavior unchanged. Consider renaming the local for clarity but not required.
- The `speed_hud_background` style in `theme.rs` is named by feature, not module — leave as-is.

## Steps
1. Move files:
   - `src/ui/piece_map.rs` → `src/ui/components/piece_map.rs`
   - `src/ui/speed_hud.rs` → `src/ui/components/speed_hud.rs`
   Use `git mv` to preserve history.

2. Update `src/ui/components/mod.rs`: append
   ```rust
   pub mod piece_map;
   pub mod speed_hud;
   ```

3. Update `src/ui/mod.rs`: remove the two lines
   - `pub mod piece_map;`
   - `pub mod speed_hud;`

4. Update call sites:
   - `src/app.rs:1218`: `crate::ui::speed_hud::view` → `crate::ui::components::speed_hud::view`
   - `src/ui/details_dialog.rs:207`: `crate::ui::piece_map::view` → `crate::ui::components::piece_map::view`

5. No internal import edits needed inside the two moved files — they use `crate::message`, `crate::task`, `crate::ui::icon`, `crate::ui::theme`, all of which remain valid.

## Validation
- `cargo build`
- `cargo clippy --workspace`
- `cargo fmt --check`
- Re-grep `piece_map|speed_hud` to confirm no stale `crate::ui::piece_map` / `crate::ui::speed_hud` references remain.

## Risks / Notes
- The `theme` local-variable shadowing in `speed_hud.rs` is pre-existing and unrelated to the move; do not change it in this task.
- Optional follow-up (out of scope): rename `speed_hud_background` style or the `theme` variable for readability.