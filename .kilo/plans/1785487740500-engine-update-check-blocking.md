# Fix: startup update check blocks engine command loop, stalling AddDownload

## Log evidence (`.local/share/remotrix/logs/remotrix.log.2026-07-31`, UTC times)

- **Per-URL fix works**: `08:52:14` and `08:53:02` → engine `add download urls=[...2 urls...]`, then two `ui: task added` lines with correct per-URL basenames (`SPlayer-3.0.0-x64.zip`, `splayer-3.0.0-x64.pacman`).
- **Failing runs**: `08:53:47` and `08:54:05` → `ui: add download submitted count=2` is logged, but there is **no** engine `add download` line and **no** `task added`. In both runs the line `check aria2 update` appears right before (`08:53:38.505`, `08:53:58.120`) and no completion/error line follows before shutdown (`08:53:54`, `08:54:11`).

## Root cause

- `run_supervisor` handles `EngineCmd::CheckAria2Update` **inline**: `engine.rs:849-851` calls `handle_check_update(&event_tx).await` directly in the `tokio::select!` command loop.
- `handle_check_update` (`engine.rs:643`) awaits `updater::fetch_latest_release` (`updater.rs:25`), whose reqwest client (`updater.rs:27-30`) has **no timeout** (`Client::builder()` without `.timeout(...)`; same for `aria2_fetcher.rs:309`).
- When the GitHub API request hangs (observed 08:53:38→shutdown, 08:53:58→shutdown), the whole command loop is blocked: queued commands — including `AddDownload` — are never processed. The app UI shows nothing added.
- The check is auto-triggered at startup by `app.rs:853-861` (`Aria2Version` handler, `should_auto_check("aria2-next")`), so this hits every launch, racing the user's first action.

## Fix

1. **`src/engine.rs:849-851`** — run the check on a detached task so the command loop stays responsive:

   ```rust
   EngineCmd::CheckAria2Update => {
       let tx = event_tx.clone();
       tokio::spawn(async move {
           handle_check_update(&tx).await;
       });
   }
   ```

   `handle_check_update` takes `&EventTx`; `event_tx` is an `mpsc::UnboundedSender` (Clone). The spawned task already communicates solely via the event channel (including `stage_update_download` progress), so nothing else needs to change. `Shutdown` breaking the loop will simply drop the channel; the task exits harmlessly.

2. **`src/updater.rs:27-30`** — add a request timeout to bound a hung check:

   ```rust
   let client = reqwest::Client::builder()
       .user_agent("remotrix-updater")
       .timeout(std::time::Duration::from_secs(30))
       .build()
       .map_err(|e| format!("create reqwest client: {e}"))?;
   ```

   This ensures a dead/slow network fails cleanly with the existing `update check failed: {e}` log + `Aria2CheckResult` event rather than lingering.

## Files to modify

- `src/engine.rs` — `CheckAria2Update` arm in `run_supervisor` (lines 849-851).
- `src/updater.rs` — reqwest builder in `fetch_latest_release` (lines 27-30).

## Out of scope (pre-existing, unrelated)

- `boot apply global options: aria2: json error: invalid type: string "OK", expected unit` at every boot — `change_global_option` response parsing issue in `aria2-ws 0.5` (`engine.rs:630`, `engine.rs:722`). Warning only, does not block; do not touch in this change.

## Validation

- `cargo build`, `cargo clippy --workspace` (no warnings), `cargo fmt --check`.
- Manual: launch the app and **immediately** (while the startup update check is in flight or hung) paste 2-3 URLs and submit → each URL appears as its own task with its own basename, without waiting for the update check.
- Manual: with network unavailable (or slow), startup still leaves the UI fully responsive; add/pause/remove work; the update check eventually reports failure (`update check failed` in log) instead of blocking.
- Re-confirm the original multi-URL case: one intentionally-bad URL in the batch logs `add_uri failed` while the others are added.
