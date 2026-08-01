# Torrent File Selection (添加种子时选择文件 + 详情页修改文件选择) — 树形结构

## Goal
1. In the **Add Download dialog → Torrent tab**: after a `.torrent` is picked, show its internal files **as a collapsible tree** (directories ↔ files) below the drop zone, with tri-state directory checkboxes and per-file checkboxes (all selected by default). The "Download" button passes only the selected file indices to aria2 via the `select-file` option.
2. In the **Details dialog → Files tab**: render the same tree (with per-file progress bars) so the user can change file selection of an already-added torrent via aria2 `changeOption`.

No new crates. No DB migration (aria2 persists `select-file` in `session.txt`). Implementation keeps offline `cargo build` working.

## Verified facts (do not re-litigate)
- aria2 `select-file` accepts comma-separated 1-based indices (`1,2,5`), ranges (`1-5`), `*`. Matches `getFiles`/`tellStatus.files[].index` ordering.
- aria2 manual: *"In multi file torrent, the adjacent files specified by this option may also be downloaded. This is by design, not a bug."* → progress bars of unselected files may show partial data (shared pieces).
- `aria2.changeOption` + `select-file`: `select-file` is **not** in the "no restart" list, so on an **active** download aria2 **restarts the download itself** (no user intervention required). Direct `changeOption` is correct — do NOT implement pause/resume.
- Unselected files stay on disk (`bt-remove-unselected-file` default `false`); already-downloaded pieces are kept.
- The lucide font is **subsetted** (`fonts/icons.toml` → build.rs → `src/ui/icon.rs` + `fonts/lucide.ttf`). Chevron/file glyphs are NOT currently in the subset — `details_dialog.rs`'s raw `'\u{E0B4}'` file glyph is a tofu bug. `iced_lucide::build` is offline (embedded unicode data).

## 1. Icon subset (`fonts/icons.toml` + cleanup)
Add:
```toml
chevron_right = "chevron-right"   # U+E06F
chevron_down  = "chevron-down"    # U+E06D
file          = "file"            # U+E0C0
folder        = "folder"          # U+E0D7
```
build.rs re-runs automatically on change; regenerates `src/ui/icon.rs` (new fns `icon::chevron_right()`, `icon::chevron_down()`, `icon::file()`, `icon::folder()`) and `fonts/lucide.ttf`. In `src/ui/details_dialog.rs` (line 371) replace the raw `'\u{E0B4}'` glyph with `icon::file()` to fix the tofu bug.

## 2. New module `src/torrent_meta.rs`
Minimal bencode parser (~120 lines), `mod torrent_meta;` in `src/main.rs`.

```rust
pub struct TorrentFile { pub index: u64, pub path: String, pub length: u64 }  // index 1-based, file-list order
pub struct TorrentMeta { pub name: String, pub files: Vec<TorrentFile>, pub total_length: u64 }
pub fn parse_torrent(bytes: &[u8]) -> Option<TorrentMeta>
```
- Recursive-descent bencode: `i<n>e` ints, `<len>:<bytes>` strings, `l...e` lists, `d...e` dicts (keys as byte slices, e.g. `b"info"`).
- `info` dict: `name` (string), `length` (int, single-file), `files` (list of dicts with `length` + `path` list-of-strings, multi-file).
- Multi-file path = `info.name` joined with `path` segments via `/`; single-file path = `info.name`. `String::from_utf8_lossy`. `index = pos + 1` in list order.
- `None` on malformed input / empty file list.
- `#[cfg(test)]` unit tests: single-file, multi-file, truncated/garbage.

## 3. New component `src/ui/components/file_tree.rs` (shared, registered in `components/mod.rs`)
```rust
pub struct FileTreeNode {
    pub name: String,
    pub rel_path: String,          // '/' joined path from torrent root (no trailing '/')
    pub is_dir: bool,
    pub file_index: Option<u64>,   // Some for leaves (aria2 1-based index)
    pub length: u64,               // file length, or sum of children for dirs
    pub children: Vec<FileTreeNode>,
}
// Input: (index, rel_path, length) tuples. Split rel_path on '/'. Preserve INPUT ORDER for siblings
// (group by first segment in first-appearance order; do not sort). Single-segment paths become leaves.
pub fn build_tree(files: &[(u64, String, u64)]) -> Vec<FileTreeNode>
```
```rust
pub fn view<'a, M>(
    nodes: &'a [FileTreeNode],
    expanded: &'a HashSet<String>,
    is_selected: &impl Fn(u64) -> bool,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>, // (completed, length) per index; None => no bars
    enabled: bool,
    on_toggle: &impl Fn(String) -> M,                     // node rel_path
    on_expand: &impl Fn(String) -> M,                     // dir rel_path
) -> Element<'a, M>
```
Recursive `render_node(node, depth)`; indent = nested containers / leading `Space` at `depth * 14.0`:
- **Dir row**: `button(icon::chevron_right()/chevron_down())` (small, `theme::style::button::toolbar_icon(false)`) → `on_expand` + **tri-state checkbox** (custom: small rounded-square `button` showing `icon::circle_check()` when all descendants selected, `icon::minus()` when partial, `icon::square()` (dim) when none; click → `on_toggle(dir_path)`) + `icon::folder()` + `truncated_text(name)` + right-aligned `format_size(length)`. Children of expanded dirs render below with `depth+1`.
- **File row**: `iced::widget::checkbox(is_selected(idx))` WITHOUT `.label(...)` (box-only; `label` is optional) with `.on_toggle_maybe(if enabled { Some(move |_| on_toggle(rel_path)) } else { None })` + `icon::file()` + `truncated_text(name)` + right-aligned `format_size(length)`; if `progress` is Some, a thin 4-6px `progress_bar` below the row.
- Dir tri-state = All/Partial/None computed from `is_selected` over descendant file indices.
- Dir rows are never disabled; leaf checkboxes disabled when `enabled == false`.

