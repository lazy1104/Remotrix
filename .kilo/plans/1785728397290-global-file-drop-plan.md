# Global File Drag-in (全局文件拖入)

## Goal
Drop files anywhere on the app window to add downloads. File-type judgment reuses
the clipboard detection logic: `clipboard_watch::parse_clipboard` on the dropped
path string (torrent → Torrent tab, small UTF-8 text file with links → URL tab,
otherwise → warning toast). Show a full-window drop overlay while files hover.

Confirmed with user:
- Add dialog already open → fill the current dialog by content (torrent → Torrent tab, links → URL tab), replacing content.
- Show a full-window semi-transparent drop-overlay hint while files hover the window.

## Current State (context)
- `iced::window::Event::FileHovered(PathBuf) / FileDropped(PathBuf) / FilesHoveredLeft` already captured in `subscription()` (`src/app.rs:2507-2518`).
- `Message::FileHovered/FileDropped/FilesHoveredLeft` handlers at `src/app.rs:575-604` only act when the Add dialog is open on the Torrent tab.
- `clipboard_watch::parse_clipboard(&str, ClipboardLinkTypes)` (`src/clipboard_watch.rs:50`) already implements the required judgment for a single-line existing file path.
- `AddDialogState::open_with` (`src/ui/add_dialog.rs:115`) already applies a `ClipboardPayload` to a freshly-opened dialog.
- winit delivers one path per drop event → single-file drop handling.

## Changes

### 1. `src/message.rs`
- Add message variant: `DroppedFileParsed(Option<crate::clipboard_watch::ClipboardPayload>)` (near `ClipboardParsed`, ~line 117).

### 2. `src/app.rs`
- Add state field to `Remotrix` struct (after `add_dialog`): `drop_hover: bool`.
- Initialize `drop_hover: false` in `init()` (~line 138).
- Rework handlers (~lines 575-604):
  - `FileHovered(_)`: set `state.drop_hover = true`; keep existing `torrent_upload.set_dragging(true)` when dialog visible on Torrent tab.
  - `FilesHoveredLeft`: set `state.drop_hover = false`; keep `set_dragging(false)` when dialog visible.
  - `FileDropped(path)`: set `drop_hover = false`; clear torrent dragging; if a blocking modal is open (`show_close_dialog || about_dialog_visible || confirm.is_some()`) return `Task::none()`; else `Task::perform` `parse_clipboard(&path.to_string_lossy(), prefs)` → `Message::DroppedFileParsed`. Follow the async pattern used by `ClipboardRead` (`app.rs:1687-1701`) so file reads never block the UI thread.
  - `DroppedFileParsed(payload)`:
    - `None` → warning toast `Tr::NoDownloadableContent` (spawn via existing `spawn_toast`, `app.rs:2710`).
    - `Some(payload)`:
      - dialog visible → `state.add_dialog.apply_payload(payload)`, `Task::none()`.
      - dialog closed → `state.add_dialog.open_with(state.settings.download_dir.clone(), state.settings.split, payload)` + normal toast `Tr::DropDetected` (mirrors `ClipboardParsed`, `app.rs:1702-1727`).
- View (`view`, ~line 2355-2412): add a `drop_overlay` layer to the `stack!` after `confirm_layer`, before `toast_layer`, gated on `state.drop_hover && !(show_close_dialog || about_dialog_visible || confirm.is_some())`; render `Space::new()` otherwise. The overlay is a plain container (no `mouse_area`), so it does not capture events.

### 3. `src/ui/components/drop_overlay.rs` (new)
- `pub fn view<'a>(fluent: &'a Fluent, theme: &'a iced::Theme) -> Element<'a, Message>`:
  - Full-window `container` centered, style = new `theme::style::drop_overlay` (semi-transparent background so the app remains visible).
  - Centered hint card: icon (reuse `icon::arrow_up()`, as in `torrent_upload`) + `text(Tr::DropFilesHint)`, styled with existing `theme::style::drop_zone(true)` for the accent border/highlight.
- Register module in `src/ui/components/mod.rs`.

### 4. `src/ui/add_dialog.rs`
- Add method:
  ```rust
  pub fn apply_payload(&mut self, payload: crate::clipboard_watch::ClipboardPayload) {
      match payload {
          crate::clipboard_watch::ClipboardPayload::Urls(urls) => {
              self.set_urls(urls);
              self.active_tab = AddTab::Url;
          }
          crate::clipboard_watch::ClipboardPayload::Torrent(path) => {
              self.set_torrent_path(path.to_string_lossy().to_string());
              self.active_tab = AddTab::Torrent;
          }
      }
  }
  ```
- Refactor `open_with` to call `open(...)` then `self.apply_payload(payload)` (behavior unchanged for the fresh-open path).

### 5. `src/ui/theme.rs`
- Add `style::drop_overlay(t)` returning a container style with `background: Some(super::OVERLAY.into())` (reuse `OVERLAY`, theme.rs:70; same value as `style::overlay`).

