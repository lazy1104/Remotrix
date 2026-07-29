# Confirmation Dialogs for Destructive Actions & Unapplied Settings

## Goal
Add modal confirmation dialogs (following the existing stack-overlay dialog pattern) for:
1. Toolbar **Delete All** (`Message::DeleteAll`)
2. Toolbar **Clear List** (`Message::ClearCompleted`)
3. Task card **Remove** (`Message::RemoveTask(gid)`)
4. **Unapplied settings** — when the user navigates from Settings → Tasks while Download/BitTorrent/Network settings have changed but not been applied.

## Decisions (confirmed with user)
- Settings dialog offers **Apply / Discard / Cancel**:
  - Apply = apply now (`ApplySettings` logic) then navigate.
  - Discard = revert in-memory settings to last-applied snapshot then navigate.
  - Cancel = stay on Settings.
- Trigger for the settings dialog: **navigating to Tasks only** (sidebar). The existing app-close flow is untouched.

## Existing patterns to follow
- Dialogs are stack overlays: a boolean/`Option` flag on `Remotrix`, rendered in `app.rs::view()` via `stack![base, dialog_view]` with `theme::style::overlay`. Examples: `close_dialog.rs`, `about_dialog.rs`.
- Button styles available in `theme::style::button`: `primary()`, `secondary()`, `danger()`, `text()`.
- i18n via `Tr` enum in `src/i18n.rs` + `key()` match, with entries in both `i18n/locales/en/main.ftl` and `i18n/locales/zh-CN/main.ftl`.

## Implementation steps

### 1. New types in `src/message.rs`
- Add enum:
  ```rust
  #[derive(Debug, Clone)]
  pub enum ConfirmAction {
      DeleteAll,
      ClearCompleted,
      RemoveTask(String),
      LeaveSettings { target: Page },
  }
  ```
- Add `Message` variants:
  - `RequestConfirm(ConfirmAction)` — opens the dialog, stores pending action.
  - `ConfirmCancel` — closes the dialog, no action.
  - `ApplyAndLeaveSettings` — apply + navigate to pending target.
  - `DiscardAndLeaveSettings` — revert + navigate to pending target.

### 2. New reusable dialog `src/ui/confirm_dialog.rs`
- `pub fn view<'a>(fluent: &'a Fluent, theme: &iced::Theme, action: &'a ConfirmAction) -> Element<'a, Message>`
- Match on `action` to build title/body/buttons (same overlay + centered card layout as `close_dialog.rs`, width ~420):
  - `DeleteAll` → title `ConfirmDeleteAllTitle`, body `ConfirmDeleteAllBody`; buttons: Cancel(`ConfirmCancel`, secondary) | Confirm(`Message::DeleteAll`, danger).
  - `ClearCompleted` → `ConfirmClearTitle`/`ConfirmClearBody`; Cancel | Confirm(`Message::ClearCompleted`, danger).
  - `RemoveTask(gid)` → `ConfirmRemoveTitle`/`ConfirmRemoveBody`; Cancel | Confirm(`Message::RemoveTask(gid.clone())`, danger).
  - `LeaveSettings { .. }` → `ConfirmUnappliedTitle`/`ConfirmUnappliedBody`; buttons: Cancel(`ConfirmCancel`, secondary) | Discard(`DiscardAndLeaveSettings`, danger) | Apply(`ApplyAndLeaveSettings`, primary).
- Reuse the real action messages directly (no logic duplication): the Confirm button emits the existing `DeleteAll`/`ClearCompleted`/`RemoveTask` message.
- Register module in `src/ui/mod.rs`: `pub mod confirm_dialog;`.

### 3. State additions in `src/app.rs` (`Remotrix` struct)
- `confirm: Option<ConfirmAction>` — pending dialog action (`None` = closed).
- `settings_dirty: bool` — true when apply-required settings changed but not applied.
- `applied_settings: Settings` — snapshot of last-applied settings (used only by Discard to revert).
- Initialize in `init()`: `confirm: None`, `settings_dirty: false`, `applied_settings: settings.clone()`.

### 4. Dirty tracking in `src/app.rs` `update()`
- At the top of the `Message::SettingChanged(key, value)` arm: if `key == SettingKey::DownloadDir`, `return pick_folder(...)` early (it opens the folder picker and does not mutate `settings.download_dir`, so it must NOT set dirty). Otherwise set `state.settings_dirty = true;` before the existing `match key`.
- In `Message::UaEditor(action)` and `Message::HeadersEditor(action)` arms: set `state.settings_dirty = true;`.
- In `Message::ApplySettings` arm (after `config::save` + sending `ApplyAria2Options`): set `state.applied_settings = state.settings.clone();` and `state.settings_dirty = false;`.

### 5. Navigation interception in `src/app.rs` `update()`
- Replace the `Message::NavigatePage(page)` arm with:
  ```rust
  Message::NavigatePage(page) => {
      if page == Page::Tasks && state.page == Page::Settings && state.settings_dirty {
          state.confirm = Some(ConfirmAction::LeaveSettings { target: page });
      } else {
          state.page = page;
      }
  }
  ```
- (Category switches use `SetSettingsCategory`, not `NavigatePage`, and do not lose in-memory changes, so no interception there.)

### 6. New message handlers in `src/app.rs` `update()`
- `Message::ConfirmCancel` → `state.confirm = None;`
- `Message::ApplyAndLeaveSettings` →
  ```rust
  if let Some(ConfirmAction::LeaveSettings { target }) = state.confirm.take() {
      config::save(&state.settings);
      let opts = state.settings.to_aria2_task_options();
      let _ = state.handle.cmd_tx.send(EngineCmd::ApplyAria2Options { options: opts });
      state.applied_settings = state.settings.clone();
      state.settings_dirty = false;
      state.page = target;
  }
  ```
