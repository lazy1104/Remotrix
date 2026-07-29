# Plan: Multi-line URL input in Add Download dialog

## Goal
Replace the single-line `text_input` for download links in the Add Download dialog (`src/ui/add_dialog.rs`) with a multi-line `text_editor`, so users can paste multiple URLs (one per line).

## Context (already verified)
- The backend **already** supports multiple URLs: `app.rs:282-288` splits `add_dialog.url` on `.lines()`, trims, drops empties, and sends `EngineCmd::AddDownload { urls: Vec<String>, .. }` (engine.rs handles `Vec<String>`).
- So this is a **UI-only** change — no engine/db changes.
- `iced::widget::text_editor` is already used in this codebase for the User-Agent / Headers editors (`src/app.rs` `ua_editor`/`headers_editor`, `src/ui/settings_page.rs` `labeled_editor`). The `advanced` iced feature is already enabled.
- `text_editor::Content` implements `Clone` + `Debug` (iced_widget 0.14.2), so it can live inside `AddDialogState` (which derives `Debug, Clone`).
- `Content::text(&self) -> String`, `Content::new()`, `Content::with_text(&str)`, `Content::perform(&mut self, Action)`, `Content::is_empty()` all available.
- `text_editor` supports `.placeholder(..)`, `.height(Length)`, `.padding(..)`, `.size(..)`, `.on_action(fn(Action) -> Message)`.

## Design decision
**Option A — single source of truth:** replace `AddDialogState.url: String` with `url_editor: text_editor::Content`, and introduce `Message::UrlEditor(Action)`.

Rationale: keeps the dialog self-contained (view signature `view(fluent, theme, state)` unchanged, no new AppState field), avoids the dual-storage sync that UA/Headers need (those sync to persisted config; the dialog URL is transient). Only 4 read sites of `add_dialog.url` exist, all easily migrated.

Rejected Option B (mirror UA/Headers: keep `url: String` + add `url_editor: Content` to `Remotrix` and sync) — redundant state for a transient field.

## Changes

### 1. `src/message.rs`
- Remove `AddUrlChanged(String)` (only used at `app.rs:215` and `add_dialog.rs:59`).
- Add `UrlEditor(iced::widget::text_editor::Action)` (place near `UaEditor`/`HeadersEditor` at line 79-80).

### 2. `src/ui/add_dialog.rs`
- Imports: add `text_editor` to the `iced::widget::{...}` import (keep `text_input` — still used by the split field at line 111).
- `AddDialogState`:
  - Replace `pub url: String` → `pub url_editor: text_editor::Content`.
  - `new()`: `url_editor: text_editor::Content::new()`.
  - `open()`: replace `self.url.clear()` → `self.url_editor = text_editor::Content::new();`.
  - `can_submit()`: replace `!self.url.trim().is_empty()` → `!self.url_editor.text().trim().is_empty()`.
- `view()`:
  - Replace the `url_input` `text_input` (lines 58-61) with:
    ```rust
    let url_input = text_editor(&state.url_editor)
        .placeholder(placeholder)
        .on_action(Message::UrlEditor)
        .height(Length::Fixed(120.0))
        .padding(10)
        .size(14);
    ```
  - Keep `placeholder` from `fluent.get(Tr::UrlPlaceholder)`. `Length` is already imported.

### 3. `src/app.rs`
- `Message::AddUrlChanged(value)` handler (lines 215-217) → replace with:
  ```rust
  Message::UrlEditor(action) => {
      state.add_dialog.url_editor.perform(action);
  }
  ```
- `Message::FilePicked` → `FileKind::Torrent` branch (lines 240-246):
  - Empty check: `state.add_dialog.url.trim().is_empty()` → `state.add_dialog.url_editor.text().trim().is_empty()`.
  - Set display: `state.add_dialog.url = <filename>` → `state.add_dialog.url_editor = text_editor::Content::with_text(&<filename>)` (build the filename `String` first, then `with_text(&fname)`).
- `Message::AddDownload` URL collection (lines 282-288): `state.add_dialog.url.lines()...` → `state.add_dialog.url_editor.text().lines()...` (rest unchanged).
- `text_editor` is already imported at `app.rs:8`; no new import needed.
- No change to `OpenAddDialog` call site — `open()` resets the editor internally.

### 4. i18n (optional, recommended)
Update placeholder to hint one-link-per-line in both `i18n/locales/en/main.ftl` and `i18n/locales/zh-CN/main.ftl`:
- en: `url-placeholder = One link per line (https:// or magnet:?xt=urn:btih:...)`
- zh-CN: `url-placeholder = 每行一个链接 (https:// 或 magnet:?xt=urn:btih:...)`

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` (must be warning-free)
- `cargo build`
- Manual run (`cargo run --`):
  1. Paste 3 URLs (one per line) → 3 tasks created.
  2. Single URL → 1 task.
  3. magnet link → works.
  4. Browse .torrent → filename still shows in the editor; submitting downloads via torrent path.
  5. Empty/whitespace-only editor → Download button stays secondary (disabled submit); non-empty → primary (enabled).
  6. Reopen dialog → editor is cleared.

## Notes / risks
- Editor height `Length::Fixed(120.0)` is a sensible default (tunable; `labeled_editor` uses `Fixed(80.0)`).
- `text_editor` placeholder only renders when content is empty — matches old `text_input` behaviour.
- Torrent filename shown inside the URL editor is pre-existing cosmetic behaviour (torrent submit never reads URLs); preserved as-is, out of scope.
- `can_submit()` calls `url_editor.text()` (allocates a `String`) on every view — negligible for a dialog.