### 6. i18n (`src/i18n.rs` + `.ftl`)
- `Tr` enum (near `DropTorrentHint`, i18n.rs:79): add `DropFilesHint`, `DropDetected`, `NoDownloadableContent`.
- Key map (`id()` fn, near i18n.rs:330): `DropFilesHint => "drop-files-hint"`, `DropDetected => "drop-detected"`, `NoDownloadableContent => "no-downloadable-content"`.
- `i18n/locales/en/main.ftl` (near line 88):
  - `drop-files-hint = Drop files to add downloads`
  - `drop-detected = Download content detected from dropped files`
  - `no-downloadable-content = No downloadable content detected in this file`
- `i18n/locales/zh-CN/main.ftl` (near line 88):
  - `drop-files-hint = 拖入文件添加下载`
  - `drop-detected = 已从拖入的文件识别到下载内容`
  - `no-downloadable-content = 未从文件中识别到可下载内容`

## Interaction with `torrent_upload` Drop Zone (conflict analysis)
No double-handling conflict:
- Window-level file DnD events (`FileHovered/FileDropped/FilesHoveredLeft`) arrive via `iced::event::listen_with` (app.rs:2507-2518); the drop zone's `mouse_area` (torrent_upload.rs:203-211) only captures mouse press/enter/exit. Different event channels; no preemption.
- The existing `Message::FileDropped` handler (app.rs:585-604) is **replaced** by the new global logic, so a drop is processed once.
- `FileHovered`/`FilesHoveredLeft` still call `torrent_upload.set_dragging()` when the dialog is visible on the Torrent tab, preserving the drop-zone highlight.

Two overlaps to reconcile:
- The global overlay is semi-transparent (`OVERLAY`, rgba(0,0,0,0.55)) and sits above the dialog, so the drop-zone dragging highlight is dimmed behind it. Harmless redundancy; keep both (if the overlay is ever disabled, the drop zone still highlights).
- **Corrupt `.torrent` guard (keep existing parity)**: `parse_clipboard` recognizes a torrent by extension only (clipboard_watch.rs:347), while the current drop path validates with `torrent_upload::is_valid_torrent_file` (extension + size ≤ 50MB + first byte `d`, torrent_upload.rs:45-64) and shows `Tr::InvalidTorrent` on failure. In `DroppedFileParsed`, when the payload is `ClipboardPayload::Torrent(path)` and `!torrent_upload::is_valid_torrent_file(&path)`, do **not** open/fill the dialog; instead show the existing `Tr::InvalidTorrent` warning toast. This keeps the pre-existing guard for both the dialog-open and dialog-closed cases.

## Behavior Notes / Edge Cases
- **Explicit scenario — Add dialog open on Torrent tab, drop a valid `.torrent`** (user-confirmed critical path): the drop zone in `torrent_upload.rs` never handled drops itself (it only renders the highlight and the Browse press, torrent_upload.rs:203-211); drops are always processed by the window-level `Message::FileDropped` in app.rs. Under the plan: hover → `FileHovered` keeps `torrent_upload.set_dragging(true)` + shows overlay; drop → `FileDropped` clears both → async `parse_clipboard` → `DroppedFileParsed(Some(Torrent))` → valid per `is_valid_torrent_file` → dialog visible → `apply_payload` → `set_torrent_path` + stays on Torrent tab (same outcome as today, app.rs:589-591). Corrupt `.torrent` → `InvalidTorrent` toast, dialog untouched (same as today, app.rs:593-602). So the torrent drop zone remains functional; only the overlay (and a one-frame async hop) is added.
- Multiple files: winit emits one path per event; each drop is handled independently (last one wins if dropped in quick succession). Directories and binary/non-link files → `parse_clipboard` returns `None` → `NoDownloadableContent` toast.
- A `.torrent` dropped while the dialog is open on the URL tab switches to the Torrent tab (per confirmed "按内容填充当前对话框").
- Corrupt/empty `.torrent` (by extension but fails `is_valid_torrent_file`) → `InvalidTorrent` toast, dialog untouched (see reconciliation above).
- Dropping while close/about/confirm modal is open is ignored (no overlay, no dialog change).
- Clipboard behavior (`last_clipboard_hash` dedup) is untouched; drops intentionally do not dedup.

## Validation
- `cargo clippy --workspace` and `cargo fmt --check` pass (no warnings).
- `cargo build` succeeds offline.
- Manual: drop a `.torrent` → dialog opens on Torrent tab with files parsed; drop a `links.txt` containing URLs → dialog opens on URL tab with links filled; drop a binary file → warning toast; drop a corrupt `.torrent` (empty file with `.torrent` extension) → `InvalidTorrent` toast, dialog untouched; hover shows overlay; overlay disappears on drop/cancel (`FilesHoveredLeft`); repeat all with the Add dialog already open (open on URL tab, drop torrent → switches to Torrent tab).
- `parse_clipboard` behavior itself is already covered by existing tests in `clipboard_watch.rs`; no new tests required (optionally add a small `apply_payload` test in `add_dialog.rs`, but `AddDialogState` currently has no test module).
