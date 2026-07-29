# Task Card Redesign + Details Dialog + Task Persistence

## Goal
Redesign the download card in `src/ui/task_list.rs` into a 3-row layout with an icon toolbar, add a 3-tab details popup (Summary / Activity / Files), and back the task list with a `rusqlite` persistence layer so records (including `added_at`) survive restarts and sync with aria2 on app open.

## Context & Findings
- `DownloadTask` (`src/task.rs:3`) currently lacks: `connections`, `added_at`, a real `dir` (set to `PathBuf::new()` in `app.rs:455`), and `url` (empty string). `save_dir`/`url` are never populated.
- `EngineEvent::Progress` (`src/engine.rs:48`) only carries `downloaded/total/speed/status`. aria2's `tell_active` `Status` (`aria2-ws-0.5.1/src/response.rs:19`) already returns `connections`, `dir`, `piece_length`, `num_pieces`, `bitfield`, `files` - the data exists but is not propagated.
- aria2 does **not** expose add time (confirmed in `aria2_ws::response::Status` and motrix-next's `Aria2Task`). motrix-next persists it in a SQLite `task_birth` table (first-write-wins) and still uses aria2 `--save-session`/`--input-file` for active-task byte resumption.
- `format_size` (`src/task.rs:60`) already auto-converts B/KB/MB/GB/TB - the "xxx/xxx auto unit" requirement is already satisfied.
- Dialogs are in-app modal overlays via `stack!` (`src/app.rs:763`). Pattern: a `*DialogState` struct in the dialog module + a `view()` rendered in `app::view` when visible.
- Icons: `iced_lucide` builds `fonts/lucide.ttf` from `fonts/icons.toml` and generates `src/ui/icon.rs` (`fn name() -> Text`, plus `ALL_ICONS`). Add new icons by editing `fonts/icons.toml`; use generated `crate::ui::icon::name()` accessors (chain `.size()`/`.color()`).
- Locale files: `i18n/locales/en/main.ftl` and `i18n/locales/zh-CN/main.ftl` (fluent `key = value`). `Tr` enum + `key()` map in `src/i18n.rs`.
- `iced::clipboard::copy::<Message>(text) -> Task<Message>` and `iced::time::every(Duration) -> Subscription<Instant>` are core (not feature-gated) in iced 0.14. `iced::widget::canvas` requires the `canvas` feature (NOT in `advanced`).
- Config dirs: `directories::ProjectDirs::from("dev","remotrix","Remotrix")`; `data_dir()` used for aria2/logs (`src/config.rs:391`). DB will live in `data_dir()`.

## Decisions
1. **Card = 3 rows.** Row1: name (left) + icon toolbar (right) = pause/start, show-in-folder, copy-link, details, delete. Row2: progress bar (status-encoded color: paused=warning, error=danger, else success). Row3: left = `downloaded/total` (`format_size`); right = ETA + speed (success color) + connections (plug icon + count). No separate pct or status text on the card (bar color + toolbar icon convey status); for completed/error tasks ETA/speed show `-` and connections `0`.
2. **Details popup** = modal overlay (`stack!`), custom button tab bar (no iced_aw `tabs` feature), 3 tabs (extensible): Summary, Activity, Files.
   - **Summary**: GID, file name, download location (dir), task status (localized), added time.
   - **Activity**: piece map (canvas) + "done/total pieces" + piece size, task progress bar, downloaded/total, speed, connections.
   - **Files**: flat list (user-confirmed), each file row = basename + size + per-file progress bar + pct.
   - Live refresh every ~2s while open (user-confirmed) via `iced::time::every` -> `EngineCmd::FetchTaskDetails`.
3. **Piece map** = `iced::widget::canvas` (add `canvas` iced feature) in a new `src/ui/piece_map.rs`. A single `canvas::Program` draws colored cells from `bitfield`+`num_pieces` (green=downloaded, background=missing), laid out in rows wrapping to the widget width. Cell size ~8px; widget height fixed (~160px) and cells beyond are clipped - the exact "done/total pieces" count is shown as text alongside, so clipping is acceptable. Scales to 10k+ pieces as one drawn widget (avoids widget explosion).
4. **Persistence** = `rusqlite` with `bundled` feature (user-confirmed). DB is the task-list source of truth; loaded on startup (list renders immediately); reconciled with aria2 via existing `sync_existing_tasks` + 1s poll (no new sync RPC needed). `added_at` is first-write-wins, persisted, never overwritten.
5. **DB ownership & write strategy**: `Db` handle held by `Remotrix` (iced main thread), `std::sync::Mutex<Connection>`, WAL mode. Writes:
   - **Immediate**: `Added` (insert with `added_at`), `Removed`/`ClearCompleted`/`DeleteAll` (delete).
   - **Debounced**: `Progress` updates the in-memory `HashMap` immediately and marks the gid dirty; a `dirty: HashSet<String>` is flushed every 1s via `iced::time::every` -> `Message::FlushDirty` as a single transaction (`upsert_progress` per dirty gid, preserve `added_at`). This absorbs the boot-sync burst and avoids per-second UI-thread writes for many active tasks.
   - All DB call sites guard `Option<Db>`: if `db_path()` is `None` or open fails, `db = None` and every DB op is a no-op (graceful degradation to in-memory-only, same as pre-change behavior).
6. **Reconciliation semantics**:
   - `EngineEvent::Added{gid,name,url,dir}`: if gid exists -> update name/url/dir in HashMap + mark dirty (keep `added_at`); if new -> create `DownloadTask { ..., connections:0, added_at: now_secs() }`, `db.upsert_meta(...)` immediately.
   - `EngineEvent::Progress{...,connections}`: update HashMap fields incl. `connections`; `dirty.insert(gid)`.
   - `EngineEvent::Removed(gid)`: delete from HashMap + `db.delete(gid)` + `dirty.remove(gid)`.
   - `DeleteAll`: `db.delete_all()`, `dirty.clear()`. `ClearCompleted`: for each cleared gid `db.delete(gid)` + `dirty.remove()`. Deletes are idempotent (no-op if missing).
   - Tasks in DB but absent from aria2 on startup: remain in list with last-known (stale) fields (known limitation, v1).
7. **add_time** recorded client-side at first-seen (Added), stored as unix seconds (`i64`); formatted via `chrono::Local` -> `"YYYY-MM-DD HH:MM:SS"` for the Summary tab. `now_secs()` via `chrono::Utc::now().timestamp()`.
8. **Engine event expansion** (must stay in sync between `engine.rs` and `message.rs`):
   - `Added { gid, name, url, dir }` (add `url`, `dir`).
   - `Progress { gid, downloaded, total, speed, status, connections }` (add `connections`).
   - New `EngineCmd::FetchTaskDetails(String)` + `EngineEvent::TaskDetails { gid, details }` (`details` = heavy/on-demand fields only: `bitfield`, `num_pieces`, `piece_length`, `files`, `upload_speed`, `num_seeders`, `info_hash`, `error_code`, `error_message`). Live numbers (downloaded/total/speed/status/connections/name/dir/add_time) come from `DownloadTask`.
   - New `EngineEvent::TaskDetailsFailed { gid }` when `tell_status` errors (task gone) so the popup can stop spinning.
9. **Show in folder**: add `open` crate, invoke via `Task::perform(async { open::that(&dir) }, |_| Message::Noop)`; disabled (no `on_press`) if `dir` empty.
10. **Copy link**: `iced::clipboard::copy(task.url)`; disabled if `url` empty (torrent tasks).
11. **Icons**: add to `fonts/icons.toml`: `folder_open = "folder-open"`, `copy = "copy"`, `details = "file-text"`, `connections = "plug"`. Use generated `crate::ui::icon::folder_open().size(15).color(...)` etc. (`plug` may be swapped for `cable`/`network` if it reads poorly.)
12. **Task-removed-while-popup-open**: `details_dialog::view` takes `task: Option<&DownloadTask>`; if `None`, show a "task no longer exists" message + close button instead of tabs. `TaskDetailsReceived`/`TaskDetailsFailed` set `loading=false`.

## Implementation Tasks

### 1. Dependencies & build config
- `Cargo.toml`:
  - iced features -> `["tokio", "advanced", "image", "canvas"]`.
  - add `rusqlite = { version = "0.32", features = ["bundled"] }`, `chrono = { version = "0.4", default-features = false, features = ["clock"] }`, `open = "5"`.
- `fonts/icons.toml`: add the 4 icon entries above (rebuild regenerates `src/ui/icon.rs`).
- `src/main.rs`: add `mod db;`.

### 2. Data model (`src/task.rs`)
- Add fields to `DownloadTask`: `connections: u64`, `added_at: i64` (unix seconds). Existing `url`/`save_dir` now actually populated.
- Add `TaskFile { index: u64, path: String, length: u64, completed_length: u64, selected: bool }` and `TaskDetails { bitfield: Option<String>, num_pieces: u64, piece_length: u64, files: Vec<TaskFile>, upload_speed: u64, num_seeders: Option<u64>, info_hash: Option<String>, error_code: Option<String>, error_message: Option<String> }`.
- Add `format_add_time(unix_secs: i64) -> String` (chrono::Local).
- Add `completed_pieces(bitfield: Option<&str>, num_pieces: u64) -> (u64 /*done*/, u64 /*total*/)` (bit set = downloaded; highest bit = piece 0; count set bits within `num_pieces`).

### 3. Persistence layer (`src/db.rs`, new)
- `pub struct Db { conn: std::sync::Mutex<rusqlite::Connection> }`.
- `open(path: &Path) -> Result<Db, String>`: create parent dir; open with `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;`; create table.
- Schema (`status` stored as the lowercase aria2 status string):
  ```sql
  CREATE TABLE IF NOT EXISTS tasks (
    gid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    url TEXT NOT NULL DEFAULT '',
    dir TEXT NOT NULL DEFAULT '',
    downloaded INTEGER NOT NULL DEFAULT 0,
    total INTEGER NOT NULL DEFAULT 0,
    speed INTEGER NOT NULL DEFAULT 0,
    connections INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    added_at INTEGER NOT NULL
  );
  ```
- Methods (all tolerate missing row / are idempotent):
  - `load_all() -> Vec<DownloadTask>` (SELECT *, `ORDER BY added_at DESC`).
  - `upsert_meta(gid, name, url, dir, status, added_at)` - `INSERT ... ON CONFLICT(gid) DO UPDATE SET name/url/dir/status` (omit `added_at` from UPDATE so it is preserved).
  - `upsert_progress(gid, downloaded, total, speed, connections, status)` - `INSERT ... ON CONFLICT(gid) DO UPDATE SET downloaded/total/speed/connections/status` (no `added_at`; row is guaranteed to exist because `Added` inserts first).
  - `flush(dirty: &[(gid, downloaded, total, speed, connections, status)])` - single transaction calling `upsert_progress` per row.
  - `delete(gid)`, `delete_all()`, `clear_completed(gids: &[String])`.

### 4. Config (`src/config.rs`)
- Add `pub fn db_path() -> Option<PathBuf>` returning `data_dir().join("remotrix.db")` (mirror `log_dir()` using `ProjectDirs` `data_dir()`).

### 5. Engine (`src/engine.rs`)
- Expand `EngineEvent::Added { gid, name, url, dir }`.
- Expand `EngineEvent::Progress { gid, downloaded, total, speed, status, connections }`.
- Add `EngineCmd::FetchTaskDetails(String)`.
- Add `EngineEvent::TaskDetails { gid: String, details: crate::task::TaskDetails }` and `EngineEvent::TaskDetailsFailed { gid: String }`.
- `emit_progress`: include `connections` from `Status.connections`.
- `Added` url/dir:
  - `sync_existing_tasks`: `url = s.files.first().and_then(|f| f.uris.first()).map(|u| u.uri.clone()).unwrap_or_default()`, `dir = s.dir.clone()`.
  - `AddDownload`/`AddTorrent` handlers: `url` = first input uri / `""` (torrent); `dir` from options.
- `handle_client_cmd` for `FetchTaskDetails(gid)`: `client.tell_status(&gid)` -> build `TaskDetails` (map `files` to `TaskFile`) -> emit `TaskDetails`; on error emit `TaskDetailsFailed { gid }`.

### 6. Messages (`src/message.rs`)
- `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum DetailsTab { Summary, Activity, Files }`.
- Add `Message` variants: `OpenTaskDetails(String)`, `CloseTaskDetails`, `RefreshTaskDetails`, `FlushDirty`, `SelectDetailsTab(DetailsTab)`, `OpenTaskFolder(String)`, `CopyTaskLink(String)`, `Noop`.
- (`TaskDetailsReceived`/`TaskDetailsFailed` flow through the existing `Message::Engine(EngineEvent)` arm.)

### 7. Piece map (`src/ui/piece_map.rs`, new)
- `pub struct PieceMap { bitfield: Option<String>, num_pieces: u64, done: u64, color_done: Color, color_missing: Color }`.
- Implement `canvas::Program<Message>` (`draw` iterates pieces, paints ~8px cells, wraps to widget width, clipped to a fixed height with scroll not needed - just cap visible rows). Register in `src/ui/mod.rs`.

### 8. Details dialog (`src/ui/details_dialog.rs`, new)
- `pub struct DetailsDialogState { visible: bool, gid: Option<String>, active_tab: DetailsTab, details: Option<TaskDetails>, loading: bool }` with `open(gid)`, `close()`, `is_visible()`.
- `pub fn view(fluent, theme, task: Option<&DownloadTask>, state: &DetailsDialogState) -> Element` - modal overlay (`theme::style::overlay`), panel `Length::Fixed(640.0)`, header (title + close button), custom tab bar (3 buttons, active styled with `theme::style::button::sidebar_icon(true)`). Body:
  - If `task` is `None`: "task no longer exists" + close.
  - **Summary**: key/value rows GID, name, dir, status (localized via existing `Tr`), added time (`format_add_time`).
  - **Activity**: `PieceMap` canvas + "done/total pieces" + piece size (`format_size(piece_length)`) + progress bar (from task) + downloaded/total + speed + connections. If `details` is `None` show loading text.
  - **Files**: overall task progress bar at top (downloaded/total + pct), then `scrollable` column; per file: file glyph + basename (path file_name) + `format_size(length)` + per-file `progress_bar(0..=100, pct)` + pct text.
- Register in `src/ui/mod.rs`: `pub mod details_dialog;`.

### 9. Card rewrite (`src/ui/task_list.rs`)
- Rewrite `task_card` to 3 rows:
  - Row1 `row![name, Space::Fill, toolbar]`; `toolbar` = icon buttons via `crate::ui::icon::*().size(15)` each in a `tooltip`:
    - pause (`icon::pause`) / resume (`icon::play`) by status -> `PauseTask`/`ResumeTask`.
    - show-in-folder (`icon::folder_open`) -> `OpenTaskFolder(gid)`; omit `on_press` if `save_dir` empty.
    - copy-link (`icon::copy`) -> `CopyTaskLink(gid)`; omit `on_press` if `url` empty.
    - details (`icon::details`) -> `OpenTaskDetails(gid)`.
    - delete (`icon::trash`) -> `RemoveTask(gid)`.
  - Row2: `progress_bar(0..=100, pct)` with status-based color (keep existing logic).
  - Row3: `row![ downloaded/total text (secondary), Space::Fill, eta text (secondary), sep, speed text (success), sep, icon::connections().size(12) + count text ]`.
- Remove the old bottom actions row (pause/resume/remove buttons + pct text). Keep `view()` signature unchanged.

### 10. App state & wiring (`src/app.rs`)
- Add fields: `db: Option<crate::db::Db>`, `dirty: std::collections::HashSet<String>`, `details: crate::ui::details_dialog::DetailsDialogState`.
- `init()`: `db = config::db_path().and_then(|p| Db::open(&p).ok())`; `db.load_all()` -> populate `tasks` + `task_order` (already DESC by `added_at`). Keep `engine::spawn_engine()` as-is.
- `update()`:
  - `Engine(Added{gid,name,url,dir})`: if exists -> update name/url/dir + `dirty.insert(gid)`; else create task with `added_at=now_secs()`, insert, `db.upsert_meta(...)`. Set `task.url`/`task.save_dir=PathBuf::from(dir)`.
  - `Engine(Progress{...,connections})`: update HashMap incl. `connections`; `dirty.insert(gid)`.
  - `Engine(Removed(gid))`: remove from HashMap; `db.delete(gid)`; `dirty.remove(gid)`.
  - `Engine(TaskDetails{gid,details})`: if `details.gid == state.details.gid` -> store, `loading=false`.
  - `Engine(TaskDetailsFailed{gid})`: if matches open gid -> `loading=false` (popup shows last/empty; if task also gone, view shows "no longer exists").
  - `FlushDirty`: collect `(gid, downloaded, total, speed, connections, status)` from HashMap for dirty gids; `db.flush(...)`; `dirty.clear()`.
  - `DeleteAll`: `db.delete_all()`, `dirty.clear()` (plus existing HashMap clear + RemoveAll cmd).
  - `ClearCompleted`: for each removed gid `db.delete(gid)` + `dirty.remove()`.
  - `OpenTaskDetails(gid)`: `state.details.open(gid)`; send `EngineCmd::FetchTaskDetails(gid)`.
  - `CloseTaskDetails`: `state.details.close()`.
  - `RefreshTaskDetails`: if visible, send `EngineCmd::FetchTaskDetails(details.gid)`.
  - `SelectDetailsTab(t)`: set `active_tab`.
  - `OpenTaskFolder(gid)`: if `task.save_dir` non-empty -> `Task::perform(async move { let _ = open::that(&dir); }, |_| Message::Noop)` (`open::that` is sync; calling it inside the async block runs it off the UI thread).
  - `CopyTaskLink(gid)`: if `task.url` non-empty -> `iced::clipboard::copy::<Message>(task.url.clone())`.
  - `Noop`: `Task::none()`.
- `view()`: when `state.details.is_visible()`, push `details_dialog::view(&fluent, t, state.details.gid.as_deref().and_then(|g| state.tasks.get(g)), &state.details)` onto the `stack!` (after close_dialog).
- `subscription()`: batch includes, unconditionally, `iced::time::every(Duration::from_millis(1000)).map(|_| Message::FlushDirty)`; and, when `state.details.is_visible()`, `iced::time::every(Duration::from_millis(2000)).map(|_| Message::RefreshTaskDetails)`.

### 11. i18n (`src/i18n.rs` + both `main.ftl`)
- Add `Tr` variants + keys: `Details`, `TabSummary`, `TabActivity`, `TabFiles`, `FieldGid`, `FieldFileName`, `FieldDownloadLocation`, `FieldTaskStatus`, `FieldAddedTime`, `Pieces`, `PieceSize`, `CompletedPieces`, `Speed`, `Connections`, `ShowInFolder`, `CopyLink`, `Loading`, `TaskGone`.
- EN + zh-CN strings (e.g. `tab-summary = Summary`/`概要`; `tab-activity = Activity`/`活动信息`; `tab-files = Files`/`文件信息`; `field-added-time = Added time`/`添加时间`; `show-in-folder = Show in folder`/`在文件夹中显示`; `copy-link = Copy link`/`复制链接`; `connections = Connections`/`连接数`; `pieces = Pieces`/`分片`; `task-gone = This task no longer exists`/`该任务已不存在`; `loading = Loading…`/`加载中…`).

## Risks & Edge Cases
- **Stale persisted records**: a task in DB but not in aria2 (session loss) stays in the list with frozen fields/status. v1 acceptable; future could mark "stale"/offer re-add. Document in a code comment.
- **Boot-sync burst**: `sync_existing_tasks` can emit thousands of `Added`/`Progress` at boot. `Added` writes are immediate (inserts, fast); `Progress` only marks dirty and flushes once/sec in one transaction -> no startup hitch.
- **First run / no migration**: existing users have no DB; on first run the list is empty until `sync_existing_tasks` populates it with `added_at = now` (they had no add_time before). No schema migration needed.
- **gid divergence**: aria2 preserves gids across session save/load, so DB↔aria2 reconciliation by gid is stable. If a gid reappears with different content, upsert updates metadata but keeps `added_at`.
- **Copy-link for torrents**: `url` empty -> button disabled.
- **Channel protocol sync**: `EngineEvent` shape changes must match the `Message::Engine(EngineEvent)` match arms in `app.rs` - update `engine.rs` and `app.rs` together.
- **icons.toml rebuild**: adding icons regenerates `src/ui/icon.rs`; `build.rs` watches `fonts/icons.toml`.
- **canvas feature**: adding `canvas` increases compile time slightly (lyon); acceptable.

## Validation
- `cargo fmt --check` and `cargo clippy --workspace` (no warnings).
- `cargo build` (offline-safe; downloads nothing at build).
- `cargo run --`:
  - Add an HTTP download -> card shows 3 rows; toolbar icons work; row3 shows downloaded/total, ETA, speed, connections (plug icon + count).
  - Open details popup -> Summary (GID/name/dir/status/added time), Activity (piece map + progress + speed + connections), Files (per-file progress).
  - While popup open, Activity/Files refresh every ~2s.
  - Show-in-folder opens the dir in the OS file manager; Copy-link puts URL on clipboard.
  - Restart the app -> task list reappears from DB immediately with preserved `added_at`; live fields sync from aria2 within ~1s.
  - Remove a task -> gone from list and DB (verify after restart it does not reappear).
  - Pause/resume toggles the toolbar icon between play/pause.
  - Open details for a task, then remove it -> popup shows "task no longer exists".

## Out of Scope
- Full `download_history` archive (separate completed-task history UI) - only the live task list is persisted.
- Stale-task detection / re-add UX for DB records absent from aria2.
- Nested file tree (flat list chosen).
- Separate iced window for details (modal overlay chosen).
- Overlay-click-to-close for the popup (close button only, matching existing dialogs).
