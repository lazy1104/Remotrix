# Missing-File Cleanup on Open + Deletion/Completion Toasts

## Goal

When the user clicks "Open File" on a task whose downloaded file no longer exists:

- If `remove_task_if_files_missing` is enabled → delete the task immediately and show a toast.
- Otherwise → show a confirm dialog (reminder that the task can be re-downloaded) asking whether to remove the task.

Additionally, add toast feedback (currently missing) for:
- Task deletion (`RemoveTask`, `DeleteTask`, `RemoveAllRecords`, `DeleteAll`)
- Download completion

## Current Behavior (src/app.rs)

- `Message::OpenTaskFile(gid)` (line 2253): handles torrent/metadata/BT re-add cases first, then opens the file via `open::that`. If `path` does not exist, it just shows a `Tr::FileMissing` warning toast (lines 2306–2314).
- `Message::RemoveTask(gid)` (859): sends `EngineCmd::Remove { delete_files: false }`, no toast.
- `Message::DeleteTask(gid)` (874): sends `EngineCmd::Remove { delete_files: true }`, no toast.
- `Message::RemoveAllRecords` (919) / `Message::DeleteAll` (907): no toasts.
- `EngineEvent::Progress` (1471): no toast on transition to complete. Completion is detectable here; the engine emits a "complete" progress event only once per task (via `stopped_seen` in engine.rs slow scan), so a transition guard is reliable.

Existing infra to reuse:
- `spawn_toast(state, group, kind, message, close_after, show_close)` — app.rs:2963
- `ToastGroup::Task`, `ToastKind::{Normal, Success}`
- `Tr::FilesMissingRemoved` ("已删除文件缺失的任务" / "Removed tasks with missing files")
- `Tr::FileMissing` ("下载文件不存在" / "The downloaded file is missing") — keep, still used for non-completed tasks
- `state.confirm: Option<ConfirmAction>` + `src/ui/confirm_dialog.rs` (exhaustive match on `ConfirmAction`)
- `fluent.get_args(Tr::X, &args)` for `{ $name }` interpolation (app.rs:2017)

## Design Decisions

