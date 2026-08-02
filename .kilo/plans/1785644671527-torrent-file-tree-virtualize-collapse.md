# Torrent File Tree: virtualized rendering + header collapse button

## Goal

1. **Fix the laggy initial render** of the torrent file tree (`src/ui/components/file_tree.rs`). Current render recursively builds a deeply nested column of *every* expanded row every frame, and each dir row calls `descendant_indices` twice per frame (O(n·depth)). For large torrents (all dirs expanded by default) this stalls the first frame and every subsequent redraw.
2. **Add a collapse-all / expand-all button** in the header of the framed file-list panel, shown **only in the New Download dialog** (Add dialog), per user decision.

## Confirmed decisions

- **Perf fix = virtualized rendering**: keep the current "all dirs expanded" default UX; instead flatten visible rows into one flat column, compute dir aggregates in a single O(n) pass, and render only rows near the scroll position (fixed row pitch + scroll offset tracked via `on_scroll`). First render becomes near-instant and scrolling stays smooth at any torrent size.
- **Collapse button appears in the Add dialog only.** The shared `torrent_file_list::view` gains `on_toggle_collapse: Option<M>`; Details dialog passes `None` (no button).
- **Button is a single toggle** (collapse-all ↔ expand-all), mirroring the master-checkbox pattern: icon + action are chosen at render time from current state (`expanded` set empty → "expand all", else → "collapse all"). The message handler flips state; scroll offset resets to 0 on toggle.

## New/changed files

`src/message.rs`, `src/app.rs`, `src/ui/add_dialog.rs`, `src/ui/details_dialog.rs`, `src/ui/components/file_tree.rs`, `src/ui/components/torrent_file_list.rs`, `src/i18n.rs`, `i18n/locales/{en,zh-CN}/main.ftl`, `fonts/icons.toml` (+ regenerated `src/ui/icon.rs`, `fonts/lucide.ttf` via `iced_lucide::build`, offline).

## Implementation steps (ordered)

### 1. Icons — `fonts/icons.toml`

Add two Lucide icons (validated by `iced_lucide::build` at compile time):
```toml
collapse = "chevrons-down-up"
expand = "chevrons-up-down"
```
Build regenerates `src/ui/icon.rs` (adds `icon::collapse()`, `icon::expand()`) and the font subset. Do not edit generated files by hand.

### 2. i18n — `src/i18n.rs` + ftl files

- Add variants `Tr::CollapseAll`, `Tr::ExpandAll` to the `Tr` enum and their `fluent_str` key entries (`collapse-all`, `expand-all`).
- `i18n/locales/en/main.ftl`: `collapse-all = Collapse all`, `expand-all = Expand all`.
- `i18n/locales/zh-CN/main.ftl`: `collapse-all = 全部折叠`, `expand-all = 全部展开`.

### 3. Messages — `src/message.rs`

Add next to the existing torrent/details variants (after line 75):
```rust
TorrentFilesScroll(f32),
TorrentFilesToggleCollapse,
DetailsFilesScroll(f32),
```

### 4. `src/ui/components/file_tree.rs` — virtualized rewrite

Keep unchanged: `build_tree`, `sum_lengths`, `collect_indices`, `descendant_indices`, `find_node`, `collect_dir_paths`, `flip_with_guard`, `FileTreeNode`, `MAX_TREE_DEPTH`, `INDENT_STEP`, `CHEVRON_SLOT`.
Remove: `dir_state` (dead after rewrite), recursive `render_node`.

New constants:
```rust
const ROW_PITCH: f32 = 24.0;          // fixed row height incl. spacing (tunable)
const VIRTUAL_BUFFER_ROWS: usize = 40;    // rows above/below the window
const VIRTUAL_WINDOW_ROWS: usize = 200;   // window = 200 + 2*40 max rows
```

