# File-list panel collapse (Add dialog) + keyed virtualization for scroll smoothness

## Goal

Revise the in-progress "collapse" feature and fix the remaining scroll lag:

1. **Redefine the collapse button** as a **panel collapse**: it collapses the whole framed file-list component down to just its header (master checkbox + title + subtitle), so the file list no longer consumes vertical height. It must **NOT** touch the tree's internal directory expansion state (`torrent_expanded`).
2. **Fix the tree scroll lag**: keep virtualization, but stop re-shaping every visible row on every scroll frame by keying the virtualized rows (iced `keyed_column`). This makes scrolling smooth immediately (not "after scrolling through all nodes once").
3. Collapse button appears in the **Add dialog only** (user decision); Details dialog passes `None`.

## Baseline (current uncommitted state)

- `file_tree.rs`: virtualized flat column (window `[first..last]` at `ROW_PITCH=24`), scroll-offset tracking via `on_scroll`, `first` clamped with `first.min(total)` (panic fix), `collect_aggregates` + `flatten_visible`.
- `torrent_file_list.rs`: `on_toggle_collapse: Option<M>` button currently driven by `expanded.is_empty()` (collapse-ALL-DIRS — wrong semantics), plus `scroll_offset`/`on_scroll` pass-through.
- `add_dialog.rs`: `torrent_scroll_offset`, `toggle_collapse_torrents()` (flips `torrent_expanded`), call passes `Some(Message::TorrentFilesToggleCollapse)`.
- `message.rs`: `TorrentFilesScroll(f32)`, `TorrentFilesToggleCollapse`, `DetailsFilesScroll(f32)`.
- `i18n.rs` + ftl: `Tr::CollapseAll`/`Tr::ExpandAll` → keys `collapse-all`/`expand-all`.
- `app.rs`: `Message::TorrentFilesToggleCollapse => state.add_dialog.toggle_collapse_torrents()`.
- `details_dialog.rs`: passes `None` for the collapse action.

## Root cause of the remaining lag

- The user observes the same "first scroll pass lags, then smooth" on the **static** Settings → Download page (no tree, no O(n) walks). That isolates a general iced cold-start cost (paragraph shaping + glyph rasterization), heavily amplified in **debug builds** (`cargo run --`). That part is inherent to iced and not fixable from app code.
- The file tree has an **additional, fixable** cost: its windowed rows live in a plain `column!`, and iced matches widget state **by index**. Every scroll frame the window shifts, so every row lands at a different index → the per-widget paragraph cache inside `truncated_text` misses for **all** ~200 window rows → cosmic-text re-shapes the whole window every frame.
- Fix: `iced::widget::keyed_column` (`iced_widget` `keyed/column.rs`) matches children **by key** (`diff_children_custom_with_search`), so per-row state (paragraph cache, hover, focus) persists per node across scroll. Only newly-entered rows are shaped per frame → scrolling is smooth immediately, and stays smooth on scroll-back.

## Implementation steps (ordered)

### 1. `src/ui/components/file_tree.rs` — key the virtualized rows

- Import: `use iced::widget::keyed_column;` (available as `iced::widget::keyed_column` in iced 0.14).
- Add a row-key enum (Copy + PartialEq; keys must be unique and stable per node):
  ```rust
  #[derive(Clone, Copy, PartialEq, Eq)]
  enum RowKey<'a> {
      SpacerTop,
      SpacerBottom,
      Node(&'a str),
  }
  ```
- Replace the flat `column!` assembly in `view` with:
  ```rust
  let mut items: Vec<(RowKey, Element<'a, M>)> = Vec::with_capacity(last - first + 2);
  items.push((RowKey::SpacerTop,
      Space::new().height(Length::Fixed(first as f32 * ROW_PITCH)).into()));
  for (node, depth) in &rows[first..last] {
      let el = if node.is_dir { /* render_dir_row(...) */ } else { /* render_file_row(...) */ };
      items.push((RowKey::Node(node.rel_path.as_str()), el));
  }
  items.push((RowKey::SpacerBottom,
      Space::new().height(Length::Fixed((total - last) as f32 * ROW_PITCH)).into()));
  let col = keyed_column(items).spacing(SPACE_NONE).width(Length::Fill);
  ```
- Keep everything else: `collect_aggregates`, `flatten_visible`, window math, `first.min(total)` clamp, `Vec::with_capacity`, the `container` + `scrollable` + `on_scroll` wrapper, `render_dir_row`/`render_file_row` (both already `.height(Length::Fixed(ROW_PITCH))`).
- `rel_path` is unique per node (build_tree dedups with ` (~n)`), so keys never collide with each other or the spacer keys.