## 4. Add dialog — state (`src/ui/add_dialog.rs`)
```rust
pub struct TorrentFileEntry { pub index: u64, pub path: String, pub length: u64, pub selected: bool }
// AddDialogState gains:
pub torrent_files: Vec<TorrentFileEntry>,   // flat source of truth; empty => download all (parse-fail fallback)
pub torrent_tree: Vec<FileTreeNode>,        // built via file_tree::build_tree
pub torrent_expanded: HashSet<String>,      // rel_paths of expanded dirs
pub torrent_parse_failed: bool,
```
Methods:
- `load_torrent_files(&mut self)` — read `torrent_upload.path()`, `parse_torrent`; success → fill entries (`selected: true`), rebuild `torrent_tree`, seed `torrent_expanded` with **all** dir rel_paths; failure → clear tree/entries + `torrent_parse_failed = true`.
- `set_torrent_path(&mut self, path: String)` — `torrent_upload.set_path(path)` then `load_torrent_files()`.
- `handle_torrent_event(&mut self, event)` — wraps `torrent_upload.update(event)`; on `Clear` clears `torrent_files`/`torrent_tree`/`torrent_expanded`/`torrent_parse_failed`; returns the action.
- `toggle_torrent_node(&mut self, path: &str)` — find node in `torrent_tree` by rel_path; file → flip its entry; dir → flip all descendant file entries; **revert if it would leave zero selected**.
- `toggle_torrent_expand(&mut self, path: &str)`; `set_all_torrent_files(&mut self, selected: bool)`.
- `selected_file_indices(&self) -> Vec<u64>`, `selected_total(&self) -> u64`, `all_selected()/none_selected()`.

Wiring: `new()` inits fields; `open()` clears all four; `open_with()` Torrent payload → `self.set_torrent_path(...)`; `can_submit()` Torrent tab → `!torrent_upload.is_empty() && save_dir_ok && (torrent_files.is_empty() || torrent_files.iter().any(|f| f.selected))`.

## 5. Add dialog — messages & app wiring
`src/message.rs`:
```rust
TorrentTreeExpand(String),     // dir rel_path
TorrentTreeToggle(String),     // node rel_path
TorrentFilesSelectAll,
TorrentFilesSelectNone,
```
`src/app.rs`:
- `Message::TorrentUpload(event)` → `add_dialog.handle_torrent_event(event)`; Browse returns `pick_path(PathPickerId::Torrent)` (app.rs:374-379).
- `FileDropped` (app.rs:390-409) and `apply_path(PathPickerId::Torrent, ..)` (app.rs:1970-1976) → `add_dialog.set_torrent_path(...)` after `is_valid_torrent_file`.
- `Message::AddDownload` torrent branch (app.rs:483-507): `let select_files = if files.is_empty() { None } else { let s = selected indices; if s.len() == files.len() { None } else { Some(s) } };` pass to `EngineCmd::AddTorrent`.
- New handlers → `toggle_torrent_expand` / `toggle_torrent_node` / `set_all_torrent_files`.

## 6. Add dialog — view (`src/ui/add_dialog.rs::view`)
In `AddTab::Torrent`, after the drop zone, when `!state.torrent_files.is_empty()`:
- Header row: `Files (N)` label + total size + right-aligned `Select all` / `Select none` buttons (`theme::style::button::text`).
- Body: `slim_scrollable(file_tree::view(..., progress: None, enabled: true, ...).width(Fill)).height(Length::Fixed(200.0))` (nested scroll keeps Save-to/split controls visible).
- Secondary line: `Selected: X / N · <selected total>`.
- If `torrent_parse_failed`: warning text row (secondary style) instead of the tree; download-all fallback stays enabled.

