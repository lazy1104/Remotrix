# Plan: Add-Dialog Tabs + Torrent Drag-and-Drop Upload

## Goal
Split the "New Download" dialog (`src/ui/add_dialog.rs`) into two tabs:
- **链接任务 (Link)**: URL multi-line editor, rename field, common options.
- **种子任务 (Torrent)**: a drop-zone upload widget (same size as the URL editor, ~120px tall) that accepts a `.torrent` file by drag-and-drop, or by click to open the native file picker.

Common options (save dir, split, advanced) stay shared below the tab content.

## Context / Constraints (verified in code)
- iced 0.14 (`iced_core-0.14.0`): no dashed-border support; no widget-level drag events. Window-level file events exist:
  - `iced::event::Event::Window(iced::window::Event::FileHovered(PathBuf))`
  - `iced::event::Event::Window(iced::window::Event::FileDropped(PathBuf))`
  - `iced::event::Event::Window(iced::window::Event::FilesHoveredLeft)`
  - **Wayland: not implemented** — click-to-browse is the fallback.
- Existing tab pattern: `details_dialog.rs` builds a `row` of `button(text(...)).padding(PADDING_TAB).style(theme::style::button::sidebar_icon(active))`.
- Existing component-with-state pattern: `path_picker.rs` (struct + `update(&mut, Event) -> Option<Action>` + `view(...)`), registered in `src/ui/components/mod.rs`.
- `icon::arrow_up()` (arrow-up-from-line) and `icon::x()` already exist in `fonts/icons.toml` / generated `src/ui/icon.rs`.
- Toasts: `spawn_toast(state, ToastKind::Warning, msg, Some(Duration::from_secs(...)), false)` in `app.rs`.
- `PathPickerId::Torrent` is reused for the native file dialog only (keep it; `pick_path` at `src/app.rs:1996` already opens a `.torrent`-filtered dialog).
- Build checks (no warnings allowed): `cargo build`, `cargo clippy --workspace`, `cargo fmt --check`.

## Ordered Tasks

### 1. i18n additions
**`src/i18n.rs`** — add `Tr` variants (in the enum near line 77) and key mappings (near line 274):
- `TabUrl` → `"tab-url"`
- `TabTorrent` → `"tab-torrent"`
- `DropTorrentHint` → `"drop-torrent-hint"`
- `DropTorrentActive` → `"drop-torrent-active"`
- `Remove` → `"remove"`
- `InvalidTorrent` → `"invalid-torrent"`

**`i18n/locales/zh-CN/main.ftl`**:
- `tab-url = 链接任务`
- `tab-torrent = 种子任务`
- `drop-torrent-hint = 拖入 .torrent 文件，或点击选择文件`
- `drop-torrent-active = 松开以添加种子文件`
- `remove = 移除`
- `invalid-torrent = 仅支持 .torrent 文件`

**`i18n/locales/en/main.ftl`**:
- `tab-url = Link`
- `tab-torrent = Torrent`
- `drop-torrent-hint = Drag a .torrent file here, or click to select`
- `drop-torrent-active = Release to add the torrent file`
- `remove = Remove`
- `invalid-torrent = Only .torrent files are supported`

Leave the old `or-torrent` key/variant in place (now unused, harmless).

### 2. `src/message.rs`
- Add:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum AddTab { Url, Torrent }
  ```
- Add messages to `Message`:
  - `SelectAddTab(AddTab)`
  - `TorrentUpload(crate::ui::components::torrent_upload::TorrentUploadEvent)`
  - `FileHovered(PathBuf)`
  - `FileDropped(PathBuf)`
  - `FilesHoveredLeft`
- Keep `PathPickerId::Torrent`.

### 3. New component `src/ui/components/torrent_upload.rs`
Mirror the `PathPicker` pattern. Register in `src/ui/components/mod.rs` (`pub mod torrent_upload;`).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorrentUploadEvent { Browse, Clear }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TorrentUploadAction { Browse }

#[derive(Debug, Clone)]
pub struct TorrentUpload { path: String, dragging: bool }
```
- Methods: `new()`, `set_path(&mut self, impl Into<String>)` (also clears `dragging`), `clear()`, `path() -> &str`, `is_empty()`, `set_dragging(bool)`, `is_dragging()`, `update(&mut self, TorrentUploadEvent) -> Option<TorrentUploadAction>` (Browse → `Some(Browse)`, Clear → `clear(); None`).
- Static helper: `pub fn is_torrent_file(p: &std::path::Path) -> bool` — `extension()` matches `"torrent"` case-insensitively.
- `view<'a, M>(&'a self, fluent, theme, map: impl Fn(TorrentUploadEvent) -> M)` → `Element<'a, M>`:
  - **Empty state**: a `container` `Length::Fixed(120.0)` high, `Length::Fill` wide, wrapped in `mouse_area` with `on_press(map(Event::Browse))`; content centered column of `icon::arrow_up().size(FONT_HERO)` + hint text (`DropTorrentHint`, or `DropTorrentActive` when `dragging`). Container style = `theme::style::drop_zone(self.dragging)`.
  - **Filled state**: same size container showing the file's basename + full path (secondary style) + a "replace" browse button and an `x` clear button (`map(Event::Browse)` / `map(Event::Clear)`). Clear/replace buttons sit inside; the whole zone is *not* clickable in this state (avoids nested-click conflicts).

