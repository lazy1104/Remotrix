# Plan: "移除" → "删除" with file deletion + aria2-next notification

## Goal
1. Rename the per-task **"移除" (Remove)** button to **"删除" (Delete)**.
2. When clicking **删除** (per task) or **全部删除** (Delete All), show a confirm dialog that offers a **choice**:
   - **删除文件 / 删除全部文件** → remove task(s) from aria2-next **AND delete the downloaded file(s) on disk**.
   - **移除记录 / 移除全部记录** → remove task(s) from aria2-next, keep files (record-only).
3. "清空列表" (Clear List) is **out of scope / unchanged** (record-only cleanup of completed/removed, no aria2 call, no file deletion).

## Key decisions
- Both the per-task delete dialog and the Delete-All dialog get **two action buttons** + Cancel (per user choice).
- File deletion happens in the **engine layer** (`engine.rs`), which is the only place with async aria2 access and the source of truth for file paths (`aria2_ws::response::Status.files[].path`). The app's `DownloadTask` only stores `save_dir`, not individual file paths — so the engine fetches paths via `tell_status` / `fetch_all_tasks`.
- `EngineCmd::Remove` / `EngineCmd::RemoveAll` gain a `delete_files: bool` flag (justified: true = delete-files choice, false = record-only choice).
- **Order of operations** for safe file deletion: fetch file paths → remove task from aria2 (releases file handles) → delete files → emit `Removed`.
- aria2 removal is made status-robust: try `remove` → `force_remove` → `remove_download_result` (the last clears stopped/completed/error tasks from aria2's stopped queue, which the current code does NOT do — pre-existing gap fixed as a side effect).
- `DeleteAll`/`RemoveAllRecords` now remove **all** tasks (active + waiting + stopped) from aria2, not just active (current `RemoveAll` only did `tell_active()`). This is required so "remove all" is persistent across engine restarts.
- **Out of scope / deliberate keep**: `Tr::Removed` status label ("已移除"/"Removed") in `details_dialog.rs` stays — it represents aria2's internal "removed" task state, distinct from the user delete action.

## File-by-file changes

### 1. `src/message.rs`
- Rename `ConfirmAction::RemoveTask(String)` → `ConfirmAction::DeleteTask(String)`.
- Keep `Message::RemoveTask(String)` (now = per-task **record-only** path, `delete_files: false`).
- Add `Message::DeleteTask(String)` (per-task **delete-files** path, `delete_files: true`), placed right after `Message::RemoveTask`.
- Add `Message::RemoveAllRecords` (delete-all **record-only** path), placed right after `Message::DeleteAll`.

### 2. `src/engine.rs`
- Change enum variants:
  - `EngineCmd::Remove(String)` → `EngineCmd::Remove { gid: String, delete_files: bool }`.
  - `EngineCmd::RemoveAll` → `EngineCmd::RemoveAll { delete_files: bool }`.
- Add helper (best-effort, all errors ignored/logged):
  ```rust
  async fn remove_task_from_aria2(client: &Client, gid: &str) {
      if client.remove(gid).await.is_err() {
          let _ = client.force_remove(gid).await;
      }
      let _ = client.remove_download_result(gid).await;
  }

  async fn delete_task_files(paths: &[String]) {
      for p in paths {
          if p.is_empty() { continue; }
          if let Err(e) = tokio::fs::remove_file(std::path::Path::new(p)).await {
              tracing::debug!(path = %p, error = %e, "delete file skipped");
          }
          let _ = tokio::fs::remove_file(std::path::Path::new(&format!("{p}.aria2"))).await;
      }
  }
  ```
- `handle_client_cmd` — `Remove { gid, delete_files }`:
  1. `let paths: Vec<String> = client.tell_status(&gid).await.ok().into_iter().flat_map(|s| s.files.into_iter().map(|f| f.path).filter(|p| !p.is_empty())).collect();`
  2. `remove_task_from_aria2(client, &gid).await;`
  3. `if delete_files { delete_task_files(&paths).await; }`
  4. `let _ = event_tx.send(EngineEvent::Removed(gid));`
- `handle_client_cmd` — `RemoveAll { delete_files }`:
  1. `for s in fetch_all_tasks(client).await {` (already returns Status with `files`)
  2. `   let paths: Vec<String> = s.files.iter().map(|f| f.path.clone()).filter(|p| !p.is_empty()).collect();`
  3. `   remove_task_from_aria2(client, &s.gid).await;`
  4. `   if delete_files { delete_task_files(&paths).await; }`
  5. `   let _ = event_tx.send(EngineEvent::Removed(s.gid.clone()));`
  6. `}`

### 3. `src/app.rs`
- `Message::RemoveTask(gid)` (record-only): change send to `EngineCmd::Remove { gid, delete_files: false }`; keep `state.confirm = None;`.
- Add `Message::DeleteTask(gid)`: send `EngineCmd::Remove { gid, delete_files: true }`; `state.confirm = None;`. (Local record cleanup still happens via the existing `EngineEvent::Removed` arm — do NOT clear locally here.)
- `Message::DeleteAll`: change send to `EngineCmd::RemoveAll { delete_files: true }`; keep the existing local clear (`tasks.clear()`, `task_order.clear()`, `dirty.clear()`, `db.delete_all()`) and `state.confirm = None;`.
- Add `Message::RemoveAllRecords`: send `EngineCmd::RemoveAll { delete_files: false }`; identical local clear + `db.delete_all()` + `state.confirm = None;`.
  - Optional: factor the shared "clear all local tasks + db" into a small `fn clear_all_local(state: &mut Remotrix)` to avoid duplication between `DeleteAll` and `RemoveAllRecords`.

### 4. `src/ui/confirm_dialog.rs`
Restructure `view` so each `ConfirmAction` builds its own button row (the single-`confirm_msg` model no longer fits the two-choice cases):
- `ConfirmAction::DeleteTask(gid)` → title `Tr::ConfirmDeleteTitle`, body `Tr::ConfirmDeleteBody`; buttons: **Cancel** (`ConfirmCancel`, secondary) | **移除记录** (`Message::RemoveTask(gid.clone())`, secondary) | **删除文件** (`Message::DeleteTask(gid.clone())`, danger).
- `ConfirmAction::DeleteAll` → title `Tr::ConfirmDeleteAllTitle`, body `Tr::ConfirmDeleteAllBody`; buttons: **Cancel** | **移除全部记录** (`Message::RemoveAllRecords`, secondary) | **删除全部文件** (`Message::DeleteAll`, danger).
- `ConfirmAction::ClearCompleted` → unchanged (Cancel | Confirm=`Message::ClearCompleted`, danger).
- `ConfirmAction::LeaveSettings { .. }` → unchanged (Cancel | Discard | Apply).
- Use `row![...].spacing(10).align_y(Alignment::Center)` for each button row; keep the existing panel container/overlay styling.

### 5. `src/ui/task_list.rs`
- Line ~284: `ConfirmAction::RemoveTask(t.gid.clone())` → `ConfirmAction::DeleteTask(t.gid.clone())`.
- Line ~289: `Tr::Remove` → `Tr::Delete` (the per-task button tooltip).

### 6. `src/i18n.rs`
- Rename variant `Tr::Remove` → `Tr::Delete`, key `"remove"` → `"delete"`.
- Rename `Tr::ConfirmRemoveTitle` → `Tr::ConfirmDeleteTitle`, key `"confirm-remove-title"` → `"confirm-delete-title"`.
- Rename `Tr::ConfirmRemoveBody` → `Tr::ConfirmDeleteBody`, key `"confirm-remove-body"` → `"confirm-delete-body"`.
- Add new variants + keys:
  - `Tr::DeleteFiles` → `"delete-files"`
  - `Tr::RemoveRecord` → `"remove-record"`
  - `Tr::DeleteAllFiles` → `"delete-all-files"`
  - `Tr::RemoveAllRecords` → `"remove-all-records"`
- Add the 4 new arms in `Tr::key()` and keep enum well-ordered.

### 7. `i18n/locales/zh-CN/main.ftl`
- Replace `remove = 移除` → `delete = 删除`.
- Replace `confirm-remove-title`/`confirm-remove-body` with:
  ```
  confirm-delete-title = 删除该任务？
  confirm-delete-body = 请选择操作：“删除文件”将移除该任务并删除已下载的文件；“移除记录”仅从列表中移除该任务，保留文件。
  ```
- Update `confirm-delete-all-body`:
  ```
  confirm-delete-all-body = 请选择操作：“删除全部文件”将移除所有任务并删除对应的下载文件；“移除全部记录”仅从列表中移除所有任务，保留文件。
  ```
- Add:
  ```
  delete-files = 删除文件
  remove-record = 移除记录
  delete-all-files = 删除全部文件
  remove-all-records = 移除全部记录
  ```

### 8. `i18n/locales/en/main.ftl`
- Replace `remove = Remove` → `delete = Delete`.
- Replace `confirm-remove-title`/`confirm-remove-body` with:
  ```
  confirm-delete-title = Delete this task?
  confirm-delete-body = Choose an action: "Delete Files" removes the task and deletes its downloaded file(s); "Remove from List" only removes the task from the list, keeping the file(s).
  ```
- Update `confirm-delete-all-body`:
  ```
  confirm-delete-all-body = Choose an action: "Delete All Files" removes all tasks and deletes their downloaded files; "Remove All from List" only removes all tasks from the list, keeping the files.
  ```
- Add:
  ```
  delete-files = Delete Files
  remove-record = Remove from List
  delete-all-files = Delete All Files
  remove-all-records = Remove All from List
  ```

## Risks / edge cases
- **Empty file path**: when a task hasn't started writing, `Status.files[].path` is empty → skip deletion for that path; still remove from aria2 + record. Safe.
- **Multi-file torrents**: `files` has multiple absolute paths; each is deleted individually. Do NOT delete parent directories (could contain user files).
- **`.aria2` control files**: best-effort delete of `<path>.aria2` per file; for multi-file tasks the single task-level control file may be left behind (acceptable for v1).
- **Windows file locking**: after `remove`/`force_remove`, aria2 should release handles, but deletion can still race; failures are logged at `debug` and ignored (graceful).
- **Engine degraded (aria2 unavailable)**: `tell_status`/`fetch_all_tasks` fail → no file paths → file deletion skipped; `Message::DeleteAll`/`RemoveAllRecords` still clear local records (matches existing graceful-degrade behavior). Per-task `DeleteTask`/`RemoveTask` rely on the `Removed` event; if aria2 is down no `Removed` fires, so the local record stays until aria2 recovers — acceptable (consistent with current behavior).
- **`RemoveAll` scope change**: now removes active+waiting+stopped (was active-only). Intended for persistence; the app already clears its whole local map, and `EngineEvent::Removed` handlers are idempotent (no-op on already-cleared entries).

## Validation
1. `cargo fmt --check`
2. `cargo clippy --workspace` (must be warning-free)
3. `cargo build` (offline; no network at build time)
4. Manual run `cargo run --`:
   - Add an HTTP download; while active click **删除** → dialog shows **删除文件** / **移除记录** / Cancel.
     - **删除文件**: task disappears from list; verify the downloaded file is gone from disk; verify aria2 no longer lists it (restart engine → task not re-added).
     - **移除记录**: task disappears from list; verify the file still exists on disk; verify aria2 no longer lists it.
   - Add 2–3 tasks; click **全部删除** → dialog shows **删除全部文件** / **移除全部记录** / Cancel.
     - **删除全部文件**: all tasks gone, all files deleted.
     - **移除全部记录**: all tasks gone, files retained.
   - **清空列表**: unchanged behavior (clears completed/removed records only, no file deletion, no aria2 call).
   - Test on a completed task: **删除文件** deletes the completed file; **移除记录** keeps it.
   - Test a multi-file torrent task: **删除文件** removes all files under the task (not the parent folder).
