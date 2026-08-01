# Speed HUD: expand when downloading tasks exist

## Goal
The speed HUD (bottom-right overlay) should expand to show up/down speeds whenever
there is at least one task actively downloading (`TaskStatus::Active`), even when the
instantaneous speed is 0 (e.g. task just started or momentarily stalled). Currently it
collapses to a lone icon whenever `download == 0 && upload == 0`.

## Context
- `app.rs:1619-1623` already zeroes `dl`/`up` unless `state.active_count > 0`. `active_count`
  counts tasks with `TaskStatus::Active` only (maintained incrementally in update paths and on load).
- `speed_hud.rs:14` expands iff `download != 0 || upload != 0`.
- "下载中" = Active status (task.rs:55 labels Active as "Downloading"). Waiting tasks are queued,
  not downloading, so they should NOT trigger expansion.

## Changes

### 1. `src/ui/components/speed_hud.rs`
- Add an `active: bool` parameter to `view`.
- Change expansion condition (line 14) from
  `if download == 0 && upload == 0` to
  `if !active && download == 0 && upload == 0`.
- Signature becomes:
  `pub fn view<'a>(theme: &'a iced::Theme, active: bool, download: u64, upload: u64) -> Element<'a, Message>`

### 2. `src/app.rs` (view fn, line 1624)
- Pass the existing gate flag as the new arg:
  `speed_hud::view(t, state.active_count > 0, dl, up)`

## Edge cases
- Active task with 0 speed → expanded (shows "0 B/s"). Desired.
- No tasks / all paused / all completed → collapsed icon.
- Waiting-only tasks → collapsed (waiting is not downloading).
- `active_count > 0` but `global_speed` is `None` → expanded with 0 B/s (correct per new rule).

## Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual: start a download, verify HUD stays expanded while speed dips to 0; complete/pause all,
  verify HUD collapses.
