# Fix: silence aria2-ws per-frame debug log spam

## Context (corrected)
User reported the log line `websocket received message: Ok(Some(Text(...)))` repeating ~every second, each carrying a payload with ~16 identical `uri` entries for a single download. User confirms they entered **one** URL and only **one** file downloaded.

## Root cause (corrected after empirical testing)
Two independent facts:

1. The ~16 `uri` entries are **normal aria2-next behavior**, NOT a remotrix bug and NOT multiple URLs/mirrors. For a single-URI download with `split=N`, aria2-next's `tellStatus.files[].uris` lists the URI `N+1` times (1 `used` + N `waiting`), all the same URL. Verified with the exact `aria2-next-2.5.2` binary (range-supporting local server, active downloads):
   - split=1 -> 2 uris; split=4 -> 5; split=8 -> 9; split=16 -> 17 (all `distinct=1`, 1 URL input).
   - The earlier claim "1 URL -> 1 uri" was wrong: that test used a URL that failed immediately (error state, split never engaged). Active downloads show the N+1 entries.
   - Default `split=16` (`ui/add_dialog.rs:25`, `config.rs:274`) -> ~16-17 entries, matching the user's log. Nothing to fix here.

2. The log line itself is `debug!("websocket received message: {:?}", msg)` in the `aria2-ws` crate (`aria2-ws-0.5.1/src/client.rs:115`), emitted for EVERY inbound WebSocket frame. It is enabled because `src/main.rs:48` sets the default `EnvFilter` to `"info,remotrix=debug,aria2_ws=debug"`. It repeats ~every 1s because `src/engine.rs:596` polls `tell_active()` every 1000ms; each response is a frame -> one debug line (notifications and `tell_status` add more).

## Fix (one line: `src/main.rs:48`)
Drop the `aria2_ws=debug` directive so the per-frame `debug!` spam is filtered out (aria2-ws falls back to the default `info` level):

```rust
// before
.unwrap_or_else(|_| EnvFilter::new("info,remotrix=debug,aria2_ws=debug"));
// after
.unwrap_or_else(|_| EnvFilter::new("info,remotrix=debug"));
```

Effect:
- Silences `websocket received message: ...` and `writing message to websocket: ...` (both `debug!` in aria2-ws `client.rs`).
- Keeps aria2-ws `info!`/`warn!` connection logs ("connecting to", "connected to", "aria2-ws reconnected", "aria2-ws connection closed") - useful and infrequent.
- Keeps all `remotrix=debug` logs and the default `info` for everything else.
- Users who want the per-frame logs back can set `RUST_LOG=aria2_ws=debug` (env override still works via `EnvFilter::try_from_default_env()` at `main.rs:47`).

## Validation
1. `cargo fmt --check`
2. `cargo clippy --workspace` (warning-free)
3. `cargo build`
4. Manual: run app, add a single URL download. Confirm the log no longer spams `websocket received message: ...` every second. Confirm aria2-ws connection `info` lines still appear once at startup. Confirm the download still progresses normally (progress polling unaffected).

## Scope / out of scope
- In scope: `src/main.rs:48` one-line filter change only.
- Out of scope (not bugs): the `N+1` uri entries in `tellStatus` are normal aria2-next split behavior - no change needed. The multi-line-URL-as-mirrors quirk (`engine.rs:340` passes all lines to one `add_uri`) is a separate design consideration, NOT the user's issue (they enter 1 URL); left for future.
