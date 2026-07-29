# Plan: Fix "delete files" not working + task re-sync after removal

## Context / Root cause (confirmed from logs)

Two bugs reported after the "移除 -> 删除" feature was implemented:

1. **Files not actually deleted** — `tell_status` returns `"path":""` (empty) for tasks that never connected in the current session (paused immediately). `delete_task_files` skips empty paths, so leftover files from previous sessions are never targeted. Log proof (2026-07-28): removed tasks had `"path":"","completedLength":"0","totalLength":"0"`.
2. **Removed tasks re-sync after restart** — `remove()` clears the task from aria2 memory, but `session.txt` (configured via `--save-session` / `--save-session-interval=60`) is NOT rewritten until the 60s periodic save or a SIGTERM. The RPC `shutdown()` used on app exit does not trigger a session save. Log proof (2026-07-29): delete at `07:44:22`, shutdown at `07:44:34`, restart at `07:44:36` → aria2 logged `"Downloading 2 item(s)"` → `"synced 2 existing tasks"` → the deleted task (and its file, re-downloaded, mtime `10:04`) came back.

The aria2-ws crate exposes `client.save_session()` (`aria2.saveSession`) which forces an immediate session rewrite excluding removed tasks. This is the fix for re-sync.

User decision: for incomplete tasks, **also delete the partial file** (not just completed tasks). This requires robust path construction when `files[].path` is empty.

## Scope
- **Only `src/engine.rs` changes.** No UI / message / i18n / app.rs changes needed — the `delete_files: bool` flag, confirm-dialog buttons, and `DeleteTask`/`RemoveTask`/`DeleteAll`/`RemoveAllRecords` messages are already correct from the prior implementation.
- `ClearCompleted` is unchanged (local-only, no aria2 call).

## File-by-file changes

### `src/engine.rs`

#### 1. New helper `collect_file_paths` (place near `name_from_status` / `basename`)
Reuses the existing `basename(uri: &str) -> Option<String>` fn. For each file: use `file.path` if non-empty; otherwise construct `dir` + `basename(file.uris[0].uri)` (covers never-connected HTTP tasks whose path is empty but whose leftover file exists on disk).
```rust
fn collect_file_paths(s: &aria2_ws::response::Status) -> Vec<String> {
    let mut paths = Vec::new();
    for f in &s.files {
        if !f.path.is_empty() {
            paths.push(f.path.clone());
        } else if let Some(uri) = f.uris.first() {
            if let Some(name) = basename(&uri.uri) {
                paths.push(
                    std::path::Path::new(&s.dir)
                        .join(name)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    paths
}
```

#### 2. `EngineCmd::Remove { gid, delete_files }` handler
Order: capture status (BEFORE removal) → collect paths → remove from aria2 → **`save_session()`** → delete files → emit `Removed`. `save_session` BEFORE file deletion so that even if deletion fails the task won't re-sync/re-download.
```rust
EngineCmd::Remove { gid, delete_files } => {
    tracing::info!(?gid, delete_files, "remove");
    let paths = client
        .tell_status(&gid)
        .await
        .ok()
        .map(|s| collect_file_paths(&s))
        .unwrap_or_default();
    remove_task_from_aria2(client, &gid).await;
    let _ = client.save_session().await;
    if delete_files {
        delete_task_files(&paths).await;
    }
    let _ = event_tx.send(EngineEvent::Removed(gid));
}
```

