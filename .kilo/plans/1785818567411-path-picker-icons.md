# Path Picker: Remove Download History Button + Fix Icons

## Goal
1. Download directory picker in Settings should not show the download-history button.
2. Fix icons in `path_picker.rs`: "Browse" (选择文件) uses `folder` icon; "Show in folder" (在文件夹中打开) uses `folder-open` icon.

## Changes

### 1. `src/ui/settings_page.rs`
Remove the download-history button for the download-directory picker by disabling history:
- `SettingsUiState::new`, line ~55: change `PathPicker::folder(settings.download_dir.to_string_lossy().into_owned(), true)` to pass `false` for `show_history`.

### 2. `src/ui/components/path_picker.rs`
Swap the two icons (all icons exist in `src/ui/icon.rs`):
- `reveal_btn` (Show in folder, `Tr::ShowInFolder`): change `icon::folder_search()` → `icon::folder_open()`.
- `browse_btn` (Browse, `Tr::Browse`, mode != ReadOnly): change `icon::folder_open()` → `icon::folder()`.

The `copy_btn` (`icon::copy`) and `history_btn` (`icon::folder_clock`) are unchanged.

## Validation
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual: Settings > Download shows download-folder picker without the history (clock) button; Browse shows a plain folder icon; Show-in-folder shows a folder-open icon.

## Out of Scope
- No other pickers or icons changed.