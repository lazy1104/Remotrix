# Staggered "Resume All" grouped by URL host

## Goal

When the user clicks "全部恢复" (Resume All / `Message::StartAll`), do **not** unpause every paused task at once (current `client.unpause_all()` burst). Instead:

1. Group paused tasks by their URL host ("按地址分隔组").
2. Within each host group, unpause one task every **500 ms** ("每组内每个500ms发起一个下载").
3. Different host groups resume **in parallel** (each group runs its own independent cadence).

This prevents a burst of simultaneous connections to a single server (connection-rejected / rate-limit), which is the reported problem.

## Current behavior

- `Message::StartAll` (src/app.rs:507): clears `paused_gids`, sends `EngineCmd::ResumeAll`.
- `EngineCmd::ResumeAll` (src/engine.rs:512): `client.unpause_all().await`, then emits progress for all fetched tasks.

## Change — all in `src/engine.rs`

1. Add helper `url_host(uri: &str) -> Option<String>`:
   - `reqwest::Url::parse(uri)` (reqwest is already a dependency and re-exports `url::Url`; `Url` is available even with `default-features = false`).
   - Return `u.host_str().map(|h| h.to_ascii_lowercase())`.
   - Returns `None` for torrents/magnet/malformed/empty URLs.

2. Rewrite the `EngineCmd::ResumeAll` arm:
   - `let tasks = fetch_all_tasks(client).await;`
   - Filter to `Aria2TaskStatus::Paused`.
   - Group into `Vec<(Option<String>, Vec<aria2_ws::response::Status>)>`, preserving aria2-returned order **within** each group (deterministic stagger order). Clone `Status` values (it is `Clone`).
   - `const RESUME_GROUP_INTERVAL: Duration = Duration::from_millis(500);`
   - For each group, `tokio::spawn` a task with cloned `client` and `event_tx`:
     - `for (i, s) in group.iter().enumerate()`:
       - `if i > 0 { tokio::time::sleep(RESUME_GROUP_INTERVAL).await; }`
       - `client.unpause(&s.gid).await` (ignore result)
       - `if let Ok(st) = client.tell_status(&s.gid).await { emit_progress(&event_tx, &st).await; }` (fresh status keeps UI accurate; mirrors existing `EngineCmd::Resume` pattern at engine.rs:483)
   - Log number of groups and per-group counts via `tracing::info!`.
   - Remove the old trailing `fetch_all_tasks` progress emission.
   - `Client` and `EventTx` are both `Clone`, so spawned tasks are `'static`-safe.

3. Grouping key decision: **host only** (lowercased), e.g. `https://a.example.com/x` and `https://a.example.com/y` share a group; different hosts (`example.com` vs `cdn.example.net`) resume in parallel. Torrent/no-URL tasks fall into a single `None` group and are also staggered at 500 ms (uniform cadence; harmless since the 500 ms interval is trivial).

## Edge cases

- No paused tasks → no groups spawned → no-op (same as today's `unpause_all` on an empty set).
- Double-click "全部恢复" → second run finds no paused tasks (already waiting) → no-op; no guard needed.
- Engine restart/shutdown during an in-flight stagger → unpause/tell_status RPCs fail silently (results ignored). Accepted.
- Paused-state race: `StartAll` clears `paused_gids` before the engine runs (app.rs:508), so the app-side forced `Paused` override (app.rs:888) won't fight the engine. Fresh `tell_status` after each unpause keeps UI in sync.

## Out of scope

- `ResumeTask` (single), `PauseAll`, add-download flows — unchanged.
- Making the 500 ms interval configurable.
- aria2 global `--max-concurrent-downloads` limiting — unchanged (engine handles that separately).

## Validation

- `cargo build`
- `cargo clippy --workspace` (no warnings allowed)
- `cargo fmt --check`
- Manual: add several URLs from the same host, pause all, click 全部恢复 → tasks on the same host start ~500 ms apart; tasks from different hosts start concurrently.
