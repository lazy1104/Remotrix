# Fix: pause all downloads on graceful quit

## Problem (user report)

Closing Remotrix does not pause downloads; on relaunch, previously active tasks auto-resume.

Root cause: the `EngineCmd::Shutdown` arm in `run_supervisor` (`src/engine.rs:889-896`) only runs
`save_session` + `shutdown`. aria2's session file records active tasks as active, and on the next
launch `--input-file` re-adds them in the active state, so downloads start again immediately.

## Decision (confirmed with user)

- **Always** pause all tasks (active + waiting + seeding) on graceful quit. No new settings item,
  no i18n changes, no settings UI changes.
- `RestartEngine` must **not** pause (engine restart keeps tasks running). Only the final-quit
  `EngineCmd::Shutdown` arm changes.

## Task list

### 1. `src/engine.rs` — pause before saving the session in the `Shutdown` arm

Replace the body of the `EngineCmd::Shutdown` arm (currently lines 889-896):

```rust
EngineCmd::Shutdown => {
    if let Some(ref s) = sidecar {
        let _ = s.client.pause_all().await;
        let mut paused = false;
        for _ in 0..10 {
            match s.client.tell_active().await {
                Ok(list) if list.is_empty() => {
                    paused = true;
                    break;
                }
                _ => {}
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if !paused {
            tracing::warn!("tasks did not fully pause before session save");
        }
        for status in fetch_all_tasks(&s.client).await {
            emit_progress(&event_tx, &status).await;
        }
        let _ = s.client.save_session().await;
        let _ = s.client.shutdown().await;
    }
    let _ = event_tx.send(EngineEvent::EngineStopped);
    break;
}
```

Notes:
- `pause_all` stops active and waiting downloads; seeding tasks are paused too.
- The bounded poll (up to 10 × 200 ms = 2 s) makes the session save deterministic — the session is
  saved only after no active tasks remain. `tokio::time::sleep` (never `std::thread::sleep`).
- `fetch_all_tasks` + `emit_progress` publish the now-paused status to the UI before
  `EngineStopped`, so `finalize_close`'s `flush_dirty` persists `paused` to `remotrix.db` (consistent
  with the session file). All helpers already exist in this file (`fetch_all_tasks` line 357,
  `emit_progress` line 345, `Duration` imported line 10).
- RPC calls are processed sequentially by aria2, so `pause_all` completes before `save_session`.
- Log the failure with `tracing::warn!` if `pause_all` errors (use `if let Err(e) = ...`), but do not
  block shutdown — the pre-existing behavior (active session) is the acceptable degradation.

### 2. No other code changes

- `app.rs`, `message.rs`, `config.rs`, i18n, and settings UI are untouched — `begin_close` already
  sends `EngineCmd::Shutdown`.
- `RestartEngine` arm (engine.rs:920-933) is unchanged — it keeps `save_session` + `shutdown` and
  does not pause, so restarts resume active tasks.

## Edge cases / out of scope

- **SIGKILL / crash quit**: `--stop-with-process` + startup orphan cleanup handle the process side,
  but the session still reflects the last 5 s checkpoint with active state, so a relaunch after a
  hard kill may resume. Unavoidable (no graceful path runs); documented limitation, unchanged by
  this fix.
- **ShutdownTimeout path**: if the engine never processes `Shutdown` (e.g., stuck in `boot`), the
  window force-closes after 5 s and no pause happens — same worst case as before. The added pause
  poll (≤2 s) + save + shutdown fit comfortably inside the existing 5 s timeout.
- **Ghost re-add on relaunch**: tasks saved as paused appear in aria2's `tell_waiting` with status
  `paused`, so `sync_existing_tasks` marks them `synced_gids` and the ghost `ReaddTask` fallback is
  not triggered. Relaunch shows them paused.

## Validation

- `cargo build`, `cargo clippy --workspace` (no warnings), `cargo fmt --check`.
- Start a download → close app via X → confirm → log shows shutdown; session save happens after
  pause. `aria2.pid` removed. `session.txt` lines for previously-active tasks start with a space
  (aria2 paused-task marker).
- Relaunch → tasks show as **paused**, no download traffic; click resume → downloads continue.
- Regression: use the engine-restart button (`RestartEngine`) with a download running → tasks stay
  active (no pause) after restart.
- Regression: normal add/pause/resume/remove still work.