### 2. `src/i18n.rs` + `i18n/locales/{en,zh-CN}/main.ftl` — rename keys

- `Tr::CollapseAll`/`Tr::ExpandAll` → `Tr::CollapseList`/`Tr::ExpandList`; key strings `collapse-list`/`expand-list`.
- en: `collapse-list = Collapse`, `expand-list = Expand`.
- zh-CN: `collapse-list = 折叠`, `expand-list = 展开`.
- Icons are unchanged and already correct for panel semantics: expanded panel shows `icon::collapse()` (chevrons-down-up), collapsed panel shows `icon::expand()` (chevrons-up-down).

### 3. `src/ui/components/torrent_file_list.rs` — panel collapse

- Add parameter `collapsed: bool` (place it after `enabled`).
- Collapse button: drive icon/label from `collapsed` instead of `expanded.is_empty()`:
  ```rust
  let (icon, label) = if collapsed {
      (icon::expand(), fluent.get(Tr::ExpandList))
  } else {
      (icon::collapse(), fluent.get(Tr::CollapseList))
  };
  ```
- Content assembly: when `collapsed`, render **only the header** (no `rule::horizontal(1)`, no `file_tree::view(...)` — skip the tree entirely, so the O(n) walks don't run) and use container height `Length::Shrink`; when expanded, keep current behavior (`rule` + `file_tree::view(...)` + given `height`). Header (master checkbox + title + subtitle + collapse button) stays in both states.
- Keep the master-checkbox `all_indices` walk (the header checkbox still needs total/selected counts even when collapsed).
- Keep the disabled branch (`btn.on_press_maybe(None)`) as defensive; Add dialog is always `enabled`.

### 4. `src/message.rs`, `src/ui/add_dialog.rs`, `src/app.rs` — toggle panel

- `message.rs`: rename `TorrentFilesToggleCollapse` → `TorrentFilesTogglePanel`.
- `add_dialog.rs`:
  - Add field `pub torrent_panel_collapsed: bool` (init `false` in `new()`; reset `false` in `open()`).
  - **Remove** `toggle_collapse_torrents` (the `torrent_expanded`-flipping logic). Add:
    ```rust
    pub fn toggle_torrent_panel(&mut self) {
        self.torrent_panel_collapsed = !self.torrent_panel_collapsed;
    }
    ```
  - Update the `torrent_file_list::view(...)` call: pass `state.torrent_panel_collapsed` and `Some(Message::TorrentFilesTogglePanel)`.
- `app.rs`: `Message::TorrentFilesTogglePanel => { state.add_dialog.toggle_torrent_panel(); }`.
- Keep `torrent_scroll_offset` and its existing reset points (`open`, `load_torrent_files`, `handle_torrent_event` Clear) and the `first.min(total)` clamp. No scroll reset needed on panel toggle (offset preserved; harmless when collapsed).

### 5. `src/ui/details_dialog.rs` — unchanged behavior

- Update the `torrent_file_list::view(...)` call to pass `false` for `collapsed` and keep `None` for `on_toggle_collapse` (button Add-only). No new Details state.

### 6. Validation

```bash
cargo build
cargo clippy --workspace   # no warnings
cargo fmt --check
```
Manual (`cargo run --`, debug build):
- Add dialog + a torrent with thousands of files: dialog opens with only a small single-frame cost; **scrolling is smooth immediately** (not "after a full pass"); collapse button (far right of header) collapses the panel to just the header and back; subtitle (total size) still renders; master checkbox + row checkboxes behave as before.
- Details → Files tab: no collapse button; scrolling tracked and smooth; clamp prevents any stale-offset panic.
- Settings → Download page: first-scroll cold-start may remain in debug (inherent iced text pipeline); confirm it is largely gone in `cargo run --release`.

## Notes / out of scope

- The general "first pass through a long scrollable is cold" cost (Settings page) is iced's paragraph-shaping + glyph-rasterization cold-start, worse in debug. No app-level fix; validate in release.
- The O(n) per-frame walks (`collect_aggregates`, `flatten_visible`, `all_indices`) remain; they were already present and are not the dominant cost once keying stops the per-frame re-shaping. Caching them (keyed by tree/expanded/selection version) is a possible follow-up for extremely large torrents in release builds.
- Window-size constants (`VIRTUAL_WINDOW_ROWS`/`VIRTUAL_BUFFER_ROWS`) can be tuned down later if the single first-frame cost still matters; not part of this change.
