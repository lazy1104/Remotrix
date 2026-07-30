# Reusable Path-Picker Component

## Goal
Create a reusable path-selection component (`src/ui/path_picker.rs`) and replace all existing path UIs with it. Layout: `[leading path icon?] [read-only selectable input] [copy icon btn] [browse icon btn] [history icon btn?]` — buttons are flush (no gaps), the group has button-like rounded corners on both outer sides. History is per-picker (unique key), max 10 entries, shown via a dropdown.

## Confirmed decisions (from user + codebase)
- **Scope (replace all 4):** Settings "Download folder" row, Add-dialog "Save to" row, Add-dialog torrent picker, Advanced readonly path displays (EngineDataDir/SessionFile/LogFile).
- **History storage:** `settings.json` — new `Settings.path_history: HashMap<String, Vec<String>>`, persisted via existing `config::save`.
- **Read-only input:** `text_input` **without** `on_input`/`on_paste`. Verified in `iced_widget 0.14.2/src/text_input.rs`: with `on_input == None` the field is non-editable but still focusable, selectable (click/drag/double/triple-click), and Ctrl+C copies (line 914 copy handler is not gated on `on_input`). Only caveat: mouse cursor shows `Idle` (arrow) instead of I-beam (cosmetic only). The copy icon button is the guaranteed one-click copy path.
- **Pre-existing bug to fix:** Settings "Download folder" Browse currently sends `SettingChanged(DownloadDir,"")` -> `pick_folder(FileKind::SaveDir)` -> `FilePicked(SaveDir, p)` which writes `add_dialog.save_dir` (NOT `settings.download_dir`). Known bug documented in `.kilo/plans/1785306524269-confirmation-dialogs-plan.md:143`. The new routing fixes this.
- **API facts:** `iced 0.14.0`, `iced_widget 0.14.2`, `iced_aw 0.14.1`. `iced::border::Radius` supports per-corner via `Radius::right(v)` etc. (iced_core/border.rs). `iced_aw::widget::drop_down::DropDown::new(underlay, overlay, expanded: bool)` + `.on_dismiss(msg)` + `.width(...)`. Overlay width defaults to `underlay_bounds.width` when `.width` is None (drop_down.rs layout) — so wrapping the whole group as the DropDown underlay makes the history list match the group width automatically.

## Design: the component (`src/ui/path_picker.rs`)

```rust
pub fn view<'a>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    value: &'a str,
    id: Option<PathPickerId>,   // None => read-only mode (copy only, no browse/history)
    show_path_icon: bool,
    show_history: bool,
    history_open: bool,
    history: &'a [String],
) -> Element<'a, Message>
```

Rules:
- `id == None` → read-only: render `[path icon?] input copy`. No browse, no history. (Used by Advanced readonly paths.)
- `id == Some` → interactive: `[path icon?] input copy browse [history?]`.
- Leading path icon = `icon::folder_open()` (size 15, secondary color) when `show_path_icon`.
- Input = `text_input("", value)` with NO `.on_input`/`.on_paste`, `.style(theme::style::input::grouped)`, `.width(Length::Fill)`, `.padding([0, 10])`, `.size(13)`.
- Copy btn = `icon::copy()`; `.on_press(Message::CopyPath(value.to_string()))` only when `!value.is_empty()` (else disabled, no on_press). Tooltip `Tr::Copy`.
- Browse btn = `icon::folder_open()`; `.on_press(Message::BrowsePath(id.unwrap()))` (interactive only). Tooltip `Tr::Browse`.
- History btn = `icon::list()`; present only when `id.is_some() && show_history`.
  - If `history` non-empty: `button(...).on_press(Message::TogglePathHistory(id))` and the **whole group is wrapped** in `DropDown::new(group, overlay, history_open).on_dismiss(Message::ClosePathHistory)` (no `.width` → overlay width = group width).
  - If `history` empty: disabled button (no on_press).
  - No tooltip on history (keeps DropDown underlay clean; matches sort-dropdown precedent).
