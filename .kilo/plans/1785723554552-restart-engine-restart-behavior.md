# Restart Engine Button Behavior Fix

## Goal

Fix the restart-engine flow so that:
1. Clicking the restart-engine button always shows a confirmation dialog with differentiated copy.
2. The button becomes disabled immediately after click and stays disabled until the restart finishes, plus a 1s cooldown.
3. The engine is restarted safely: active downloads are paused first, session is saved, engine shuts down, then boots again.
4. After successful restart, only the downloads that were active before the restart are resumed one by one.
5. Duplicate toasts ("engine restarting / started") stop appearing.

## Root Cause of Current "Endless" Toasts

`Message::RestartEngine` currently sends `EngineCmd::RestartEngine` without any guard. The engine restarts and emits `EngineEvent::EngineReady`, which shows an `EngineStarted` toast. Because the button stays enabled, rapid or repeated clicks trigger multiple restarts → multiple toasts.

Additionally, `EngineCmd::RestartEngine` in `engine.rs` only does `save_session` + `shutdown` when a sidecar exists; it never re-boots in that branch, so a second click is required to start the engine again, which also contributes to the confusing repeated-restart behavior.

## Design Decisions

### 1. Always show confirmation dialog

`Message::RestartEngine` will always set `state.confirm = Some(ConfirmAction::RestartEngine { has_active })`, never restart directly. `has_active` is computed from `state.tasks` at click time.

### 2. Differentiated dialog copy

Extend `ConfirmAction::RestartEngine` to carry `has_active: bool`. In `confirm_dialog.rs`, choose the title/body translation keys based on this flag.

Add these new translation keys:

| Key | zh-CN | en |
|---|---|---|
| `confirm-restart-engine-title` | 重启引擎？ | Restart engine? |
| `confirm-restart-engine-body` | 确定要重启 aria2-next 引擎吗？ | Restart the aria2-next engine? |
| `confirm-restart-engine-active-title` | 重启引擎？ | Restart engine? |
| `confirm-restart-engine-active-body` | 有下载任务正在运行，重启后任务将暂停并恢复。确定重启？ | Active downloads will be paused and resumed after restart. Continue? |

The existing `confirm-restart-engine-title/body` can be reused for the no-active case.

### 3. Disable button during restart + 1s cooldown

Add a new state field `engine_restart_in_progress: bool`.

Flow:
- `Message::RestartEngine`: if `engine_restart_in_progress` is true, ignore; otherwise open the confirm dialog.
- `Message::ConfirmRestartEngine`:
  - `state.engine_restart_in_progress = true`.
  - Record current active task gids into `restart_resume_gids: HashSet<String>`.
  - Send `EngineCmd::RestartEngine`.
  - Start a 10s safety timeout `Task` → `Message::EngineRestartSafetyTimeout`.
- `EngineEvent::EngineReady`:
  - Existing logic runs.
  - If `engine_restart_in_progress` is true, schedule a 1s delayed `Task` → `Message::EngineRestartCooldownFinished`.
- `Message::EngineRestartCooldownFinished`:
  - `state.engine_restart_in_progress = false`.
  - Clear `restart_resume_gids`.
- `Message::EngineRestartSafetyTimeout`:
  - If still in progress, reset `engine_restart_in_progress = false` and clear `restart_resume_gids`.
- On engine failure events (`EngineEvent::EngineDegraded`, `EngineEvent::Aria2FetchFailed`):
  - If `engine_restart_in_progress` is true, reset it immediately and clear `restart_resume_gids`.

`settings_page.rs` passes `engine_restart_in_progress` to the actions-bar restart button, which uses `on_press_maybe(None)` while in progress.

### 4. Safe engine restart: pause → save → shutdown → boot

Modify `EngineCmd::RestartEngine` in `engine.rs`:

```rust
EngineCmd::RestartEngine => {
    if let Some(ref s) = sidecar {
        // Pause active tasks before shutdown, mirroring Shutdown behavior
        let _ = s.client.pause_all().await;
        let mut paused = false;
        for _ in 0..10 {
            match s.client.tell_active().await {
                Ok(list) if list.is_empty() => { paused = true; break; }
                _ => tokio::time::sleep(Duration::from_millis(200)).await,
            }
        }
        if !paused {
            tracing::warn!("tasks did not fully pause before engine restart");
        }
        for status in fetch_all_tasks(&s.client).await {
            emit_progress(&event_tx, &status).await;
        }
        let _ = s.client.save_session().await;
        let _ = s.client.shutdown().await;
        sidecar = None;
    }
    // Always boot, regardless of whether a sidecar existed before
    match boot(&config, &restart_tx, &event_tx).await {
        Ok((s, applied)) => {
            poll_handles = on_sidecar_ready(&s, &event_tx);
            scheduler_handle = Some(start_scheduler(&s, &event_tx));
            sidecar = Some(s);
            retry_count = 0;
            if let Some(v) = applied {
                let _ = event_tx.send(EngineEvent::Aria2UpdateApplied { version: v });
            }
        }
        Err(e) => {
            let _ = event_tx.send(EngineEvent::EngineDegraded { reason: e });
        }
    }
}
```