New helper — single-pass aggregates over the whole tree (keyed by `rel_path`):
```rust
struct DirAgg { selected: u32, total: u32, done: u64 }
fn collect_aggregates(
    node: &FileTreeNode,
    is_selected: &impl Fn(u64) -> bool,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>,
    out: &mut HashMap<String, DirAgg>,
) -> DirAgg   // post-order; inserts aggregate only for is_dir nodes
```

New helper — flatten visible rows in DFS order respecting `expanded`:
```rust
fn flatten_visible<'a>(nodes: &'a [FileTreeNode], expanded: &HashSet<String>, out: &mut Vec<(&'a FileTreeNode, u32)>, depth: u32)
```

Rewrite `view`:
```rust
pub fn view<'a, M>(
    nodes, expanded, is_selected, progress, enabled,
    on_toggle, on_expand,
    scroll_offset: f32,
    on_scroll: &impl Fn(f32) -> M,   // new params
) -> Element<'a, M>
```
Body:
1. `collect_aggregates` over all roots → `HashMap<String, DirAgg>`.
2. `flatten_visible` → `rows: Vec<(&FileTreeNode, u32)>`; `total = rows.len()`.
3. Window: `first = (scroll_offset / ROW_PITCH).floor().max(0.0) as usize .saturating_sub(VIRTUAL_BUFFER_ROWS)`; `last = (first + VIRTUAL_WINDOW_ROWS).min(total)`.
4. Build a **flat** `column![].spacing(SPACE_NONE)`:
   - top `Space::new().height(Length::Fixed(first * ROW_PITCH))`
   - `rows[first..last]` rendered via `render_dir_row`/`render_file_row` (dispatch on `node.is_dir`), each row `.height(Length::Fixed(ROW_PITCH))` so content height = `total * ROW_PITCH` exactly (scrollbar accuracy)
   - bottom `Space::new().height(Length::Fixed((total - last) * ROW_PITCH))`
5. Wrap in a vertical scrollable with `on_scroll` (replace the `slim_scrollable(col)` call; drop the now-unused `slim_scrollable` import):
```rust
iced::widget::scrollable(
    container(column).width(Length::Fill).padding(iced::padding::bottom(5.0)),
)
.direction(iced::widget::scrollable::Direction::Vertical(
    iced::widget::scrollable::Scrollbar::new().width(6.0).scroller_width(6.0),
))
.spacing(SPACE_SCROLL)
.style(theme::style::scrollable::standard)
.height(Length::Fill)
.on_scroll(move |v: iced::widget::scrollable::Viewport| on_scroll(v.absolute_offset().y))
```
(Add `container`, `scrollable`, `Scrollbar` imports as needed.)

`render_dir_row` changes:
- Take `agg: &DirAgg` (looked up from the aggregates map) instead of calling `descendant_indices`.
- Check state: `agg.total > 0 && agg.selected == agg.total` → `Checked`; `agg.selected > 0` → `Partial`; else `Unchecked`.
- Size text: with `progress`, `format!("{} / {}", format_size(agg.done), format_size(node.length))`; else `format_size(node.length)`.
- Row gets `.height(Length::Fixed(ROW_PITCH))`.

`render_file_row` changes: only add `.height(Length::Fixed(ROW_PITCH))`; otherwise untouched.

### 5. `src/ui/components/torrent_file_list.rs` — header button + pass-through

- Imports: restore `use crate::i18n::{Fluent, Tr};` (rename `_fluent` back to `fluent`), add `use crate::ui::icon;` and `use crate::ui::components::tooltip;`.
- Signature: add `on_toggle_collapse: Option<M>` and `scroll_offset: f32`, `on_scroll: &'a impl Fn(f32) -> M`. Forward `scroll_offset`/`on_scroll` to `file_tree::view`.
- Header layout becomes `[master][title][Space:Fill][subtitle?][collapse-button?]`. When `on_toggle_collapse` is `Some` and `enabled`:
```rust
let collapsed = expanded.is_empty();
let (icon, label) = if collapsed {
    (icon::expand(), fluent.get(Tr::ExpandAll))
} else {
    (icon::collapse(), fluent.get(Tr::CollapseAll))
};
let btn = button(icon.size(FONT_SMALL))
    .padding(PADDING_XS)
    .style(theme::style::button::toolbar_icon(false));
let btn = if enabled {
    btn.on_press(on_toggle_collapse) // Option<M> → move/unwrap at build time; disabled → on_press_maybe(None::<fn() -> M>)
} else { btn.on_press_maybe(None::<fn() -> M>) };
header = header.push(tooltip::standard(btn, text(label).size(FONT_TINY), tooltip::Position::Bottom));
```
Note: `Option<M>` is owned/consumed only in the `Some` branch (like the master checkbox consumes `on_select_all`/`on_select_none`).