- History overlay = `container(column![...].spacing(2).width(Fill)).padding(6).style(theme::style::card)`; each entry = `button(text(path).size(12)).on_press(Message::SelectPathHistory(id, PathBuf::from(path))).width(Fill).padding([6,8]).style(theme::style::button::text())`. Max 10 (already capped in data).
- Trailing button (right-rounded hover fill) selection:
  - read-only → copy
  - interactive + history btn present → history
  - interactive + no history btn → browse
  All other buttons use the square (middle) style.

Group assembly:
```rust
let mut row = row![].spacing(0).align_y(Alignment::Center);
if show_path_icon { row = row.push(container(icon::folder_open().size(15).color(text_secondary)).padding([0, 10, 0, 8])); }
row = row.push(text_input(...).width(Length::Fill));
row = row.push(copy_btn).push(browse_btn); // browse only if interactive
if history btn { row = row.push(history_btn_or_dropdown); }
let group = container(row).width(Length::Fill).height(Length::Fixed(36.0)).style(theme::style::grouped_frame);
```
- If history enabled & non-empty: return `DropDown::new(group, overlay, history_open).on_dismiss(ClosePathHistory).into()`.
- Else: return `group.into()`.

## New theme styles (`src/ui/theme.rs`, `style` module)

1. Top-level container style:
```rust
pub fn grouped_frame(t: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(t.extended_palette().background.base.color.into()),
        border: iced::Border {
            color: super::border_color(t),
            width: 1.0,
            radius: super::RADIUS_BUTTON.into(),
        },
        ..Default::default()
    }
}
```
2. New `pub mod input` with `grouped(t, _status) -> text_input::Style`:
```rust
text_input::Style {
    background: iced::Background::Color(iced::Color::TRANSPARENT),
    border: iced::Border::default(), // width 0
    icon: p.background.weak.text,
    placeholder: p.secondary.base.color,
    value: p.background.base.text,
    selection: p.primary.weak.color,
}
```
(text_input::Style fields verified: background: Background, border: Border, icon, placeholder, value, selection — all Color/Background, no Option.)
3. New button style `button::grouped_icon(trailing: bool)`:
```rust
pub fn grouped_icon<'a>(trailing: bool) -> impl Fn(&iced::Theme, Status) -> Style + 'a {
    move |t, status| {
        let base_text = t.extended_palette().background.base.text;
        let radius = if trailing { iced::border::Radius::right(super::super::RADIUS_BUTTON) }
                     else { iced::border::Radius::default() };
        Style {
            background: match status {
                Status::Hovered => Some(iced::Color::from_rgba(1.0,1.0,1.0,0.08).into()),
                Status::Pressed => Some(iced::Color::from_rgba(1.0,1.0,1.0,0.14).into()),
                _ => None,
            },
            text_color: base_text,
            border: iced::Border { color: iced::Color::TRANSPARENT, width: 0.0, radius },
            shadow: Shadow::default(),
            ..Default::default()
        }
    }
}
```
The container draws the single 1px rounded outline; buttons are borderless/transparent by default and only show a hover fill (square for middle, right-rounded for trailing). Input is transparent so the container bg shows through. Left corners are always the input's (no hover) → always clean.

## Data model (`src/config.rs`)

- Add field to `Settings`:
  ```rust
  #[serde(default)]
  pub path_history: std::collections::HashMap<String, Vec<String>>,
  ```
- Add helpers:
  ```rust
  impl Settings {
      pub fn record_path(&mut self, key: &str, path: &str) {
          let e = self.path_history.entry(key.to_string()).or_default();
          e.retain(|p| p != path);
          e.insert(0, path.to_string());
          if e.len() > 10 { e.truncate(10); }
      }
  }
  ```
  (Read access in views: `settings.path_history.get("download_dir").cloned().unwrap_or_default()`.)

## Message changes (`src/message.rs`)

