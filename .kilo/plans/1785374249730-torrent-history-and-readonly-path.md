# Torrent History Removal + Read-Only Path Picker for Advanced Settings

## Goal
1. Add-dialog torrent file picker must **not** show a history dropdown and must **not** persist torrent paths into `settings.path_history`.
2. Settings → Advanced 的三个只读路径（EngineDataDir / EngineSessionFile / EngineLogFile）改用 `path_picker` 组件，**只保留复制按钮**（无浏览、无历史）。

## Files to edit
- `src/ui/add_dialog.rs` — torrent picker: drop history
- `src/ui/settings_page.rs` — reimplement `labeled_readonly` to render `path_picker` (copy-only)
- `src/app.rs` — `apply_path`: stop recording torrent path history

---

## 1. add_dialog.rs — torrent picker no history

Current (lines 74-93):
```rust
let hist_torrent: &[String] = path_history
    .get("torrent")
    .map(|v| v.as_slice())
    .unwrap_or(&[]);
let torrent_row = column![]
    .spacing(4)
    .push(text(fluent.get(Tr::OrTorrent)).size(12).style(theme::style::text::secondary))
    .push(path_picker::view(
        fluent, theme, torrent_str,
        Some(PathPickerId::Torrent),
        true,                                          // show_history
        path_history_open == Some(PathPickerId::Torrent),
        hist_torrent,
    ));
```

Change:
- Delete the `hist_torrent` lookup block (lines 74-77).
- In the `path_picker::view` call set `show_history = false`, `history_open = false`, `history = &[]`:
```rust
.push(path_picker::view(
    fluent, theme, torrent_str,
    Some(PathPickerId::Torrent),
    false,
    false,
    &[],
))
```
The browse (`folder_open`) button stays — only history is removed.
No need to touch the `save_row` picker; it keeps history.

## 2. settings_page.rs — Advanced paths via path_picker (copy-only)

`labeled_readonly` is used **only** by the 3 advanced paths (lines 491, 498, 504). Reimplement it to render the `path_picker` component in read-only mode.

New `labeled_readonly` (replaces lines 676-688):
```rust
fn labeled_readonly<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    label: String,
    value: String,
) -> Element<'a, Message> {
    row![]
        .push(text(label).size(13).width(Length::Fixed(200.0)))
        .push(path_picker::view(
            fluent, theme, &value,
            None,    // id None → no browse button, no history button
            false,   // show_history false
            false,
            &[],
        ))
        .height(Length::Fixed(36.0))
        .align_y(Alignment::Center)
        .into()
}
```

With `id: None`, `path_picker::view` renders only the text field + copy button (see `path_picker.rs` lines 50-61, gated browse/history blocks at 64-96 and dropdown at 104-126 are skipped). Copy fires `Message::CopyPath(value)` when non-empty — already handled in `app.rs:250` via `iced::clipboard::write`.
The text field is the standard `text_input` with no `on_input`, so it is display-only (same as the DownloadDir picker today). The three paths are always non-empty (returned by `config::aria2_bin_dir` / `session_dir` / `log_dir`), so the copy button is always enabled.

Update the 3 callers in `advanced_view` (lines 490-508) to pass `fluent, theme`:
```rust
if let Some(dir) = crate::config::aria2_bin_dir() {
    engine_rows.push(labeled_readonly(
        fluent, theme,
        fluent.get(Tr::EngineDataDir),
        dir.to_string_lossy().to_string(),
    ));
}
if let Some(path) = crate::config::session_dir() {
    let sf = path.join("session.txt");
    engine_rows.push(labeled_readonly(
        fluent, theme,
        fluent.get(Tr::EngineSessionFile),
        sf.to_string_lossy().to_string(),
    ));
}
if let Some(dir) = crate::config::log_dir() {
    engine_rows.push(labeled_readonly(
        fluent, theme,
        fluent.get(Tr::EngineLogFile),
        dir.to_string_lossy().to_string(),
    ));
}
```
(`fluent` and `theme` are already parameters of `advanced_view`.)

## 3. app.rs — stop recording torrent history

`apply_path` (app.rs:1254-1271) currently calls `record_path` unconditionally before the `match`. Move the call into the `DownloadDir` and `SaveDir` arms only, omitting `Torrent`:
```rust
fn apply_path(state: &mut Remotrix, id: PathPickerId, p: PathBuf) {
    let s = p.to_string_lossy().to_string();
    match id {
        PathPickerId::DownloadDir => {
            state.settings.record_path(id.history_key(), &s);
            state.settings.download_dir = p;
            state.settings_dirty = true;
        }
        PathPickerId::SaveDir => {
            state.settings.record_path(id.history_key(), &s);
            state.add_dialog.save_dir = p;
            config::save(&state.settings);
        }
        PathPickerId::Torrent => {
            state.add_dialog.torrent_path = Some(p);
            config::save(&state.settings);
        }
    }
}
```
`id.history_key()` (`message.rs:20`) maps `Torrent` → `"torrent"`; that key is no longer written.

---

## Out of scope
- Existing users may already have a stale `path_history["torrent"]` entry saved in their `settings.json`. It is now unused (never displayed, never written) and harmless; no migration/cleanup is performed. Optionally scrub on load if desired — left out to keep the change minimal.

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` (no warnings; confirm no now-unused imports in add_dialog.rs after removing `hist_torrent` — none expected since `path_history`/`PathPickerId` still used)
- `cargo build`
- Manual: open Add dialog, pick a torrent file → no history dropdown button appears; reopen dialog → torrent field empty, no history. Open Settings → Advanced → three paths render as a text field with only a copy icon; clicking copy writes the path to clipboard; no browse/history icons present.