1. **Missing-file cleanup applies only to `TaskStatus::Completed` tasks.** For active/waiting/paused tasks the file legitimately does not exist yet — keep the existing `FileMissing` warning toast. Otherwise clicking Open would delete in-progress tasks.
2. **Auto-delete path** (cleanup enabled): send `EngineCmd::Remove { gid, delete_files: false }` directly (files are already gone; `remove_task_from_aria2` in engine.rs also purges the aria2 download result so the task won't resync), then toast `Tr::FilesMissingRemoved`. Local removal happens when `EngineEvent::Removed(gid)` arrives (app.rs:1597).
3. **Dialog path** (cleanup disabled): set `state.confirm = Some(ConfirmAction::RemoveMissingFileTask(gid))`. Dialog has only Cancel + "Remove from List" (`Tr::RemoveRecord`); deleting files is pointless since the file is gone.
4. **Deletion toasts** fire directly in the message handlers (user-action-driven, no spurious toasts from orphan detection / `FilesMissing` engine flow which already toasts).
5. **Completion toast**: detect transition inside the `EngineEvent::Progress` arm. Compute `was_completed` before the `get_mut` update; after the torrent-follow block (end of arm), if `status == "complete" && state.sync_done && !was_completed` and the task is now `Completed`, toast `Tr::DownloadComplete` with the task name. Guarded by `state.sync_done` (reset on engine restart at app.rs:1252) so startup re-sync does not spam toasts; `was_completed` prevents duplicates from Refresh/snapshot events. Place it *after* the torrent-follow block so that block still runs.

## Implementation Tasks

### 1. `src/message.rs`
Add variant to `ConfirmAction`:
```rust
RemoveMissingFileTask(String),
```

### 2. `src/i18n.rs`
Add to `Tr` enum (near `FilesMissingRemoved` / `FileMissing`):
```rust
TaskRemoved,
TaskDeleted,
TasksRemoved,
TasksDeleted,
DownloadComplete,
ConfirmMissingFileTitle,
ConfirmMissingFileBody,
```
Add matching entries in `fn key()`:
```rust
Tr::TaskRemoved => "task-removed",
Tr::TaskDeleted => "task-deleted",
Tr::TasksRemoved => "tasks-removed",
Tr::TasksDeleted => "tasks-deleted",
Tr::DownloadComplete => "download-complete",
Tr::ConfirmMissingFileTitle => "confirm-missing-file-title",
Tr::ConfirmMissingFileBody => "confirm-missing-file-body",
```

### 3. `i18n/locales/en/main.ftl` and `i18n/locales/zh-CN/main.ftl`
Add (both files, matching line ~105 area for task toasts and ~205 area for dialogs):
- en:
  - `task-removed = Task removed from list`
  - `task-deleted = Task and its files deleted`
  - `tasks-removed = All tasks removed from list`
  - `tasks-deleted = All tasks and files deleted`
  - `download-complete = Download complete: { $name }`
  - `confirm-missing-file-title = Remove this task record?`
  - `confirm-missing-file-body = The local file has been deleted. Remove the task record?`
- zh-CN:
  - `task-removed = 任务已从列表移除`
  - `task-deleted = 任务及其文件已删除`
  - `tasks-removed = 所有任务已从列表移除`
  - `tasks-deleted = 所有任务及其文件已删除`
  - `download-complete = 下载完成：{ $name }`
  - `confirm-missing-file-title = 移除任务记录？`
  - `confirm-missing-file-body = 本地文件已被删除，是否移除任务记录？`

### 4. `src/ui/confirm_dialog.rs`
In the `(title_key, body_key)` match (line 15) add:
```rust
ConfirmAction::RemoveMissingFileTask(_) => (Tr::ConfirmMissingFileTitle, Tr::ConfirmMissingFileBody),
```
In the `buttons` match add an arm rendering `cancel_btn` + a "Remove from List" button:
```rust
ConfirmAction::RemoveMissingFileTask(gid) => {
    let remove_btn = button(text(fluent.get(Tr::RemoveRecord)).size(FONT_BODY))
        .on_press(Message::RemoveTask(gid.clone()))
        .padding(PADDING_BUTTON_LG)
        .style(theme::style::button::secondary());
    row![cancel_btn, remove_btn]
        .spacing(SPACE_XL)
        .align_y(Alignment::Center)
        .into()
}
```

### 5. `src/app.rs`
- **`Message::OpenTaskFile(gid)`** — replace the final missing-path branch (lines 2298–2314):
  ```rust
  if path.exists() {
      return Task::perform(
          async move { let _ = open::that(&path); },
          |_| Message::Noop,
      );
  }
  if t.status == TaskStatus::Completed {
      if state.settings.remove_task_if_files_missing {
          state.paused_gids.remove(&gid);
          let _ = state.handle.cmd_tx.send(EngineCmd::Remove {
              gid: gid.clone(),
              delete_files: false,
          });
          let (_, task) = spawn_toast(
              state,
              ToastGroup::Task,
              ToastKind::Normal,
              state.fluent.get(Tr::FilesMissingRemoved),
              Some(Duration::from_secs(3)),
              false,
          );
          return task;
      }
      state.confirm = Some(ConfirmAction::RemoveMissingFileTask(gid));
      return Task::none();
  }
  let (_, task) = spawn_toast(
      state,
      ToastGroup::Task,
      ToastKind::Warning,
      state.fluent.get(Tr::FileMissing),
      Some(Duration::from_secs(4)),
      false,
  );
  return task;
  ```
- **`Message::RemoveTask(gid)`** — after sending the command, spawn toast `Tr::TaskRemoved` (Normal, ~3s).
- **`Message::DeleteTask(gid)`** — after sending the command, spawn toast `Tr::TaskDeleted` (Normal, ~3s).
- **`Message::RemoveAllRecords`** — after `clear_all_local`, spawn toast `Tr::TasksRemoved` (Normal, ~3s).
- **`Message::DeleteAll`** — after `clear_all_local`, spawn toast `Tr::TasksDeleted` (Normal, ~3s).
- **`EngineEvent::Progress` arm**:
  - At the top of the arm, before mutation: `let was_completed = state.tasks.get(&gid).map(|t| t.status == TaskStatus::Completed).unwrap_or(false);`
  - After the torrent-follow block closes (end of the arm, after line 1595), add:
    ```rust
    if status == "complete" && state.sync_done && !was_completed {
        if let Some(t) = state.tasks.get(&gid) {
            if t.status == TaskStatus::Completed {
                let mut args = std::collections::HashMap::new();
                args.insert(std::borrow::Cow::from("name"), std::borrow::Cow::from(t.name.clone()).into());
                let (_, task) = spawn_toast(
                    state,
                    ToastGroup::Task,
                    ToastKind::Success,
                    state.fluent.get_args(Tr::DownloadComplete, &args),
                    Some(Duration::from_secs(4)),
                    false,
                );
                return task;
            }
        }
    }
    ```
    (Mirror the `args` pattern already used at app.rs:2016.)

## Notes / Edge Cases
- `push_toast` (app.rs:2937) replaces the previous auto-close toast in the same group, so simultaneous task toasts won't stack — acceptable.
- The auto-delete path relies on `EngineEvent::Removed(gid)` → `remove_task_local`; `gid_recently_removed` prevents resync.
- `Tr::FileMissing` remains used (non-completed tasks), so it is not dead code.
- No changes needed in `config.rs`, `settings_page.rs`, or `engine.rs` (setting and `EngineCmd::Remove` already exist).

## Validation
1. `cargo build`
2. `cargo clippy --workspace` (no warnings allowed)
3. `cargo fmt --check`
4. Manual:
   - Complete a download → one "下载完成" success toast.
   - Delete a task via confirm dialog (both "Remove from List" and "Delete Files") → toast appears; task removed from list.
   - Delete All / Remove All from List → batch toast.
   - Completed task whose file was deleted externally, click Open:
     - cleanup enabled → task removed + `FilesMissingRemoved` toast.
     - cleanup disabled → confirm dialog; Cancel keeps task, Remove removes it (with `TaskRemoved` toast).
   - Click Open on an active/paused/incomplete task → existing "file missing" warning toast, task NOT deleted.
   - Restart app with existing completed tasks → no spurious completion toasts.
