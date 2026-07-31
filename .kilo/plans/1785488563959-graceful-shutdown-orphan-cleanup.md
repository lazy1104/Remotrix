# Fix: graceful shutdown handshake + stop aria2 on abnormal close

## Problem (from code audit)

1. **Normal close is racy**: `app.rs:975-1001` `CloseDialog(Close)` sends `EngineCmd::Shutdown` and immediately returns `iced::window::close(id)`. The UI never waits for `EngineEvent::EngineStopped`. The `run_supervisor` task runs on iced's tokio runtime; if the runtime is dropped before it processes the queued command, the `Child` handle (held by the wait task, `engine.rs:707-714`) is dropped and `kill_on_drop(true)` (`engine.rs:247`) SIGKILLs aria2 — session then only reflects the last `--save-session-interval 5` checkpoint.
2. **Abnormal close leaves an orphan**: SIGKILL / SIGTERM / crash kills remotrix without running destructors. `kill_on_drop` never fires; aria2-next is reparented to PID 1 and keeps downloading with a random port/secret nobody can control. On next launch a second aria2 is spawned against the same `session.txt`, causing double downloads / file conflicts.

## Decisions (confirmed with user)

- **Abnormal close → aria2 stops too**: spawn aria2 with `--stop-with-process=<remotrix pid>` so it self-terminates shortly after remotrix dies, plus startup cleanup of stale orphans via PID file + SIGTERM/SIGKILL (covers orphans from before this fix and immediate relaunches).
- **Include SIGTERM/SIGINT handling**: a subscription listens for termination signals and routes into the same graceful close path as the close button (RPC `save_session` + `shutdown` → flush DB/config → close window).

## Task list

### 1. `Cargo.toml` — add `libc`

```toml
libc = "0.2"
```

Only used (and only referenced under `#[cfg(unix)]`) for `libc::kill` in orphan cleanup.

### 2. `src/message.rs` — new `Message` variants

Add to the `Message` enum (~line 90, near `CloseDialog`):

```rust
ShutdownRequested,   // from SIGTERM/SIGINT subscription
ShutdownTimeout,     // from the 5s close timeout
```

Both unit variants; `Message` already derives `Clone`.

### 3. `src/app.rs` — close handshake

- **State**: add `closing: bool` to `Remotrix` (near `pending_close`, line 65); init `false` in `init()` (near line 153).
- **Extract helper** from the `FlushDirty` handler (lines 1118-1151): `fn flush_dirty(state: &mut Remotrix)`; call it from both the `FlushDirty` arm and `finalize_close`.
- **New helpers**:
  - `fn begin_close(state: &mut Remotrix) -> Task<Message>` — guard `if state.closing { return Task::none(); }`; set `closing = true`; log `ui: shutdown requested`; send `EngineCmd::Shutdown`; return `shutdown_timeout_task()`.
  - `fn shutdown_timeout_task() -> Task<Message>` — `Task::perform(async move { tokio::time::sleep(Duration::from_secs(5)).await }, |_| Message::ShutdownTimeout)`.
  - `fn finalize_close(state: &mut Remotrix) -> Task<Message>` — guard `if !state.closing { return Task::none(); }`; set `closing = false`; `flush_dirty(state)`; then the exact logic currently in the `Close` arm (lines 983-996): if `geometry_dirty` → `pending_close = true` + `iced::window::is_maximized(id).then(...)`; else `sync_geometry_to_settings(state)`; `config::save(&state.settings)`; `iced::window::close::<Message>(id)` (fallback `Task::none()` when no `window_id`).