### 6. `src/ui/add_dialog.rs`

- Add state field `pub torrent_scroll_offset: f32` (init `0.0` in `new()` and `open()`; also reset on torrent load/clear in `load_torrent_files` and `handle_torrent_event`).
- Add method:
```rust
pub fn toggle_collapse_torrents(&mut self) {
    if self.torrent_expanded.is_empty() {
        file_tree::collect_dir_paths(&self.torrent_tree, &mut self.torrent_expanded);
    } else {
        self.torrent_expanded.clear();
    }
    self.torrent_scroll_offset = 0.0;
}
```
- Add helper `fn torrent_files_scroll(y: f32) -> Message { Message::TorrentFilesScroll(y) }`.
- Update the `torrent_file_list::view(...)` call: pass `state.torrent_scroll_offset`, `&torrent_files_scroll`, and `Some(Message::TorrentFilesToggleCollapse)`.

### 7. `src/ui/details_dialog.rs`

- Add state field `pub files_scroll_offset: f32` (init `0.0` in `new()`; reset in `open()`/`close()`).
- Add helper `fn details_files_scroll(y: f32) -> Message { Message::DetailsFilesScroll(y) }`.
- Update the `torrent_file_list::view(...)` call: pass `state.files_scroll_offset`, `&details_files_scroll`, and `None` (no collapse button).

### 8. `src/app.rs` — handlers

```rust
Message::TorrentFilesScroll(off) => { state.add_dialog.torrent_scroll_offset = off; }
Message::TorrentFilesToggleCollapse => { state.add_dialog.toggle_collapse_torrents(); }
Message::DetailsFilesScroll(off) => { state.details.files_scroll_offset = off; }
```

### 9. Validation

```bash
cargo build
cargo clippy --workspace   # no warnings
cargo fmt --check
```
Manual (`cargo run --`):
- Add dialog → Torrent tab with a large torrent (thousands of files): first render near-instant; scroll is smooth; rows render only near the viewport; master checkbox + per-row checkboxes behave as before; collapse button at far right collapses/expands all dirs and resets scroll; subtitle (total size) still renders.
- Details → Files tab on a torrent: works the same, scroll tracked, **no collapse button**, disabled (completed/removed) tree greys rows and the header button is absent.

## Risks / notes

- **ROW_PITCH accuracy**: rows are single-line; 24.0 keeps content height `total × ROW_PITCH`. If visual spacing looks wrong, tune `ROW_PITCH` (and the top/bottom spacers use the same constant, so the scrollbar stays consistent). Any row with `Length::Fill` height must be avoided — every row uses `Length::Fixed(ROW_PITCH)`.
- **Scroll latency**: window follows the stored offset updated by `on_scroll`; worst case one-frame lag while scrolling. Reset offset to 0 on torrent load/collapse to avoid stale windows.
- **Icon names** `chevrons-down-up` / `chevrons-up-down` exist in Lucide; the build panics on unknown names, so a typo fails CI immediately.
- **Two O(n) walks per frame remain** (master checkbox `all_indices` in `torrent_file_list`, aggregates in `file_tree`); acceptable, consistent with current behavior.
- **Empty tree**: `total == 0` → window empty, spacers zero-height; button shows "expand" icon (no-op), harmless.
