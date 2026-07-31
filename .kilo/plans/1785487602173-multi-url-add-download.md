# Fix: multi-line URL dialog only creates one download record

## Problem
In the Add Download dialog the URL editor accepts multiple lines, but submitting them creates only **one** task.

Root cause: `src/engine.rs` `handle_client_cmd` `EngineCmd::AddDownload` arm (lines 423–453) passes the full `Vec<String>` to a single `client.add_uri(urls, ...)` call (line 442). Per aria2 semantics, `addUri([u1, u2, ...])` treats the array as **mirror/alternative URLs for a single download** → one GID → one `EngineEvent::Added` → one task record in `app.rs`.

The UI layer is correct: `src/app.rs:351-358` collects every non-empty line and `url_count()` (add_dialog.rs:77) counts them all.

## Fix
In `src/engine.rs`, rewrite the `EngineCmd::AddDownload` arm to iterate per URL:

- Loop over `urls`, and for each URL call `client.add_uri(vec![url.clone()], Some(options.clone()), None, None)`.
- Build the base `TaskOptions` (dir/split/max_connection_per_server + `advanced.apply()`) **once**, clone per iteration (`TaskOptions: Clone` — implied by `EngineCmd: Clone`).
- Emit one `EngineEvent::Added { gid, name: basename(&url)..., url, dir }` per successful call.
- Name per URL via existing `basename()` (engine.rs:298), fallback to gid as today.

### Error handling (per-URL)
- On an individual `add_uri` failure: `tracing::error!` the error and **continue** with the next URL (a single bad link must not drop the whole batch).
- Return `Err` only if **zero** URLs succeeded (preserves existing upstream behavior at engine.rs:882-884 where errors are logged; a mix of failures should still leave the successful tasks added).

### No change needed elsewhere
- `app.rs` `EngineEvent::Added` handler (lines 724-770) already inserts one `DownloadTask` per event — will work unchanged.
- `out` rename option: `app.rs:311-315` already clears `out` when `url_count() > 1`, so per-URL `out` never collides.
- Dialog/`EngineCmd` shape stays the same; single-URL flow is unchanged behaviorally.

## Files to modify
- `src/engine.rs` — only the `AddDownload` arm in `handle_client_cmd` (lines 423-453).

## Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual: run app, paste 2–3 URLs (one line each) in dialog → submit → task list shows N records, each with its own basename; one intentionally-bad URL in the batch → other URLs still appear, bad one logged.