#### 3. `EngineCmd::RemoveAll { delete_files }` handler
Capture all statuses first (via existing `fetch_all_tasks`, which already returns `Vec<Status>` with `files`), remove each + emit `Removed`, then **`save_session()` once**, then delete files (so a crash mid-loop can't leave files deleted but session stale → re-download).
```rust
EngineCmd::RemoveAll { delete_files } => {
    tracing::info!(delete_files, "remove all");
    let tasks = fetch_all_tasks(client).await;
    for s in &tasks {
        remove_task_from_aria2(client, &s.gid).await;
        let _ = event_tx.send(EngineEvent::Removed(s.gid.clone()));
    }
    let _ = client.save_session().await;
    if delete_files {
        for s in &tasks {
            delete_task_files(&collect_file_paths(s)).await;
        }
    }
}
```

#### 4. `delete_task_files` — keep current logic, add a `debug` log line per attempted path (for log-based verification)
```rust
async fn delete_task_files(paths: &[String]) {
    for p in paths {
        if p.is_empty() {
            continue;
        }
        tracing::debug!(path = %p, "deleting file");
        if let Err(e) = tokio::fs::remove_file(std::path::Path::new(p)).await {
            tracing::debug!(path = %p, error = %e, "delete file skipped");
        }
        let _ = tokio::fs::remove_file(std::path::Path::new(&format!("{p}.aria2"))).await;
    }
}
```
(`remove_task_from_aria2` unchanged — already does `remove` → `force_remove` → `remove_download_result`.)

## Why this fixes both bugs
- **Re-sync fixed:** `save_session()` rewrites `session.txt` immediately after removal, so removed tasks (active/waiting/stopped) are excluded from the session. On restart `--input-file session.txt` no longer re-adds them; `sync_existing_tasks` finds nothing. Confirmed by aria2 semantics: `saveSession` writes only current active+waiting downloads.
- **File deletion fixed:** for never-connected tasks with empty `files[].path`, `collect_file_paths` constructs `dir + basename(uri)` so leftover partial files (and their `.aria2` control files) are targeted. For completed/connected tasks the real `path` is used as before. With re-sync fixed, deleted files are no longer re-downloaded.

## Risks / edge cases
- **Constructed path mismatch:** if the on-disk filename differs from the URL basename (Content-Disposition rename, `auto-file-renaming` suffix), the constructed path won't match and deletion is silently skipped (best-effort, logged at debug). Acceptable — completed/connected tasks always have the real `path`.
- **`save_session` failure:** logged and ignored; task is still gone from aria2 memory, but could re-sync on restart. Best-effort, consistent with existing error handling.
- **Multi-file torrents:** `files[].path` is always populated from torrent metadata, so construction fallback never triggers; each file deleted individually, parent dirs never touched.
- **Engine degraded (aria2 down):** `Remove`/`RemoveAll` aren't dispatched to `handle_client_cmd` (supervisor emits `EngineDegraded`); `save_session` never called. `DeleteAll`/`RemoveAllRecords` still clear local state (existing graceful-degrade behavior).
- **`RemoveAll` partial crash:** if the engine crashes mid-removal-loop before `save_session`, un-removed tasks re-sync on restart (acceptable); files are only deleted after `save_session`, so no file is deleted for a task that might re-sync.

## Out of scope
- `ClearCompleted` still leaves completed tasks in aria2's stopped queue (may re-sync on restart via `tell_stopped`) — pre-existing, not addressed.
- Paused/waiting tasks not polled (DB shows `downloaded=0`) — pre-existing, not addressed.
- `AGENTS.md` `EngineCmd` protocol block is stale (`Remove(String)`/`RemoveAll` instead of the `{ gid, delete_files }` / `{ delete_files }` forms) — drifted during the prior implementation; optional doc update, not required for this fix.

## Validation
1. `cargo fmt --check`
2. `cargo clippy --workspace` (warning-free)
3. `cargo build` (offline)
4. Manual `cargo run --`:
   - Complete an HTTP download → **删除文件**: file gone from disk; restart engine/app → task does NOT reappear (session excludes it).
   - Complete an HTTP download → **移除记录**: file remains on disk; restart → task does NOT reappear.
   - Add a download, **pause** it before completion → **删除文件**: partial file AND `.aria2` control file gone; restart → task does NOT reappear.
   - Never-started task (paused immediately, empty `path`) with a leftover file from a prior session → **删除文件**: leftover file deleted (via constructed `dir+basename` path); restart → not re-synced.
   - 2–3 tasks (mix complete + incomplete) → **全部删除** → **删除全部文件**: all files + `.aria2` gone; restart → none reappear.
   - 2–3 tasks → **全部删除** → **移除全部记录**: files retained; restart → none reappear.
   - **清空列表**: unchanged (no aria2 call, no file deletion).
5. Grep logs to confirm: `saveSession` WS call after each remove; `deleting file` debug lines with the constructed/real paths.
