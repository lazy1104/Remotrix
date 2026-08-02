# Task Card: Re-download for Completed, Tooltips, Icon Spacing

## Goal
1. For `Completed` tasks, replace the pause/resume button with a **re-download** button (原地替换: same gid, existing file kept/renamed).
2. Add tooltips to all card icons currently missing them.
3. Slightly increase the spacing between the task-card toolbar icons.

## Background / Decisions
- `DownloadTask` stores `url`, `save_dir`, `info_hash` but **no split** — reuse `state.settings.aria2.split` (config default 16) for re-download.
- Re-download keeps the same gid: `aria2.removeDownloadResult(gid)` first (frees the gid), then `add_uri` with `gid` option, `continue: false`, `auto_file_renaming: true`, `allow_overwrite` unset → existing file gets renamed (name.1), no file deletion. Single list row, `added_at` preserved (app.rs `EngineEvent::Added` takes the existing-task branch).
- Re-download source URL: use `t.url` if non-empty, else reconstruct `magnet:?xt=urn:btih:{info_hash}` (mirrors `ReaddTask` logic at app.rs:1090-1095). Button disabled when both are absent.
- No `EngineEvent::Removed` is emitted during re-download, so `gid_recently_removed` never interferes.
- Icons for re-download: `icon::refresh()` (`\u{E145}`, already generated).
- Missing tooltips to add: pause (Active/Waiting), resume (Paused), disabled pause (Error/Removed), disabled folder (empty `save_dir`), disabled copy (no url + no info_hash).

## Tasks (in order)

### 1. i18n — new key `ReDownload`
- `src/i18n.rs`: add `ReDownload` variant to `enum Tr` (near `Details`/`CopyLink`); add `Tr::ReDownload => "re-download"` in the key map (near `Tr::CopyLink => "copy-link"`).
- `i18n/locales/en/main.ftl`: add `re-download = Re-download`.
- `i18n/locales/zh-CN/main.ftl`: add `re-download = 重新下载`.

### 2. Message enum
- `src/message.rs`: add `RedownloadTask(String)` to `Message` (next to `ResumeTask`).

### 3. Engine — `EngineCmd::Redownload`
- `src/engine.rs`:
  - Add variant `Redownload { gid: String, url: String, save_dir: PathBuf, split: u16 }` to `EngineCmd`.
  - In `handle_client_cmd` add arm:
    - `let _ = client.remove_download_result(&gid).await;` (frees gid for reuse)
    - `add_uri(vec![url], TaskOptions { gid: Some(gid), dir, split, max_connection_per_server, continue: Some(false), auto_file_renaming: Some(true), ..Default::default() })` (mirror `ReaddTask` block at engine.rs:926-969).
    - On Ok: `tell_status(&gid)` → `emit_added` + `emit_progress` (real name via `name_from_status`).
    - On Err: `tracing::warn!(?gid, error = ?e, "re-download failed")`, no event.

### 4. App — handle `Message::RedownloadTask`
- `src/app.rs` (near `ResumeTask` handler, ~line 688):
  - `state.paused_gids.remove(&gid)`.
  - If task exists in `state.tasks`: derive url (url or magnet-from-hash), `split = state.settings.aria2.split`, send `EngineCmd::Redownload`. Missing url+hash → just log, no-op.
  - Send failure → `tracing::warn!("ui: redownload cmd send failed")`.

### 5. UI — `src/ui/task_list.rs`
- **`toolbar_icon` closure** (line 248): add a `tip_label: String` parameter and wrap the built button in `tip::standard(btn, text(tip_label).size(FONT_SMALL), tooltip::Position::Bottom)`. Keep returning `Element`.
- **pause/resume button** (lines 274-284):
  - `Active | Waiting` → pause icon, `Some(Message::PauseTask)`, tip `fluent.get(Tr::Pause)`.
  - `Paused` → play icon, `Some(Message::ResumeTask)`, tip `fluent.get(Tr::Resume)`.
  - `Completed` → NEW re-download button: `icon::refresh().size(FONT_ICON)`, `on_press(Message::RedownloadTask(gid))` only when `!t.url.is_empty() || t.info_hash.is_some()`, wrapped in `tip::standard` with `Tr::ReDownload`.
  - `_` (Error/Removed) → disabled pause icon, no action, tip `fluent.get(Tr::Pause)`.
- **disabled folder button** (lines 297-301): wrap in `tip::standard` with `Tr::ShowInFolder`.
- **disabled copy button** (lines 315-319): wrap in `tip::standard` with `Tr::CopyLink`.
- **spacing**: task-card toolbar row `.spacing(SPACE_XS)` → `.spacing(SPACE_SM)` (line 356).

## Risks
- aria2 `add_uri` with a reused gid fails if `remove_download_result` didn't purge (e.g., task still present). Mitigated by logging; result stays complete.
- Magnet re-download needs metadata fetch before progress shows; total starts at 0 — existing UI already handles total=0.
- `auto_file_renaming` may rename to `name.1`; intended (no overwrite).

## Validation
- `cargo clippy --workspace` (no warnings) and `cargo fmt --check`.
- Manual: complete a download → card shows refresh icon with "Re-download" tooltip; click → single row returns to downloading, progress resets; pause/resume/folder/copy icons all show tooltips; icon spacing slightly wider.
- Verify completed torrent/magnet task re-downloads via reconstructed magnet link.