- `Message::DiscardAndLeaveSettings` →
  ```rust
  if let Some(ConfirmAction::LeaveSettings { target }) = state.confirm.take() {
      revert_apply_settings(state);
      state.page = target;
  }
  ```
- Add private helper `fn revert_apply_settings(state: &mut Remotrix)` that copies only the apply-relevant fields from `state.applied_settings` into `state.settings` (do NOT touch `theme_mode`/`light_theme`/`dark_theme`/`locale`/`update`/`download_dir`/window geometry — those are managed separately and saved immediately):
  - `max_concurrent`, `download_limit_kb`, `upload_limit_kb`, `split`, `nav_to_tasks_after_add`, `delete_torrent_after_complete`, and the entire `aria2` (`Aria2Options`) clone.
  - Re-sync the editors from the reverted values:
    `state.ua_editor = text_editor::Content::with_text(&state.settings.aria2.user_agent);`
    `state.headers_editor = text_editor::Content::with_text(&state.settings.aria2.headers.join("\n"));`
  - Set `state.settings_dirty = false;`.
- In the existing `Message::DeleteAll`, `Message::ClearCompleted`, and `Message::RemoveTask(gid)` arms: append `state.confirm = None;` at the end so the dialog closes once the action executes.

### 7. Render the dialog in `src/app.rs` `view()`
- After the existing dialog `stack!` blocks (alongside `show_close_dialog`, `details`), add:
  ```rust
  if let Some(ref action) = state.confirm {
      stacked = stack![stacked, crate::ui::confirm_dialog::view(&state.fluent, t, action)]
          .width(Length::Fill).height(Length::Fill).into();
  }
  ```

### 8. Wire toolbar/card buttons to `RequestConfirm` in `src/ui/task_list.rs`
- Import `ConfirmAction` from `crate::message`.
- Toolbar **Delete All** button (≈ line 135): `Message::DeleteAll` → `Message::RequestConfirm(ConfirmAction::DeleteAll)`.
- Toolbar **Clear List** button (≈ line 141): `Message::ClearCompleted` → `Message::RequestConfirm(ConfirmAction::ClearCompleted)`.
- Task card **Remove** button (≈ line 284): `Message::RemoveTask(t.gid.clone())` → `Message::RequestConfirm(ConfirmAction::RemoveTask(t.gid.clone()))`.

### 9. i18n — add `Tr` variants + locale entries
- In `src/i18n.rs` `Tr` enum and `key()` match, add: `ConfirmDeleteAllTitle`, `ConfirmDeleteAllBody`, `ConfirmClearTitle`, `ConfirmClearBody`, `ConfirmRemoveTitle`, `ConfirmRemoveBody`, `ConfirmUnappliedTitle`, `ConfirmUnappliedBody`, `Discard`. (Reuse existing `Apply` and `Cancel`.)
- `i18n/locales/en/main.ftl`:
  - `confirm-delete-all-title = Delete all tasks?`
  - `confirm-delete-all-body = This will remove all download tasks. This action cannot be undone.`
  - `confirm-clear-title = Clear completed tasks?`
  - `confirm-clear-body = This will remove completed and removed tasks from the list.`
  - `confirm-remove-title = Remove this task?`
  - `confirm-remove-body = This will remove the task from the list. This action cannot be undone.`
  - `confirm-unapplied-title = Apply settings changes?`
  - `confirm-unapplied-body = You have settings changes that have not been applied.`
  - `discard = Discard`
- `i18n/locales/zh-CN/main.ftl`:
  - `confirm-delete-all-title = 删除全部任务？`
  - `confirm-delete-all-body = 将移除所有下载任务，此操作不可撤销。`
  - `confirm-clear-title = 清空列表？`
  - `confirm-clear-body = 将从列表中清除已完成和已移除的任务。`
  - `confirm-remove-title = 移除该任务？`
  - `confirm-remove-body = 将从列表中移除该任务，此操作不可撤销。`
  - `confirm-unapplied-title = 应用设置更改？`
  - `confirm-unapplied-body = 您有尚未应用的设置更改。`
  - `discard = 放弃`

## Known limitations / out of scope
- `settings_dirty` is a coarse boolean: if a user changes a value and then changes it back to the original, the dialog still fires (no field-level diff). Accepted trade-off vs. the complexity/geometry false-positives of full-snapshot comparison.
- Pre-existing bug (not introduced here, not fixed): the Settings → "Download folder" Browse button sends `SettingChanged(DownloadDir, "")` which opens the folder picker, but `FilePicked(FileKind::SaveDir, ..)` writes to `add_dialog.save_dir` instead of `settings.download_dir`. So the download folder setting never actually changes via the UI. Because of this, `DownloadDir` is explicitly excluded from dirty tracking to avoid spurious prompts. Fixing the picker routing is out of scope.
- App close with unapplied settings is out of scope (existing close dialog remains; it already calls `config::save` on close).

## Validation
- `cargo build` (offline, no network).
- `cargo clippy --workspace` (must be warning-free).
- `cargo fmt --check`.
- Manual scenarios:
  1. Click Delete All / Clear List / Remove → dialog appears; Cancel does nothing; Confirm executes and dialog closes.
  2. Change a Download/BitTorrent/Network setting, click Tasks in sidebar → Apply/Discard/Cancel dialog appears.
  3. Apply → settings applied + navigates to Tasks + dirty cleared (subsequent nav has no dialog).
  4. Discard → editors/fields revert to last-applied + navigates to Tasks.
  5. Cancel → stays on Settings, dialog closes, changes retained.
  6. Change General theme/locale (saved immediately) → navigating to Tasks does NOT trigger the dialog.
  7. Only one confirm dialog is shown at a time; overlay blocks background interaction.