## 7. Details dialog — Files tab (`src/ui/details_dialog.rs`)
`src/message.rs`:
```rust
DetailsTreeExpand(String),
DetailsTreeToggle(String),
DetailsFilesSelectAll,
DetailsFilesSelectNone,
```
- `DetailsDialogState` gains `files_expanded: HashSet<String>`; reset in `open()` and when new `TaskDetails` arrives (app.rs:1155-1160 sets `files_expanded = all dir paths` from the received details).
- `files_tab` (details_dialog.rs:311): header row with overall progress (keep) + `Select all` / `Select none`; then `file_tree::view` built **in the view** from `state.details.details.files`:
  - rel_path = `file.path.strip_prefix(task.save_dir)` (fallback: basename) → `build_tree`.
  - `is_selected` = `details.files[i].selected`; `progress` = `(completed_length, length)` per index; `enabled` = task status not `Completed`/`Removed`; `on_toggle/on_expand` → new Messages.
- `src/app.rs` handlers:
  - `DetailsTreeToggle(path)`: build tree from `details.files`, find node, flip `selected` on all descendant `TaskFile`s (revert if zero remain), send `EngineCmd::SelectFiles { gid, files: <all selected 1-based indices> }` + `EngineCmd::FetchTaskDetails(gid)`.
  - `DetailsTreeExpand(path)`: toggle `files_expanded`.
  - `DetailsFilesSelectAll/None`: set all `details.files[i].selected`, send the same engine cmd + refetch.

## 8. Engine (`src/engine.rs`)
- `EngineCmd::AddTorrent` gains `select_files: Option<Vec<u64>>`.
- `add_torrent_and_emit(...)` gains `select_files: Option<&[u64]>`; before `client.add_torrent`:
  ```rust
  if let Some(files) = select_files {
      if !files.is_empty() {
          let csv = files.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
          options.extra_options.insert("select-file".into(), serde_json::Value::String(csv));
      }
  }
  ```
- `AddTorrent` handler passes it through; `FollowTorrent` passes `None`.
- New cmd + event:
  ```rust
  SelectFiles { gid: String, files: Vec<u64> },          // files must be non-empty
  // on failure: EngineEvent::SelectFilesFailed { gid }
  ```
  Handler: `client.change_option(&gid, TaskOptions { extra_options: { "select-file": csv }, ..Default::default() })`; on error send `SelectFilesFailed { gid }` (UI is optimistic and refetches details regardless, so checkboxes revert to real state).
- `src/app.rs` `SelectFilesFailed` handler → warning toast (`Tr::SelectFilesFailed`).

## 9. i18n (`src/i18n.rs`, `i18n/locales/en/main.ftl`, `i18n/locales/zh-CN/main.ftl`)
| Tr | key | en | zh-CN |
|---|---|---|---|
| `Tr::TorrentFiles` | `torrent-files` | Files | 文件 |
| `Tr::SelectAll` | `select-all` | Select all | 全选 |
| `Tr::SelectNone` | `select-none` | Select none | 全不选 |
| `Tr::TorrentParseFailed` | `torrent-parse-failed` | Could not read the torrent contents | 无法读取种子文件内容 |
| `Tr::SelectFilesFailed` | `select-files-failed` | Failed to change file selection | 修改文件选择失败 |

## 10. Risks & edge cases
- `select-file` is 1-based in torrent file-list order; parser order must match aria2 (`getFiles` order) — preserve input order in `build_tree`.
- `changeOption` requires ≥1 selected file → both dialogs revert the last uncheck. On **active** tasks aria2 auto-restarts (documented; no pause/resume in our code).
- Unselected files keep downloaded data on disk and may show partial progress (shared pieces) — accepted, documented in the UI's behavior only.
- Icons: must add to `fonts/icons.toml` BEFORE using `icon::chevron_*`/`icon::file()`/`icon::folder()`, otherwise build fails (functions won't exist). `details_dialog` raw `\u{E0B4}` glyph is tofu — replaced with `icon::file()`.
- Details rel paths: `strip_prefix(save_dir)` may fail (dir changed) → basename fallback (flat, no nesting).
- Default expansion = all expanded on load; resets when torrent/dialog changes. Very large trees scroll in a nested scrollable; if lag, default to top-level-only expansion (fallback, not implemented unless needed).
- Parse-failure fallback = current behavior (download all) + warning; `can_submit` still allows.
- Persistence across restarts comes from aria2 `session.txt` (`select-file` recorded); no DB change.
- Recursive `file_tree::view` must borrow closures via `&impl Fn` (reborrow per recursion depth) to satisfy `'a`/`M` lifetimes.

## 11. Validation
- `cargo build` (offline; verifies build.rs regenerates icons + no new deps), `cargo clippy --workspace` (no warnings), `cargo fmt --check`.
- `cargo test` for `torrent_meta` parser tests.
- Manual QA: (1) pick multi-file torrent in Add dialog, collapse/expand dirs, toggle a dir (tri-state) + individual files, Download → task list total reflects only selected files; (2) details → Files tab toggles selection of an active torrent (dir + file), totals update after refetch and aria2 restarts the download; (3) last remaining file cannot be unchecked; (4) invalid/corrupt `.torrent` shows parse-failed warning and still downloads all; (5) file glyph in Details → Files tab renders (no tofu).
