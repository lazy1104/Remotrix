# Plan: Extract reusable stateful `PathPicker` component into `ui/components/`

## Goal
Turn `src/ui/path_picker.rs` (a stateless free function tightly coupled to `Message`/`PathPickerId`) into a reusable, message-generic stateful component struct, and relocate it to a new `src/ui/components/` folder that will host future generic UI components.

## Background / current state
- `src/ui/path_picker.rs` is a single `pub fn view(...)` producing concrete `Message` variants (`CopyPath`, `BrowsePath`, `TogglePathHistory`, `SelectPathHistory`, `ClosePathHistory`). It is read-only for editing (the `text_input` has no `on_input`).
- 3 call sites:
  - `src/ui/add_dialog.rs:81` — torrent picker: `id=Some(Torrent)`, no history.
  - `src/ui/add_dialog.rs:103` — save-dir picker: `id=Some(SaveDir)`, history from `path_history["save_dir"]`.
  - `src/ui/settings_page.rs:212` — download-dir picker: `id=Some(DownloadDir)`, history from `path_history["download_dir"]`.
  - `src/ui/settings_page.rs:490-513` — 3 read-only pickers via `labeled_readonly` (`id=None`, copy-only, no browse/history) for engine data dir / session file / log file.
- `PathPickerId` (`src/message.rs:9`) carries `history_key()` + `is_folder()`. App state:
  - `Remotrix.path_history_open: Option<PathPickerId>` (`src/app.rs:65`) is the open/close state, reset on `SetSettingsCategory`/`OpenAddDialog`/`CancelAdd`.
  - `Settings.path_history: HashMap<String, Vec<String>>` (`src/config.rs:275`) is the persisted history, mutated by `Settings::record_path`.
  - `pick_path(id)` (`src/app.rs:1274`) runs the `rfd` dialog and yields `Message::PathPicked(id, Option<PathBuf>)`.
  - `apply_path(state, id, p)` (`src/app.rs:1254`) mutates `settings.download_dir` / `add_dialog.save_dir` / `add_dialog.torrent_path` and records+persistence history.

## Design decisions
- **Component owns UI-local state**: value (`String`), `mode` (Folder | File | ReadOnly), `history_open: bool`, `show_history: bool`. The **history list stays owned by `Settings`** (it is shared/persisted config) and is borrowed into `view` each frame — the component only renders it, never mutates it. This avoids duplicating persistence plumbing while still encapsulating the transient open/close + displayed value.
- **Message-generic via `map` closure**: `view<M>` takes `map: impl Fn(PathPickerEvent) -> M`. Callers decide how to wrap events into their own `Message`.
- **Component emits its own event enum**; app routes pure-UI events (toggle/dismiss history) into `picker.update(...)` and handles surfaced `PathPickerAction`s (copy -> clipboard, browse -> `pick_path` Task, select -> `apply_path`).
- **Read-only pickers** are constructed ad-hoc each frame (`PathPicker::read_only(&str)`); their `map` closure simply maps `Copy(s) -> Message::CopyPath(s)` and everything else to `Message::Noop`, so they need no id/state in app.
- **Value as source of truth moves into the component**: `AddDialogState` and the new `SettingsUiState` store `PathPicker` instances and expose value via a getter; persisted fields (`add_dialog.save_dir`, `settings.download_dir`) are updated in lock-step by the app on every change (select / rfd pick) by also calling `picker.set_value(...)` is NOT needed because the component owns the value — instead the app reads `picker.value()` when it needs the path (e.g. at `AddDownload`, `apply_path`). `settings.download_dir` is still the persisted mirror: `apply_path` updates both `settings.download_dir` and `picker.set_value`.

  Refinement (chosen): keep `settings.download_dir` / `add_dialog.save_dir` as the authoritative persisted values AND mirror into the picker via `set_value`. This is simpler/safer than threading `picker.value()` reads through every existing consumer, and keeps `config::save` unchanged. The component's `value` is purely the rendered string, synced on every change. The component still owns `history_open` (the only genuinely UI-local state) plus mode/show_history config.

  Final ownership split:
  - Component owns: `history_open: bool`, `mode`, `show_history: bool`, and a `value: String` mirror that the app keeps in sync via `set_value`.
  - App/persist owns: the actual path (`Settings.download_dir`, `AddDialogState.save_dir`, `torrent_path`), the history `Vec`, and `PathPickerId`.

- **Removed app messages**: `BrowsePath`, `SelectPathHistory`, `TogglePathHistory`, `ClosePathHistory` are folded into the new `Message::PathPicker(PathPickerId, PathPickerEvent)`. Keep `PathPicked` (rfd result) and `CopyPath` (read-only clipboard). Keep `PathPickerId`.