### 4. `src/ui/theme.rs`
Add in `pub mod style` (top level of the module, next to `grouped_frame_state`):
```rust
pub fn drop_zone(active: bool) -> impl Fn(&iced::Theme) -> iced::widget::container::Style
```
Solid border (1.0), radius `RADIUS_BUTTON`, background `background.weak`. When `active`: border + text tinted with `primary.base.color` (e.g. `Color::from_rgba(accent.r, accent.g, accent.b, 0.18)` background like `active_filter`); else default border via `super::border_color(t)`.

### 5. `src/ui/add_dialog.rs`
- **State**: add `pub active_tab: AddTab`; replace `pub torrent_picker: PathPicker` with `pub torrent_upload: TorrentUpload`.
- **Methods**:
  - `new`: `active_tab: AddTab::Url`, `torrent_upload: TorrentUpload::new()`.
  - `open`: reset `active_tab = AddTab::Url`, `torrent_upload.clear()` (+ drag flag off).
  - `close`: unchanged behavior (visible=false). Remove `torrent_picker.close_history()`.
  - `open_with`: after `open(...)`, for `Torrent(path)` payload set `torrent_upload.set_path(path)` and `active_tab = AddTab::Torrent`; for `Urls(_)` leave default URL tab.
  - `can_submit`: per active tab — Url: `urls non-empty && save_dir non-empty`; Torrent: `!torrent_upload.is_empty() && save_dir non-empty`.
  - `has_torrent`: use `torrent_upload.is_empty()`.
- **view**:
  - Remove the `torrent_row` (the "或选择 .torrent 文件" + PathPicker row).
  - Build a tab bar (buttons with `PADDING_TAB` + `sidebar_icon(active)` style, emitting `Message::SelectAddTab`) placed above the scrollable body, followed by `rule::horizontal(1)` — same layout as `details_dialog.rs:75-91`.
  - Build shared sections once: `save_row`, `split_input`, `advanced_checkbox`, `advanced_form` (unchanged).
  - Body = scrollable column (`height: Length::Fixed(400.0)`, keep):
    - Url tab: `url_input`, `rename_row`, then shared sections.
    - Torrent tab: `torrent_upload.view(fluent, theme, |e| Message::TorrentUpload(e))`, then shared sections.
  - Footer Download button unchanged (enabled by `can_submit`).
  - Drop `theme`/`path_history` params only if unused (they are still used by save_picker/history — keep).

### 6. `src/app.rs`
- **`update`** new arms:
  - `Message::SelectAddTab(tab)` → `state.add_dialog.active_tab = tab`.
  - `Message::TorrentUpload(event)` → call `add_dialog.torrent_upload.update(event)`; on `Some(TorrentUploadAction::Browse)` return `pick_path(PathPickerId::Torrent)`.
  - `Message::FileHovered(_)` → if `add_dialog.is_visible() && active_tab == AddTab::Torrent` set `torrent_upload.set_dragging(true)`.
  - `Message::FilesHoveredLeft` → if visible, `set_dragging(false)`.
  - `Message::FileDropped(path)` → if visible: `set_dragging(false)`; if `torrent_upload::is_torrent_file(&path)` → `set_path(path)`, `active_tab = AddTab::Torrent`; else spawn warning toast `Tr::InvalidTorrent` (via `spawn_toast`).
- **`Message::OpenAddDialog` / `CancelAdd`** (lines 360-371): remove the `torrent_picker.close_history()` calls.
- **`Message::AddDownload`** (line 443): read path from `state.add_dialog.torrent_upload.path()` instead of `torrent_picker.value()`.
- **`picker_mut`** (line 1886): remove the `PathPickerId::Torrent` arm (component is not a `PathPicker`).
- **`apply_path`** (line 1917): Torrent arm → `state.add_dialog.torrent_upload.set_path(p.to_string_lossy())`; drop the `config::save` if not already expected (keep current side effects otherwise).
- **`subscription`** (line 1839): add a `listen_with` stream (or extend the existing `focus` one) mapping the three window file events to the new messages (clone the `PathBuf`). Add to the `Subscription::batch` list. Only these window events, no other behavioral change.

### 7. Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual: open dialog → tabs switch and data is preserved per tab; drag a `.torrent` onto the Torrent tab → path fills + submit enabled; drag a non-`.torrent` file → warning toast; click empty drop zone → native file picker; clear button resets; submit from either tab works.

## Risks / Notes
- Drag-over highlight is **window-wide** (iced has no widget-level drag events): the drop zone highlights whenever a file hovers anywhere over the window while the Torrent tab is active and the dialog is open.
- **Wayland**: no window-level drop events → click-to-browse is the only way there (accepted limitation).
- Dashed borders unavailable in iced 0.14 → solid accent border for the active/drag state.
- Multi-file drop: each `FileDropped` fires separately; last valid `.torrent` wins (each call overwrites the path).
- Dropping a directory → `extension()` is `None` → rejected with the warning toast.
