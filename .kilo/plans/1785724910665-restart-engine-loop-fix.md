# Fix Endless Engine Restart Loop + Restart Button Position

## Goal

1. Stop the engine from restarting endlessly after a single manual restart (or after
   ApplySettings/ApplyAndLeaveSettings with ED2K changes).
2. Move the "Restart Engine" button in the Settings actions bar so it sits immediately
   to the right of the Reset button (currently it is pushed to the far right edge).

## Root Cause of the Endless Restart Loop

`engine.rs` `EngineCmd::RestartEngine` (line ~1635) now correctly does
`shutdown()` + `boot()` in one command, but this introduced a stale-notification race:

1. Every `boot()` (engine.rs:1333-1340) spawns a task that awaits the aria2 child and
   sends `()` into the `restart_tx`/`restart_rx` channel when the process exits.
2. Manual restart calls `s.client.shutdown().await` (engine.rs:1655). The old process
   exits → its wait-task sends `()` into `restart_rx`. This message stays buffered
   while the replacement is being booted.
3. After the replacement boot completes, the supervisor `select!` (engine.rs:1688)
   receives the stale `()` in the crash-recovery branch, wrongly concludes the NEW
   sidecar crashed, aborts its poll/scheduler handles, sets `sidecar = None`, and
   boots again.
4. That re-boot calls `cleanup_stale_aria2` (engine.rs:1326, 1240-1317), which reads
   `aria2.pid` (still the pid of the abandoned-but-alive new sidecar), finds it alive
   and ours, and SIGTERMs it.
5. That kill triggers another wait-task `()` → crash branch → boot → SIGTERM → ...
   **infinite loop**. `retry_count` resets to `0` on each successful boot, so the
   `MAX_RETRIES = 3` guard never trips.

Consequence: the user sees the engine toast repeatedly and the process keeps cycling.

## Fix Design: Generation-Counted Exit Notifications

Make each child-exit notification carry the generation of the sidecar that spawned it.
The crash-recovery branch ignores notifications whose generation does not match the
current sidecar's generation (i.e. stale notifications from a sidecar we deliberately
shut down).

### Changes in `src/engine.rs`

1. Change the channel payload from `()` to `u64` (the generation):
   - Line 1518: `mpsc::unbounded_channel::<u64>()`
   - Add `let mut generation: u64 = 0;` in `run_supervisor` next to `retry_count`.

2. `boot()` (line 1319): add a `generation: u64` parameter; change
   `restart_tx: &mpsc::UnboundedSender<()>` to `&mpsc::UnboundedSender<u64>`.
   In the child wait-task closure (lines 1333-1340), capture `let gen = generation;`
   and send `gen` instead of `()`.

3. Update all four `boot()` call sites to pass the generation:
   - Initial boot (line 1526): pass `generation` (0). No increment.
   - `RetryAria2Fetch` (line 1620): `generation += 1;` before `boot(...)` (it may
     abandon a running sidecar).
   - `EngineCmd::RestartEngine` (line 1660): `generation += 1;` before `boot(...)`.
   - Crash-recovery branch (line 1701): pass `generation` unchanged (the notification
     was just consumed; the replacement inherits the same generation).

4. Crash-recovery branch (lines 1688-1717): bind the payload, e.g.
   `gen = restart_rx.recv() =>`, and add at the top:
   ```rust
   if gen != generation {
       tracing::debug!(?gen, current = generation, "ignoring stale sidecar exit notification");
       continue;
   }
   ```
   Keep the existing recovery logic below it unchanged.

### Why this is correct

- Manual restart: old sidecar gen G is shut down → its notification `G` arrives after
  generation has been incremented to `G+1` → `G != G+1` → ignored. The replacement is
  never aborted, `cleanup_stale_aria2` never SIGTERMs it, so no further `()` is
  generated. One restart, one `EngineReady` toast.
- Genuine crash: the crashed sidecar's notification `G` matches current `G` → recovery
  runs exactly once.
- Back-to-back restarts (e.g. ApplySettings + manual): every abandoned sidecar's
  notification carries an older generation and is ignored.
- Crash-recovery keeps the same generation because each child sends exactly one
  notification (consumed before re-boot), so there is no collision.

### No changes needed elsewhere

- `Message::ApplySettings` / `Message::ApplyAndLeaveSettings` already send
  `EngineCmd::RestartEngine`; the generation fix covers them. (Optional UX nicety,
  out of scope: set `engine_restart_in_progress` there too so the button disables.)

## Fix Design: Restart Button Position

In `src/ui/settings_page.rs` the actions bar (lines 128-162) is currently:

```
[Apply] [Reset] [Space::Fill] [RestartEngine]
```

Reorder so RestartEngine is adjacent to Reset:

```
[Apply] [Reset] [RestartEngine] [Space::Fill]
```

Move the `actions.push(iced::widget::Space::new().width(Length::Fill));` line (145)
to AFTER the RestartEngine button push (after line 162). Keep the
`on_press_maybe(engine_restart_in_progress)` guard and the Reset button
always-visible behavior unchanged.

## Files to Modify

| File | Change |
|---|---|
| `src/engine.rs` | `restart_tx/rx` payload `()` → `u64`; add `generation` counter; `boot()` gains `generation` param and sends it; increment before boot in RestartEngine and RetryAria2Fetch; ignore mismatched generations in crash branch. |
| `src/ui/settings_page.rs` | Move the fill `Space` after the RestartEngine button so it sits right of Reset. |

## Edge Cases

| Scenario | Handling |
|---|---|
| Manual restart with active downloads | Shutdown/pause flow unchanged; resume-on-ready unchanged; generation prevents spurious re-boot. |
| ApplySettings (ED2K changed) triggers restart | Same `RestartEngine` cmd → generation increments → one restart. Button may stay enabled during it (accepted; a second click queues a second, safe restart). |
| Genuine aria2-next crash | Notification matches current generation → auto-recovery still works. |
| Crash loop after MAX_RETRIES | Unchanged: `Aria2FetchFailed` after 3 genuine consecutive crashes. |
| App close during restart | `Shutdown` takes over; supervisor exits; generation is discarded. |

## Validation

1. `cargo fmt --check` passes.
2. `cargo clippy --workspace` passes with no warnings.
3. `cargo build` compiles.
4. Manual: click Restart Engine → confirm → exactly one "engine started" toast;
   no repeated restarts in logs (`grep "aria2-next exited"` appears at most once
   for an intentional restart, and the engine stays up).
5. Manual: with ED2K changed, click Apply → single restart, no loop.
6. Manual: confirm the Restart Engine button renders immediately right of Reset
   (Apply | Reset | Restart | spacer).
7. Manual: verify genuine crash recovery still works (kill the aria2-next process →
   it re-boots once, then stays).
