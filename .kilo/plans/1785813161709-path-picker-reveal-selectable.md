# PathPicker: reveal-in-file-manager button + selectable path text

## Goal
Enhance `src/ui/components/path_picker.rs` so every picker (Folder, File, ReadOnly) gets:
1. A new icon button that opens the value in the file manager — the folder itself if it's a directory, otherwise the value's parent directory.
2. Mouse-selectable path text (drag to select → Ctrl+C) by making the internal `text_input` focusable.

## Confirmed decisions
- **Icon**: new Lucide icon `folder-search` (folder + magnifier), distinct from `folder-open` (used by the Browse button). Add `folder_search = "folder-search"` to `fonts/icons.toml`; `build.rs` regenerates `src/ui/icon.rs` and `fonts/lucide.ttf` automatically.
- **Read-only pickers**: reveal button included (EngineDataDir / session file / LogLocation rows in Settings). Requires new `TaskMsg::OpenFolder(PathBuf)` variant + handler.

## Design
- **Event → Action**: `PathPickerEvent::Open` → `PathPickerAction::Open(PathBuf)` (carries the raw value; the app resolves dir vs. parent at runtime via `is_dir()`, which also covers ReadOnly rows that may hold a file).
- **Selection**: add `.on_input(...)` to the `text_input` so iced treats it as enabled/focusable (with `on_input: None` the widget is `Disabled` and cannot focus → no selection). New event `PathPickerEvent::Changed(String)` is a no-op in `update()`. Because callers never mutate their stored value, any typed edit reverts on the next render — the field stays effectively read-only while being selectable. Caveat: a caret appears on focus; Ctrl+A/Ctrl+C and drag-select all work.
- **Open target helper** (shared by both handlers): if value is empty → no-op; if `is_dir()` → open value; else open `parent()` (fall back to value when parent is empty/none).

## Implementation tasks

### 1. `fonts/icons.toml`
Add a line, e.g. after `folder_open`:
```toml
folder_search = "folder-search"
```
(`src/ui/icon.rs` + `fonts/lucide.ttf` are auto-regenerated on next build; do not hand-edit them.)

### 2. `src/ui/components/path_picker.rs`
- Add variants:
  - `PathPickerEvent::Changed(String)`
  - `PathPickerEvent::Open`
- Add `PathPickerAction::Open(PathBuf)`.
- `update()`:
  - `PathPickerEvent::Changed(_) => None`
  - `PathPickerEvent::Open => Some(PathPickerAction::Open(PathBuf::from(self.value.clone())))`
- `view()`:
  - `text_input` gains `.on_input(move |s| map(PathPickerEvent::Changed(s)))` (all modes).
  - Insert a reveal button between the copy button and the browse button (editable modes) / after copy (ReadOnly): `icon::folder_search().size(FONT_ICON).color(text_secondary)`, `theme::style::button::grouped_icon(false)`, height `Length::Fill`, tooltip `text(fluent.get(Tr::ShowInFolder))` via `tooltip::standard(...)`; attach `.on_press(map(PathPickerEvent::Open))` only when `!self.value.is_empty()` (mirror the copy button's disabled pattern at line 169-171).

### 3. `src/message.rs`
Add to `TaskMsg`: `OpenFolder(PathBuf)`.

### 4. `src/app.rs`
- Add free helper (e.g. near `pick_path`, ~line 3231):
  ```rust
  fn open_path_in_manager(p: PathBuf) -> Task<Message> {
      if p.as_os_str().is_empty() {
          return Task::none();
      }
      let target = if p.is_dir() {
          p
      } else {
          p.parent()
              .filter(|q| !q.as_os_str().is_empty())
              .map(Path::to_path_buf)
              .unwrap_or(p)
      };
      Task::perform(async move { let _ = open::that(&target); }, |_| Message::Noop)
  }
  ```
- In the `Message::Add(AddMsg::PathPicker(id, event))` handler (line ~865-879), add:
  ```rust
  Some(PathPickerAction::Open(p)) => {
      return open_path_in_manager(p);
  }
  ```
- After `Message::Task(TaskMsg::CopyPath(s))` (line ~886), add:
  ```rust
  Message::Task(TaskMsg::OpenFolder(p)) => {
      return open_path_in_manager(p);
  }
  ```

### 5. `src/ui/settings_page.rs`
- Add `use std::path::PathBuf;` (currently only `std::collections::HashMap` / `std::sync::Mutex` are imported).
- In `labeled_readonly` (line ~1648), extend the map closure:
  ```rust
  PathPickerEvent::Open => Message::Task(TaskMsg::OpenFolder(PathBuf::from(value.to_string()))),
  ```
  (`Changed(_)` is already swallowed by the existing `_ => Message::Noop` arm.)

## No changes needed
- `add_dialog.rs` and the DownloadDir / ED2K pickers already route all `PathPickerEvent`s through `AddMsg::PathPicker` → `update()` → action; `Open` flows through the new app.rs arm.
- i18n: `Tr::ShowInFolder` already exists and is translated (used by `task_list.rs:355`).

## Validation
- `cargo build` (regenerates icon module/font; confirms `folder_search` glyph resolves).
- `cargo clippy --workspace` (no warnings).
- `cargo fmt --check`.
- Manual: in Settings, verify reveal button opens the download dir; in add-dialog, verify it opens the parent of a picked file (ED2K list); on an Engine/Logs read-only row verify a file value opens its parent dir. Verify drag-select + Ctrl+C copies the path text.

## Risks / caveats
- Focusable input shows a caret and accepts keystrokes that visually revert next frame (iced `draw` renders the caller-supplied value). This is the only practical way to get mouse selection in iced 0.14 (no read-only selectable text widget).
- `is_dir()` is evaluated on the UI thread (cheap syscall, same as existing `OpenTaskFolder` handler at app.rs:2611).
- Build script mutates `src/ui/icon.rs` + `fonts/lucide.ttf`; commit those regenerated artifacts.
