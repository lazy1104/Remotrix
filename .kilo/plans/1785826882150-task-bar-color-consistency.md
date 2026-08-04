# Unify task progress bar color

## Goal
Make the progress bar color in `src/ui/details_dialog.rs` match the task card in `src/ui/task_list.rs`, and avoid drift by sharing one helper.

## Current behavior
- Task card (`task_list.rs:430-435`):
  ```rust
  match t.status {
      Paused => theme::primary_weak(theme),
      Error => theme::danger(theme),
      Completed => theme::success(theme),
      _ => theme::primary(theme),   // Active, Waiting
  }
  ```
- Details dialog (`details_dialog.rs:238-246` activity tab and `336-344` files tab) — inconsistent:
  ```rust
  match task.status {
      Paused => theme::warning(theme),
      Error => theme::danger(theme),
      _ => theme::success(theme),  // Active, Waiting, Completed
  }
  ```

## Changes

### 1. Add shared helper in `src/ui/theme.rs`
Add near the other color helpers (after `primary_weak`, ~line 262):
```rust
pub fn task_bar_color(t: &Theme, status: crate::task::TaskStatus) -> Color {
    match status {
        crate::task::TaskStatus::Paused => primary_weak(t),
        crate::task::TaskStatus::Error => danger(t),
        crate::task::TaskStatus::Completed => success(t),
        _ => primary(t),
    }
}
```

### 2. Use helper in `src/ui/task_list.rs`
Replace the inline `bar_color` match (lines 430-435) with:
```rust
let bar_color = theme::task_bar_color(theme, t.status);
```

### 3. Use helper in `src/ui/details_dialog.rs`
Replace the inline `bar_color` match in both `activity_tab` (lines 238-243) and `files_tab` (lines 336-341) with:
```rust
let bar_color = theme::task_bar_color(theme, task.status);
```

## Validation
- `cargo build` (offline)
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual: open Details dialog for a downloaded task and confirm the bar follows the card color (paused = weak primary, error = danger, completed = success, active/waiting = primary).

## Notes
- No i18n or type changes; `TaskStatus` is already `Copy` + `PartialEq` so passing by value is fine.
- `theme.rs` already imports `iced::{Color, Theme}`; only needs `crate::task::TaskStatus` (referenced via full path to avoid new imports).