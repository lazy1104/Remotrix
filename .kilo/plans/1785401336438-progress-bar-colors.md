# Progress Bar Color Adjustment (v2)

## Goal
- Rename `prime` → `primary` function (more conventional naming)
- Paused progress bar: use a weaker shade of `primary` instead of `background.weak`

## Changes

### 1. `src/ui/theme.rs` — Replace `prime` and `disabled` with `primary` and weaker primary variant
```rust
pub fn primary(t: &Theme) -> Color {
    t.extended_palette().primary.base.color
}

pub fn primary_weak(t: &Theme) -> Color {
    t.extended_palette().primary.weak.color
}
```
Remove the old `prime()` and `disabled()` functions.

### 2. `src/ui/task_list.rs` — Update bar_color match
```rust
let bar_color = match t.status {
    TaskStatus::Paused => theme::primary_weak(theme),
    TaskStatus::Error => theme::danger(theme),
    _ => theme::primary(theme),
};
```
- Active/Waiting: `primary` (full primary brand color)
- Paused: `primary_weak` (softer/weaker shade of primary)
- Error: `danger` (unchanged)

## Verification
- `cargo build` succeeds
- `cargo clippy --workspace` passes (no new warnings)
