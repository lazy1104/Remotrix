# Plan: Show config file location in Settings page

## Goal
Surface the settings file path (`settings.json`) in the Settings UI so the user can easily locate/delete it. Follow the existing readonly-path display pattern used for the engine data dir, session file, and log location.

## Context
- Settings file resolved in `src/config.rs:652-656` by private `config_path()` -> `ProjectDirs::from("dev","remotrix","Remotrix").config_dir().join("settings.json")` (Linux: `~/.config/remotrix/settings.json`).
- The Advanced settings page (`src/ui/settings_page.rs`) already renders read-only file paths via `labeled_readonly(...)` (line 1653), which supports copy/open read-only events. Engine rows are built in `advanced_view` (lines 1155-1268) and rendered under the `Engine` group (line 1365-1366).
- Translations are keyed by `Tr` enum in `src/i18n.rs` with fluent IDs per locale (e.g. `EngineDataDir` -> `engine-data-dir` at line 545).

## Changes

### 1. `src/config.rs`
Add a public accessor next to `config_path()`:
```rust
pub fn config_file_path() -> Option<PathBuf> { config_path() }
```
(Reuse existing `config_path()`; do not create the parent dir — display only.)

### 2. `src/i18n.rs`
- Add `ConfigFile` variant to the `Tr` enum (near `EngineDataDir`/`EngineSessionFile`, rought line 263-264).
- Add fluent id mapping in `tr()` (near line 545): `Tr::ConfigFile => "config-file"`.

### 3. `src/ui/settings_page.rs`
In `advanced_view`, inside the `engine_rows` block, after the session-file row (line ~1198) and before the `aria2_status` block (line 1200), add:
```rust
if let Some(path) = crate::config::config_file_path() {
    let p_str = path.to_string_lossy().into_owned();
    engine_rows.push(labeled_readonly(
        fluent,
        theme,
        fluent.get(Tr::ConfigFile),
        &p_str,
        settings_ui.readonly_hovered.contains(&p_str),
    ));
}
```

### 4. Locale files
- `i18n/locales/en/main.ftl` (near line 177-178): `config-file = Config file`
- `i18n/locales/zh-CN/main.ftl` (near line 177-178): `config-file = 配置文件`

## Validation
- `cargo build` compiles.
- `cargo clippy --workspace` (no warnings allowed).
- `cargo fmt --check`.
- Manual: open Settings > Advanced, confirm the config file path row appears under the Engine group with copy/open working.

## Notes
- No new dependencies; reuse existing `labeled_readonly`/`PathPicker` and `readonly_hovered` state.
- Path is display-only (read-only path picker), consistent with existing rows.