This ensures the engine actually restarts in one command and active tasks are paused first.

### 5. Resume only pre-restart active tasks

Add new state field `restart_resume_gids: HashSet<String>`.

Add new engine command:

```rust
pub enum EngineCmd {
    // ... existing variants ...
    ResumeGids(Vec<String>),
}
```

In `handle_client_cmd`, implement `ResumeGids(gids)` by mirroring `ResumeAll` logic but filtering to gids in the list and tasks whose status is `Paused`:

1. Fetch all tasks.
2. Filter tasks where `gids.contains(&s.gid)` and `s.status == Aria2TaskStatus::Paused`.
3. Group by host (using first file URI), like `ResumeAll`.
4. Staggered unpause per group with 500ms interval.

In `app.rs` `EngineEvent::EngineReady`:

```rust
if !state.restart_resume_gids.is_empty() {
    let gids: Vec<String> = state.restart_resume_gids.iter().cloned().collect();
    if state.handle.cmd_tx.send(EngineCmd::ResumeGids(gids)).is_err() {
        tracing::warn!("resume gids cmd send failed");
    }
}
```

`restart_resume_gids` is cleared by the cooldown/timeout handlers.

## Files to Modify

| File | Changes |
|---|---|
| `src/message.rs` | Add `has_active` to `ConfirmAction::RestartEngine`; add `Message::EngineRestartCooldownFinished` and `Message::EngineRestartSafetyTimeout`; add `EngineCmd::ResumeGids`. |
| `src/engine.rs` | Update `EngineCmd::RestartEngine` to pause-all, save, shutdown, then always boot; implement `ResumeGids`. |
| `src/app.rs` | Add `engine_restart_in_progress` and `restart_resume_gids` fields; update init; update `RestartEngine`, add cooldown/timeout handlers; update `EngineReady` to send `ResumeGids`; reset in-progress on failure events. |
| `src/ui/settings_page.rs` | Pass `engine_restart_in_progress` into `view` and disable restart button while true. |
| `src/ui/confirm_dialog.rs` | Use `has_active` to choose title/body keys for `RestartEngine`. |
| `src/i18n.rs` | Add new `Tr` variants for no-active confirmation copy. |
| `i18n/locales/en/main.ftl` | Add no-active confirmation strings. |
| `i18n/locales/zh-CN/main.ftl` | Add no-active confirmation strings. |

## Edge Cases

| Scenario | Handling |
|---|---|
| User clicks restart while another restart is in progress | Ignored via `engine_restart_in_progress` guard. |
| Engine restart fails | Failure events reset `engine_restart_in_progress`; safety timeout also resets it after 10s. |
| No active downloads at click time | Empty `restart_resume_gids`; `ResumeGids` command receives empty list and no-ops. |
| Active task completes during pause-before-restart | Gid is still in `restart_resume_gids`, but `ResumeGids` filters by `Paused` status, so completed tasks are skipped. |
| User closes app during restart | `Shutdown` takes over; `engine_restart_in_progress` is discarded. |
| Restart triggered from "Restart to Update" button (Advanced tab) | Uses same `Message::RestartEngine`, so same confirm + disable + resume behavior applies. |

## Validation

1. `cargo fmt --check` passes.
2. `cargo clippy --workspace` passes with no warnings.
3. `cargo build` compiles.
4. Manual: click restart with no active downloads → confirm dialog with no-active copy → engine restarts → toast appears once → button disabled for ~1s after EngineReady.
5. Manual: start a download, click restart → confirm dialog with active-copy → engine pauses task, restarts, then resumes the same task.
6. Manual: double-click the restart button quickly → only one restart occurs.

## Open Question

The current implementation of `ResumeGids` mirrors `ResumeAll`'s host-based staggering. If you prefer a simpler sequential resume without host grouping, let me know and the plan can be adjusted.