- **Remove:** `SaveDirChanged(String)`, `BrowseSaveDir`, `BrowseTorrent`, `FilePicked(FileKind, Option<PathBuf>)`, `enum FileKind`, and `SettingKey::DownloadDir` variant (no emitter after change).
- **Add:**
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PathPickerId { DownloadDir, SaveDir, Torrent }
  impl PathPickerId {
      pub fn history_key(self) -> &'static str {
          match self { Self::DownloadDir => "download_dir", Self::SaveDir => "save_dir", Self::Torrent => "torrent" }
      }
      pub fn is_folder(self) -> bool { !matches!(self, Self::Torrent) }
  }
  ```
  New Message variants:
  - `BrowsePath(PathPickerId)`
  - `PathPicked(PathPickerId, Option<PathBuf>)`
  - `SelectPathHistory(PathPickerId, PathBuf)`
  - `TogglePathHistory(PathPickerId)`
  - `ClosePathHistory`
  - `CopyPath(String)`

## App state & update (`src/app.rs`)

- Add field `path_history_open: Option<PathPickerId>` (init `None`).
- Replace `pick_folder`/`pick_file` with one helper:
  ```rust
  fn pick_path(id: PathPickerId) -> Task<Message> {
      let title = if id.is_folder() { "Select folder" } else { "Select torrent file" };
      if id.is_folder() {
          Task::perform(async move { rfd::AsyncFileDialog::new().set_title(title).pick_folder().await.map(|h| h.path().to_path_buf()) },
              move |maybe| Message::PathPicked(id, maybe))
      } else {
          Task::perform(async move { rfd::AsyncFileDialog::new().set_title(title).add_filter("Torrent", &["torrent"]).pick_file().await.map(|h| h.path().to_path_buf()) },
              move |maybe| Message::PathPicked(id, maybe))
      }
  }
  ```
- New private `apply_path(state, id, p)`:
  ```rust
  fn apply_path(state: &mut Remotrix, id: PathPickerId, p: PathBuf) {
      let s = p.to_string_lossy().to_string();
      state.settings.record_path(id.history_key(), &s);
      match id {
          PathPickerId::DownloadDir => { state.settings.download_dir = p; state.settings_dirty = true; }
          PathPickerId::SaveDir => { state.add_dialog.save_dir = p; config::save(&state.settings); }
          PathPickerId::Torrent => { state.add_dialog.torrent_path = Some(p); config::save(&state.settings); }
      }
      // DownloadDir persists on Apply/Discard (apply-tracked); SaveDir/Torrent persist immediately.
  }
  ```
- Handlers:
  - `BrowsePath(id)` => `return pick_path(id);`
  - `PathPicked(id, maybe)` => `if let Some(p) = maybe { apply_path(state, id, p); } state.path_history_open = None;`
  - `SelectPathHistory(id, p)` => `apply_path(state, id, p); state.path_history_open = None;`
  - `TogglePathHistory(id)` => `state.path_history_open = if state.path_history_open == Some(id) { None } else { Some(id) };`
  - `ClosePathHistory` => `state.path_history_open = None;`
  - `CopyPath(s)` => `if !s.is_empty() { return iced::clipboard::write::<Message>(s); }`
- **Remove** the `if key == SettingKey::DownloadDir { return pick_folder(...) }` early-return and the `SettingKey::DownloadDir => unreachable!()` arm from `Message::SettingChanged`. (DownloadDir no longer flows through SettingChanged.)
- **`revert_apply_settings`**: add `state.settings.download_dir = state.applied_settings.download_dir;` so Discard reverts the download dir value (it is now apply-tracked).
- **`DiscardAndLeaveSettings`**: after `revert_apply_settings(state)`, add `config::save(&state.settings);` so the reverted download_dir value is written (history entries remain, since `path_history` is never reverted — it is an append-only log).
- Reset `path_history_open = None` in: `OpenAddDialog`, `CancelAdd`, `SetSettingsCategory`, `NavigatePage` (prevents stale dropdown).
- Update import: replace `FileKind` with `PathPickerId` in the `use crate::message::...` line.

## View wiring

### `src/ui/settings_page.rs`
- `view` signature: add `path_history: &'a HashMap<String, Vec<String>>` and `path_history_open: Option<PathPickerId>`. Thread both into `download_view` (for the folder row). `advanced_view` needs no history/open (readonly). Pass `theme` into `download_view` (replace the `accent: Color` param; compute `let accent = theme::accent(theme);` inside — it already uses accent for `group_title`).
- `download_view` signature → `download_view(fluent, theme, settings, path_history, path_history_open)`.
- Replace `download_folder_row` body: `row![ text(fluent.get(Tr::DownloadFolder)).size(13).width(Length::Fixed(200.0)), path_picker::view(fluent, theme, &settings.download_dir.to_string_lossy(), Some(PathPickerId::DownloadDir), true, true, path_history_open == Some(PathPickerId::DownloadDir), &hist) ].height(36).align_y(Center)` where `hist = settings.path_history.get("download_dir").cloned().unwrap_or_default()`.
- Replace `labeled_readonly` (used 3× in `advanced_view`) to use the component in read-only mode: `row![ text(label).size(13).width(Length::Fixed(200.0)), path_picker::view(fluent, theme, &value, None, true, false, false, &[]) ].height(36).align_y(Center)`.