- **Rework handlers**:
  - `CloseRequested` (line 972): guard — `if state.closing { return Task::none(); }` before showing the dialog.
  - `CloseDialog(Close)` (line 978): replace body with `begin_close(state)`. `Cancel`/`MinimizeToTray` unchanged.
  - `EngineEvent::EngineStopped` (line 672): keep existing state clears, then `if state.closing { return finalize_close(state); }`.
  - New arms: `Message::ShutdownRequested => begin_close(state)`; `Message::ShutdownTimeout => { if state.closing { tracing::warn!("engine did not stop in time, closing anyway"); } finalize_close(state) }` (finalize_close's own guard makes the `if` safe).
- The existing `WindowMaximized` `pending_close` path (lines 1010-1027) already performs the geometry save + close; no change.

### 4. `src/app.rs` — SIGTERM/SIGINT subscription

- Add a stream builder:

```rust
fn signal_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(4, |mut sender: iced::futures::channel::mpsc::Sender<Message>| async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = signal(SignalKind::terminate()).ok();
            let mut int = signal(SignalKind::interrupt()).ok();
            let _ = tokio::select! {
                _ = async { if let Some(ref mut t) = term { let _ = t.recv().await; } }, if term.is_some() => {}
                _ = async { if let Some(ref mut i) = int { let _ = i.recv().await; } }, if int.is_some() => {}
            };
            tracing::info!("termination signal received");
        }
        #[cfg(not(unix))]
        {
            std::future::pending::<()>().await;
        }
        let _ = sender.send(Message::ShutdownRequested).await;
    })
}
```

  (Implementer may simplify: use `tokio::select!` over the two `Option<Signal>` receivers or a single `SignalKind::terminate() | interrupt()` listener; any correct formulation is fine.)
- Register in `subscription()` (line 1522-1531 batch): `Subscription::run_with((), |_| signal_stream())`.

### 5. `src/engine.rs` — spawn with `--stop-with-process` + PID file

- In `Sidecar::spawn` (line 227): restructure the `Command` builder so the stop flag is added conditionally:

```rust
let mut cmd = Command::new(bin_path);
#[cfg(unix)]
cmd.arg("--stop-with-process").arg(std::process::id().to_string());
cmd.arg("--enable-rpc").arg("--rpc-listen-all=false")/* ...existing args... */;
let mut child = cmd.spawn().map_err(|e| format!("spawn aria2-next: {e}"))?;
```

  (`--stop-with-process` is a standard aria2 option retained by aria2-next per its README: "CLI aria2 option names and behavior" intact.)
- In the Ok branch after the RPC connect succeeds (line 268-273), before returning:

```rust
if let Ok(pid) = child.id() {
    let pid_path = config.session_path.join("aria2.pid");
    if let Err(e) = std::fs::write(&pid_path, pid.to_string()) {
        tracing::warn!(?e, "write aria2 pid file failed");
    }
}
```

### 6. `src/engine.rs` — stale orphan cleanup

Add (all under `#[cfg(unix)]`; empty `#[cfg(not(unix))]` stub):

```rust
async fn cleanup_stale_aria2(bin_path: &Path, pid_path: &Path) {
    let Ok(content) = std::fs::read_to_string(pid_path) else { return };
    let Ok(pid) = content.trim().parse::<i32>() else {
        let _ = std::fs::remove_file(pid_path);
        return;
    };
    let alive = std::path::Path::new(&format!("/proc/{pid}")).exists();
    let is_ours = std::fs::read_link(format!("/proc/{pid}/exe"))
        .map(|p| p == bin_path)
        .unwrap_or(false);
    if alive && is_ours {
        tracing::warn!(%pid, "stale aria2-next from previous run detected, SIGTERM");
        unsafe { libc::kill(pid, libc::SIGTERM) };
        let mut waited = 0;
        while std::path::Path::new(&format!("/proc/{pid}")).exists() && waited < 50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            waited += 1;
        }
        if std::path::Path::new(&format!("/proc/{pid}")).exists() {
            tracing::warn!(%pid, "stale aria2-next still alive, SIGKILL");
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    }
    let _ = std::fs::remove_file(pid_path);
}
```

  - Call it in `boot()` (engine.rs:695-717) after `ensure_aria2_next` returns and before `Sidecar::spawn`:
    ```rust
    let pid_path = config.session_path.join("aria2.pid");
    cleanup_stale_aria2(&bin_path, &pid_path).await;
    ```
  - Safe on every `boot()` call (initial / `RetryAria2Fetch` / `RestartEngine` / `restart_rx`): in all restart paths the previous aria2 is already dead or RPC-shutdown, so its pid fails the `is_ours`/alive check and only the stale pidfile is removed.
  - `--save-session` is passed to the orphan too, so SIGTERM makes it save the session before exiting; the new instance loads it via `--input-file`.
  - The exe `read_link` comparison prevents killing an unrelated process on PID reuse (comparison with the same binary is a deliberate accept).
- Remove the pidfile on supervisor exit: after the `loop` ends (before line 942 `engine supervisor stopped`):
  ```rust
  let _ = std::fs::remove_file(session_path.join("aria2.pid"));
  ```
  (`session_path` is in scope at line 804.) `RestartEngine` does not break the loop, so the file persists and is overwritten by the next spawn.

## Out of scope

- Fixing the pre-existing `boot apply global options: json error` warning (`change_global_option` response parsing in `aria2-ws 0.5`).
- The startup update-check hang fix (separate plan `1785487740500-engine-update-check-blocking.md`).
- Multi-instance support (two simultaneous remotrix processes fight over `session.txt`; this change makes the second launch kill the first's aria2 — an improvement, not a full single-instance lock).
- Tray/minimize-to-close behavior.

## Risks / notes

- Timeout fallback: if the engine can't stop within 5 s (e.g., aria2 fetch still hanging during `boot`), the window force-closes; worst case matches today's behavior (SIGKILL via `kill_on_drop`, ≤5 s session loss).
- `--stop-with-process` poll is ~1 s; a crash + immediate relaunch within that window is exactly what the startup cleanup covers.
- `std::process::exit` is never used; the signal path exits via the normal iced close flow, so destructors/config/DB all run.

## Validation

- `cargo build`, `cargo clippy --workspace` (no warnings), `cargo fmt --check`.
- **Normal close**: start a download, click X → confirm → log shows `ui: shutdown requested` then `engine stopped` *before* the window closes; `aria2.pid` gone; `session.txt` contains the task; relaunch → task restored and resumable.
- **SIGTERM**: `kill <remotrix pid>` mid-download → log `termination signal received`, graceful close, aria2 exits, session saved.
- **SIGINT**: Ctrl+C from the launching terminal → same graceful path.
- **SIGKILL**: `kill -9 <remotrix pid>` mid-download → aria2-next becomes an orphan for ≤ ~1 s, then exits by itself (stop-with-process); `session.txt` reflects at most the last 5 s.
- **Restart after crash**: while the orphan still runs, relaunch → log `stale aria2-next from previous run detected, SIGTERM`; old process exits; new engine starts from saved session; no duplicate downloads.
- **No regression**: add/pause/resume/remove still work during normal operation.
