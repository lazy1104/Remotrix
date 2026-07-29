# Fix: pause icon does not update (status stays "active")

## Problem

When clicking pause on a task, the download stops at the aria2 level but the task card's icon
does not flip from pause -> play. Resume works correctly (icon flips play -> pause).

## Root cause

Confirmed in `src/engine.rs`. The UI's pause/resume icon is driven by `t.status`
(`src/ui/task_list.rs:210-220`), which is only updated when an `EngineEvent::Progress` arrives
(`src/app.rs:527-550`). On pause, no such event is ever delivered:

1. `EngineCmd::Pause(gid)` in `handle_client_cmd` (`src/engine.rs:352-355`) calls
   `client.pause(&gid).await` and emits **nothing** afterward.
2. The 1s poll loop (`src/engine.rs:595-610`) only calls `tell_active()`. Once paused, aria2
   moves the task OUT of the active list (into the waiting list), so the poller never reports
   the "paused" status again.
3. The notification handler (`src/engine.rs:572-579`) matches only
   `Complete | Error | Stop | BtComplete`, dropping `Event::Pause` and `Event::Start`
   (the `aria2_ws::Event` enum has `Start, Pause, Stop, Complete, Error, BtComplete`).

Why resume works: `unpause` re-enters the active list, so `tell_active()` picks it up within
~1s and emits `Progress { status: "active" }`, flipping the icon back.

## Fix

Two complementary, low-risk changes in `src/engine.rs`. Both are required: (B) gives
deterministic immediate feedback for the button click; (A) makes the engine reflect any
Start/Pause transition regardless of source.

### A. Handle `Event::Start` and `Event::Pause` notifications

In `on_sidecar_ready`'s notification task (`src/engine.rs:572-579`), add `Start` and `Pause`
to the match arms so a `tell_status` + `emit_progress` fires:

```rust
match event {
    Event::Start | Event::Pause | Event::Complete | Event::Error | Event::Stop | Event::BtComplete => {
        if let Ok(status) = notif_client.tell_status(&gid).await {
            emit_progress(&notif_event_tx, &status).await;
        }
    }
    _ => {}
}
```

### B. Emit progress immediately after pause/resume RPC calls

In `handle_client_cmd` (`src/engine.rs:352-380`), after each control RPC call, fetch the
affected task(s) and emit `Progress` so the UI updates without waiting on a notification or
the next poll tick.

- `EngineCmd::Pause(gid)`: after `client.pause(&gid).await`, `client.tell_status(&gid)` then
  `emit_progress`.
- `EngineCmd::Resume(gid)`: after `client.unpause(&gid).await`, `client.tell_status(&gid)`
  then `emit_progress`.
- `EngineCmd::PauseAll`: after `client.pause_all().await`, iterate `fetch_all_tasks(client)`
  and `emit_progress` each (paused tasks live in the waiting list, so `tell_active()` alone is
  insufficient).
- `EngineCmd::ResumeAll`: after `client.unpause_all().await`, iterate `fetch_all_tasks(client)`
  and `emit_progress` each.

Note: duplicate `Progress` events (notification + explicit emit) are harmless — the app's
update handler is idempotent (`src/app.rs:543-550`).

## Out of scope

- Polling waiting/stopped tasks periodically (only transitions matter for the icon; progress
  for non-active tasks is not requested).
- DB status persistence of paused state (separate concern, not reported).

## Validation

1. `cargo clippy --workspace` — no warnings.
2. `cargo fmt --check`.
3. `cargo build`.
4. Manual: add a download, click pause -> icon flips to play within ~instant; progress bar
   color turns warning (paused); click resume -> icon flips back to pause and speed resumes.
5. Manual: PauseAll / ResumeAll flip all task icons correctly.