## New component API (`src/ui/components/path_picker.rs`)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerMode { Folder, File, ReadOnly }

#[derive(Debug, Clone)]
pub enum PathPickerEvent {
    ToggleHistory,
    DismissHistory,
    SelectHistory(PathBuf),
    Browse,
    Copy(String),
}

#[derive(Debug, Clone)]
pub enum PathPickerAction {
    Copy(String),
    Browse,
    Select(PathBuf),
}

pub struct PathPicker {
    value: String,
    mode: PickerMode,
    show_history: bool,
    history_open: bool,
}

impl PathPicker {
    pub fn folder(value: impl Into<String>, show_history: bool) -> Self;
    pub fn file(value: impl Into<String>) -> Self;          // show_history = false
    pub fn read_only(value: impl Into<String>) -> Self;     // mode = ReadOnly
    pub fn set_value(&mut self, v: impl Into<String>);
    pub fn value(&self) -> &str;
    pub fn close_history(&mut self);                        // history_open = false
    pub fn is_history_open(&self) -> bool;
    /// Mutate UI-local state; returns an action the app must handle (if any).
    pub fn update(&mut self, event: PathPickerEvent) -> Option<PathPickerAction>;
    pub fn view<'a, M>(
        &self,
        fluent: &'a Fluent,
        theme: &'a iced::Theme,
        history: &'a [String],
        map: impl Fn(PathPickerEvent) -> M + 'a,
    ) -> Element<'a, M>
    where
        M: Clone + 'a;
}
```
`update` semantics:
- `ToggleHistory` → flip `history_open` (only when not ReadOnly), returns `None`.
- `DismissHistory` → `history_open=false`, returns `None`.
- `SelectHistory(p)` → `history_open=false`, returns `Some(Select(p))`.
- `Browse` → returns `Some(Browse)` (no state change).
- `Copy(s)` → returns `Some(Copy(s))` if non-empty.

## Implementation steps

1. **Create `src/ui/components/` module.**
   - Add `src/ui/components/mod.rs` with `pub mod path_picker;`.
   - In `src/ui/mod.rs`: replace `pub mod path_picker;` with `pub mod components;`. Add `pub use components::path_picker;` re-export if convenient (optional; prefer explicit paths).

2. **Rewrite the component** at `src/ui/components/path_picker.rs` per the API above. Port the existing widget tree from the old free function verbatim, swapping `Message::CopyPath(...)`/`BrowsePath(pid)`/etc. buttons to `map(PathPickerEvent::Copy(...))` / `map(PathPickerEvent::Browse)`, gated on `mode != ReadOnly`. Hide browse + history controls when `mode == ReadOnly`; show history button only when `show_history && mode != ReadOnly`. Drop the `id: Option<PathPickerId>` parameter (no longer needed — callers encode id via the `map` closure). Keep `icon::copy/folder_open/folder_clock`, `theme::style::*`, `iced_aw::drop_down::DropDown`, tooltips as-is.

3. **Handle rfd pick** remains app-side: keep `pick_path(id)` -> `Message::PathPicked(id, Option<PathBuf>)` and `apply_path`. In `apply_path`, after mutating persisted fields, also call the matching `picker.set_value(...)` and `picker.close_history()`.

4. **Route `PathPickerId` -> owning picker.** Add accessors on `Remotrix` (or helpers):
   - `PathPickerId::Torrent` / `SaveDir` -> `&mut add_dialog.torrent_picker` / `save_picker`.
   - `PathPickerId::DownloadDir` -> `&mut settings_ui.download_picker`.

5. **Update `AddDialogState`** (`src/ui/add_dialog.rs`):
   - Replace fields `save_dir: PathBuf` -> `save_picker: PathPicker`, `torrent_path: Option<PathBuf>` -> `torrent_picker: PathPicker`.
   - `new(default_dir)` constructs `save_picker = PathPicker::folder(default_dir.to_string_lossy(), true)`, `torrent_picker = PathPicker::file(String::new())`.
   - `open(...)` resets both pickers via `set_value`; `can_submit()` reads `save_picker.value()` / `torrent_picker.value()`; `AddDownload` path reads `save_picker.value()`/`torrent_picker.value()`.
   - `view` calls `save_picker.view(fluent, theme, hist_save, |e| Message::PathPicker(PathPickerId::SaveDir, e))` and `torrent_picker.view(fluent, theme, &[], |e| Message::PathPicker(PathPickerId::Torrent, e))`.

6. **Introduce `SettingsUiState`** (new struct, lives in `src/ui/settings_page.rs` or `src/app.rs`): `{ download_picker: PathPicker }`. Construct once with `PathPicker::folder("", true)`; on settings load, `set_value(settings.download_dir.to_string_lossy())`.
   - Add field `settings_ui: SettingsUiState` to `Remotrix`; init in `Remotrix::new`.
   - `settings_page::view` signature: replace `path_history: &[String]>`+`path_history_open: Option<PathPickerId>` with `settings_ui: &SettingsUiState`; `download_view` calls `settings_ui.download_picker.view(fluent, theme, download_hist, |e| Message::PathPicker(PathPickerId::DownloadDir, e))`.
   - `labeled_readonly` constructs `PathPicker::read_only(value.to_string()).view(fluent, theme, &[], |e| match e { PathPickerEvent::Copy(s) => Message::CopyPath(s), _ => Message::Noop })`.

7. **Update `Message`**, `src/message.rs`:
   - Remove variants: `BrowsePath`, `SelectPathHistory`, `TogglePathHistory`, `ClosePathHistory`.
   - Add `PathPicker(PathPickerId, PathPickerEvent)`.
   - Keep `PathPicked(PathPickerId, Option<PathBuf>)`, `CopyPath(String)`.

8. **Update `app.rs` `update`**, `src/app.rs`:
   - Remove handlers for the 4 deleted variants.
   - Add `Message::PathPicker(id, event)` handler: get the owning picker via helper, call `picker.update(event)`; on returned action: `Copy(s)` -> `iced::clipboard::write(s)`; `Browse` -> `return pick_path(id)`; `Select(p)` -> `apply_path(state, id, p)` then `Task::none()`.
   - `Message::PathPicked(id, maybe)`: same as today (apply_path + picker.set_value + close already happen in `apply_path`). Remove the old `state.path_history_open = None`.
   - `SetSettingsCategory`/`OpenAddDialog`/`CancelAdd`: replace `state.path_history_open = None` with closing the relevant pickers' histories (`add_dialog.save_picker.close_history()`, `add_dialog.torrent_picker.close_history()`, `settings_ui.download_picker.close_history()` as appropriate per event).
   - Remove `Remotrix.path_history_open` field.
   - At settings-load / init, ensure `settings_ui.download_picker.set_value(settings.download_dir.to_string_lossy())`.

9. **`apply_path`**: after setting `settings.download_dir` / `add_dialog.save_dir` / `add_dialog.torrent_path`, also call `picker.set_value(...)` and `picker.close_history()` on the matching instance, then existing `record_path` + `config::save`.

10. **Apply fallback review**: ensure `path_picker::view` old fn import `crate::ui::path_picker` in `add_dialog.rs`/`settings_page.rs` is switched to `crate::ui::components::path_picker::PathPicker`.

## Affected files
- `src/ui/components/mod.rs` (NEW)
- `src/ui/components/path_picker.rs` (NEW, replaces old `src/ui/path_picker.rs`)
- `src/ui/path_picker.rs` (DELETE)
- `src/ui/mod.rs` (module swap)
- `src/message.rs` (variant churn + new event enum lives in component, imported via `Message::PathPicker`)
- `src/app.rs` (handler rewrites, remove `path_history_open`, add `SettingsUiState`, helpers)
- `src/ui/add_dialog.rs` (stateful pickers)
- `src/ui/settings_page.rs` (SettingsUiState, readonly via component)

## Risks / watch
- `iced` view with generic `M` and `map` closure must satisfy lifetimes (`Element<'a, M>`, `M: Clone`). The old `Message: Clone` callers are fine; verify `iced_aw::drop_down::DropDown` accepts `M`.
- Borrow-check: `picker.update(...)` then `apply_path(state, ...)` which also mutates the same picker via `set_value`. Sequence as: `let action = picker_mut(state, id).update(event);` then drop borrow before `apply_path(state, id, p)` (apply_path re-borrows the picker to set_value). Split into two statements to avoid double mutable borrow.
- Don't break `config::save` / persistence plumbing; history stays in `Settings`.
- Read-only pickers constructed each frame must remain stateless (no history) — confirm `update` is never called for them (their `map` only emits `Copy`, which bypasses `picker.update`). Actually readonly copy currently goes straight to `Message::CopyPath` via the map, so `picker.update` is never invoked — no state to preserve. Good.
- `clippy` no warnings; `cargo fmt` clean.

## Validation
- `cargo build`
- `cargo clippy --workspace`
- `cargo fmt --check`
- Manual: add dialog torrent/browse, save-dir history toggle/select, settings download-dir browse + history, copy buttons on all pickers (incl. readonly engine paths), and verify rfd pick updates the field and persists.

## Out of scope
- Making the text value editable via keyboard (input is read-only display today).
- Extracting other existing UI files (category_bar, dialog widgets) into `components/` now — only establish the folder; future components go here.
- Refactoring `PathPickerId` history-key/is_folder helpers (unchanged).