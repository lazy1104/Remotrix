# Path Picker: History Tooltip & DropDown Fix

## Issues

### 1. Missing tooltip on history icon
The `folder_clock` icon button (path_picker.rs:205-211) lacks a tooltip wrapper, unlike the copy button (tooltip: `Tr::Copy`) and browse button (tooltip: `Tr::Browse`).

### 2. DropDown overlay position incorrect in settings page
The history DropDown overlay appears at the wrong position when the path picker is inside a scrollable context (settings page). Root cause: the scrollable's `overlay()` passes `visible_bounds` (widget-local coords) as the `viewport` to the DropDown, but `DropDownOverlay::layout()` uses `self.viewport.height` to clamp `new_position.y` which is in screen coordinates (`previous_position + translation`). This coordinate mismatch causes the clamping to incorrectly reposition the overlay. It works correctly in the add_dialog (outside a scrollable, where local == screen coords).

## Changes

### 1. i18n — Add `Tr::DownloadHistory`
- **src/i18n.rs**: Add `DownloadHistory` variant to `Tr` enum and map to key `"download-history"` in `key()`.
- **i18n/locales/en/main.ftl**: Add `download-history = Download History`
- **i18n/locales/zh-CN/main.ftl**: Add `download-history = 下载历史`

### 2. path_picker.rs — Add tooltip to history button
Wrap the disabled and enabled history buttons in `tooltip::standard(...)` with `text(fluent.get(Tr::DownloadHistory))` and `iced::widget::tooltip::Position::Bottom`.

### 3. path_picker.rs — Set explicit DropDown alignment + offset
In `DropDown::new(group, overlay, self.history_open)` chain:
```rust
drop_down::DropDown::new(group, overlay, self.history_open)
    .alignment(drop_down::Alignment::Bottom)
    .offset(drop_down::Offset::from(0.0))
    .on_dismiss(map(PathPickerEvent::DismissHistory))
    .into()
```
- `Alignment::Bottom` (explicit, same as default) — overlay appears below underlay, centered
- `Offset::from(0.0)` — removes the 5px gap between underlay and overlay

## Validation
```bash
cargo clippy --workspace
cargo fmt --check
```
