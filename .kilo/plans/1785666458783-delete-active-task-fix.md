# Fix: instant deletion of active (downloading) tasks

## Problem
1. *(fixed)* Deleting an active task used to resurrect it in the list (event-ordering race between background poll/notification tasks and `Removed`).
2. *(current)* Deleting an active task takes ~1–2s before the task disappears. User asks: why not just pause-then-delete for immediacy.

## Root Cause of the 1–2s delay
`remove_task_from_aria2` (`src/engine.rs:532`) calls `client.remove(gid)` (graceful `aria2.remove`) first. For an **active** download, aria2 does a graceful stop — it waits for the current piece / buffered writes to finish — and only then removes it. The RPC does not return until that stop completes, so the supervisor (and thus the UI's `Removed` event) is blocked ~1–2s. `force_remove` is only used as a fallback on error/timeout.

`aria2.forceRemove` behaves like forcePause + remove: it stops the download immediately without "actions which take time", and returns right away. This is exactly the "pause 然后 delete" the user suggests, in a single RPC call. For a task being deleted (possibly with its files), skipping the graceful stop is safe.

## Changes

### 1. `src/app.rs` — guard against resurrecting removed tasks (DONE, from previous iteration)
Already implemented and passing build/clippy/fmt:
- `Remotrix.removed_gids: HashMap<String, Instant>` (60s grace), populated in `remove_task_local` / `clear_all_local`.
- `gid_recently_removed` helper with lazy expiry.
- `Progress` re-creation condition and `Added` create branch both skip recently-removed gids.

No further changes needed here.

### 2. `src/engine.rs` — force removal first (instant), keep graceful as fallback
Rewrite `remove_task_from_aria2` (`engine.rs:532`) so `force_remove` is the primary call:

```rust
async fn remove_task_from_aria2(client: &Client, gid: &str) {
    if client.force_remove(gid).await.is_err() {
        let _ = client.remove(gid).await;
    }
    let mut gone = false;
    for _ in 0..25 {
        match client.tell_status(gid).await {
            Err(_) => {
                gone = true;
                break;
            }
            Ok(s) => {
                if matches!(
                    s.status,
                    Aria2TaskStatus::Removed | Aria2TaskStatus::Complete | Aria2TaskStatus::Error
                ) {
                    gone = true;
                    break;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let _ = client.remove_download_result(gid).await;
    if !gone {
        tracing::warn!(?gid, "remove: task still present after force");
    }
}
```

- `force_remove` returns in ~ms for active downloads (no graceful stop) → confirmation poll resolves on its first `tell_status` → total delay well under 100ms.
- Graceful `remove` remains only as a fallback if `force_remove` errors (e.g. unknown gid); for waiting/paused/complete tasks both paths are already instant.
- The post-`force_remove` grace poll stays as a safety net for stuck removals; it is no longer the primary delay.
- Remove the now-redundant "escalate after 5s poll" block introduced in the previous iteration (lines 555–578).
- `RemoveAll` / `FollowTorrent delete_after` benefit automatically (they reuse this helper).

## Validation
- `cargo build`, `cargo clippy --workspace`, `cargo fmt --check`.
- Manual:
  1. Add an HTTP download; while actively downloading, delete it → task disappears essentially instantly (< ~0.2s) and stays gone (no resurrection after the 10s slow-scan tick).
  2. Repeat with "remove" (keep files) and "delete with files"; verify files are deleted when requested.
  3. Delete a paused task and a completed task → still works (instant, unaffected).
  4. `DeleteAll` while multiple tasks download → list empties immediately and stays empty.
  5. Restart the app → deleted tasks do not come back from the saved session.

## Notes
- No channel-protocol, DB-schema, or UI changes; only `engine.rs` behavior changes in this iteration.
- Force-removing an active BitTorrent/seed task skips the graceful tracker unregister — acceptable for a delete operation.