### `src/ui/add_dialog.rs`
- `view` signature: add `path_history: &'a HashMap<String, Vec<String>>` and `path_history_open: Option<PathPickerId>` (and use the `_theme` param — rename to `theme`).
- Replace `save_row` with: a small `text(fluent.get(Tr::SaveTo)).size(12).style(secondary)` label above `path_picker::view(fluent, theme, &state.save_dir.to_string_lossy(), Some(PathPickerId::SaveDir), true, true, open_save, &hist_save)` (column, spacing 4).
- Replace `torrent_row` with: label `text(fluent.get(Tr::OrTorrent)).size(12).style(secondary)` above `path_picker::view(fluent, theme, &torrent_value, Some(PathPickerId::Torrent), true, true, open_torrent, &hist_torrent)` where `torrent_value = state.torrent_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()`.
- **Drop** the cosmetic url-editor auto-fill of the torrent filename (was in app.rs `FilePicked` Torrent branch). `can_submit()` already returns true when `torrent_path.is_some()`, and the AddDownload handler takes the torrent branch regardless of url_editor content — so the URL box can stay empty when a torrent is chosen. Note this as an intentional behavior change.

### `src/app.rs` `view()`
- Pass `&state.settings.path_history` and `state.path_history_open` to both `settings_page::view` and `add_dialog::view`.

### `src/ui/mod.rs`
- Add `pub mod path_picker;`.

## i18n (`src/i18n.rs` + locale files)
- Add `Tr::Copy` (key `"copy"`). Map in `Tr::key()`.
- `i18n/locales/en/main.ftl`: `copy = Copy`
- `i18n/locales/zh-CN/main.ftl`: `copy = 复制`
- Reuse existing `Tr::Browse` for the browse tooltip.

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` (no warnings)
- `cargo build` (offline; no new deps — `iced_aw` `drop_down` already enabled, `rfd`/`iced::clipboard` already used)
- Manual/visual checks:
  1. Settings → Download: pick folder → input shows path; copy btn writes clipboard; history btn opens dropdown listing the path; selecting from history updates `settings.download_dir`; Apply persists; Discard reverts value (history kept). Confirms the bug fix (download_dir now actually changes).
  2. Add dialog: Save-to picker + torrent picker each have independent history; Ctrl+C in the read-only input copies selected text; one history dropdown open at a time.
  3. Advanced page: 3 readonly paths render with leading icon + input + copy only (no browse/history); copy works.
  4. Group visuals: no gaps between input and buttons; rounded corners on both outer sides; hover fills respect corners (trailing right-rounded, middle square).

## Risks / notes
- **Read-only input cursor**: hover shows arrow (not I-beam) because `on_input` is None (iced_widget text_input.rs:1410). Cosmetic only; selection + Ctrl+C still work. If undesirable later, pass `.on_input(|_| Message::Noop)` (keeps non-editing, restores I-beam) — but that fires a Noop per keystroke; current no-`on_input` approach is preferred.
- **Download dir now apply-tracked**: changing it sets `settings_dirty`, triggering the leave-confirm. This is intentional and consistent with the Download category. `revert_apply_settings` + `config::save` on Discard keeps file state correct.
- **Leading icon is always `folder_open`** (even for torrent file paths). Acceptable as a generic "path" indicator; can be parameterized later if needed for future components.
- **History overlay width** auto-matches the group because the whole group is the DropDown underlay (overlay width defaults to `underlay_bounds.width`). No explicit `.width` needed. If a future iced_aw change alters this default, set `.width(Length::Fixed(360))` as fallback.
- **No "clear history" action** in the dropdown (not requested). History is capped at 10 via `record_path`. Add later if desired.
