# Settings Restructure — settings.md Parity

Follow-up to `1785132853154-settings-overhaul.md`. Replaces the Basic/Advanced/Lab layout with the 6-category structure in `settings.md`, adds new aria2 options + app-behavior flags, fixes UI polish, and uses proper widgets (NumberInput / pick_list / text_editor).

## Resolved Decisions (user-confirmed)
- **6 categories** with sidebar buttons: 通用 / 下载 / BitTorrent / ED2K / 网络 / 高级. ED2K renders a "coming soon" placeholder page.
- **New features in scope:** 强制BT加密, 启用代理开关, 新建任务后跳转下载页, 完成后删除种子文件, 引擎路径只读展示.
- **Deferred:** 限速时段调度 (omit from UI; no field).
- **Number inputs:** add `iced_aw` 0.14 (`number_input` feature) → `NumberInput` with steppers. Verified compatible (iced_aw 0.14.1 → iced_core 0.14.0 / iced_widget 0.14.2; toolchain rustc 1.96 OK).
- **User-Agent & headers:** multi-line `text_editor` (keep headers field; one header per line).
- **Theme & locale:** `pick_list` (dropdown) instead of radio.
- Apply button shown only for 下载 / BitTorrent / 网络 (aria2 engine options). 通用 / 高级 / ED2K have no Apply (immediate / read-only).

## Category → Settings Mapping
| Category | Section (group_title) | Fields |
|---|---|---|
| 通用 General | 语言 | locale (pick_list) |
|  | 外观 | theme_mode (pick_list) |
| 下载 Download | 下载路径 | download_dir (browse, single row) |
|  | 连接与分段 | max_concurrent, split, max_connection_per_server, min_split_size (NumberInput) |
|  | 恢复与重试 | max_tries, retry_wait (NumberInput); r#continue, check_integrity (toggle) |
|  | 文件 | auto_file_renaming, allow_overwrite (toggle) |
|  | 速度限制 | download_limit_kb, upload_limit_kb, max_download_limit_kb, max_upload_limit_kb, lowest_speed_limit_kb (NumberInput). *(限速时段 omitted/deferred)* |
|  | 通知与确认 | nav_to_tasks_after_add (toggle) |
|  | 自动清理 | delete_torrent_after_complete (toggle) |
| BitTorrent | BT设置 | bt_require_crypto, enable_dht (toggle); bt_tracker (text_input); seed_ratio (NumberInput f64), seed_time (NumberInput u32) |
| ED2K | — | placeholder text "coming soon" |
| 网络 Network | 代理 | proxy_enabled (toggle), all_proxy (text_input) |
|  | User-Agent | user_agent (text_editor) |
|  | 请求头 | headers (text_editor) |
|  | 连接 | connect_timeout (NumberInput) |
| 高级 Advanced | Aria2 Next下载引擎 | version (read-only), auto-check-update (toggle), check-update/restart buttons, status, + read-only path rows (引擎数据目录 / 会话文件 / 日志目录) |

## New Config Fields & aria2 Mapping
`config.rs`:
- `Aria2Options += bt_require_crypto: bool` (default false) → extra `"bt-require-crypto"` ("true"/"false")
- `Aria2Options += proxy_enabled: bool` (default false) → gates `all_proxy`
- `Settings += nav_to_tasks_after_add: bool` (default **true**) — app behavior, not aria2
- `Settings += delete_torrent_after_complete: bool` (default false) — app behavior

`to_aria2_task_options()` changes:
- Add `bt-require-crypto` to extra_options.
- `all_proxy`: if `proxy_enabled && !all_proxy.is_empty()` → `Some(all_proxy)`; else → `Some("")` (**must send empty string to clear** — `change_global_option` leaves omitted keys unchanged, so omitting won't disable a previously-set proxy).
- Keep all other existing mappings.

## NumberInput Bounds (per field)
| Field | Type | Bounds | Step |
|---|---|---|---|
| max_concurrent | u32 | 1..=u32::MAX | 1 |
| split | u16 | 1..=128 | 1 |
| max_connection_per_server | u32 | 1..=16 | 1 |
| max_tries | u32 | 0..=u32::MAX | 1 |
| retry_wait | u32 | 0..=u32::MAX | 1 |
| connect_timeout | u32 | 0..=u32::MAX | 1 |
| download_limit_kb / upload_limit_kb | u64 | 0..=u64::MAX | 100 |
| max_download_limit_kb / max_upload_limit_kb / lowest_speed_limit_kb | u64 | 0..=u64::MAX | 100 |
| seed_ratio | f64 | 0.0..=100.0 | 0.1 |
| seed_time | u32 | 0..=u32::MAX | 1 |

`NumberInput::new(&settings.field, bounds, move |v| Message::SettingChanged(key, v.to_string()))` — on_change gives typed `T`; stringify into existing `SettingChanged(SettingKey, String)` (handler already parses back). Apply `.width(Length::Fixed(160.0))` and `.step(...)`. Requires `T: Bounded` (all listed types satisfy).

## UI Polish (addresses user complaints)
1. **Single download folder row** — `download_view` has exactly one `download_folder_row` (browse). Remove the duplicate `labeled_input` row that existed in the old `basic_view`.
2. **Uniform row height** — add a `setting_row(label, control)` helper that wraps label + control in a `row` with `.height(Length::Fixed(36.0))` and `.align_y(Alignment::Center)` for all single-line controls (NumberInput / text_input / toggler / pick_list). `text_editor` rows use natural height (multi-line), label left-aligned (align_y Top).
3. **Unified label width** — every setting row uses `Length::Fixed(200.0)` for the label (remove the old 180/240 special-casing). Input widths: NumberInput `Fixed(160.0)`; text_input for proxy/bt_tracker `Fill`; pick_list `Fixed(180.0)`; toggler `Fixed(50.0)`; text_editor `Fill`.
4. **Number inputs** — `iced_aw::NumberInput` for all numeric fields (above).

## Torrent Add + Delete Flow (prereq for "完成后删除种子文件")
Current add_dialog sets `url = file://...` and engine uses `add_uri` — aria2 rejects `file://`, so torrents are broken. Fix:
- `AddDialogState += torrent_path: Option<PathBuf>`. `open()` resets it to `None`. `BrowseTorrent` → `FilePicked(Torrent, path)` sets `torrent_path = Some(path)` (and puts the filename in `url` for display). Stop using `file://`.
- `EngineCmd += AddTorrent { path: PathBuf, save_dir: PathBuf, split: u16 }`.
- `engine.rs handle_client_cmd`: `AddTorrent` → `tokio::fs::read(&path)` bytes → `client.add_torrent(bytes, None, options, None, None)`; emit `Added{gid, name=basename(path)}`.
- `app.rs AddDownload` handler: if `add_dialog.torrent_path.is_some()` → send `AddTorrent` and store `state.pending_torrent_path = torrent_path.clone()`; else existing `AddDownload`.
- `app.rs Added` handler: if `pending_torrent_path.is_some()` → insert `torrent_files[gid] = path`, clear pending.
- `app.rs Progress` handler: if `status == "complete"` and `torrent_files` has `gid` and `settings.delete_torrent_after_complete` → `std::fs::remove_file(path)` (ignore errors), remove from map.
- `Remotrix += torrent_files: HashMap<String, PathBuf>, pending_torrent_path: Option<PathBuf>`.
- After submitting (AddDownload or AddTorrent), if `settings.nav_to_tasks_after_add` → `state.page = Page::Tasks`.

## text_editor State (UA / headers)
- `Remotrix += ua_editor: iced::widget::text_editor::Content, headers_editor: iced::widget::text_editor::Content` (type = `Content<iced::Renderer>`; not Clone, but app state needs no Clone — OK).
- `init()`: `Content::with_text(&settings.aria2.user_agent)` and `Content::with_text(&settings.aria2.headers.join("\n"))`.
- `Message += UaEditor(iced::widget::text_editor::Action)`, `HeadersEditor(Action)` (Action is `Clone` ✓). Use tuple-variant as `on_edit` fn: `text_editor(&state.ua_editor, Message::UaEditor)`.
- update: `state.ua_editor.perform(action); state.settings.aria2.user_agent = state.ua_editor.text();` (headers: split `.lines()`, trim, filter empty).
- Pass `&state.ua_editor`, `&state.headers_editor` into `settings_page::view` (add 2 params; keep `#[allow(clippy::too_many_arguments)]`).

## pick_list for theme/locale (localized)
Add a small wrapper to keep type-safety + localization:
```rust
struct Labeled<T> { value: T, label: String }
impl<T> ToString for Labeled<T> { fn to_string(&self) -> String { self.label.clone() } }
```
- options: `vec![Labeled{Dark, fluent.get(Tr::ThemeDark)}, ...]`
- selected: the `Labeled` whose `.value == settings.theme_mode`
- on_select: `|opt| Message::ThemeModeChanged(opt.value)`
Same pattern for `Locale`. `ThemeMode`/`Locale` already `Copy + PartialEq`.

## Files to Edit
1. **Cargo.toml** — add `iced_aw = { version = "0.14", default-features = false, features = ["number_input"] }`. (Pulls non-optional `iced_fonts`; ensure USTC mirror has iced_aw 0.14.1 + iced_fonts 0.3.0.)
2. **src/config.rs** — new fields (above); update `Default` impls; extend `to_aria2_task_options` (bt-require-crypto + proxy gating).
3. **src/message.rs** — `SettingsCategory { General, Download, BitTorrent, Ed2k, Network, Advanced }`; `SettingKey += BtRequireCrypto, EnableProxy, NavToTasksAfterAdd, DeleteTorrentAfterComplete`; `Message += UaEditor(Action), HeadersEditor(Action)`; `EngineCmd` lives in engine.rs.
4. **src/engine.rs** — `EngineCmd += AddTorrent{...}`; `handle_client_cmd` arm reads file + `add_torrent`. (boot-apply already exists from prior plan.)
5. **src/app.rs** — struct fields (editors, torrent_files, pending_torrent_path); init editors; AddDownload branch + nav; Added associate; Progress delete; SettingChanged new arms; view passes editors + settings_cat.
6. **src/ui/settings_page.rs** — rewrite: `view` dispatches to 6 sub-views (`general_view`, `download_view`, `bittorrent_view`, `ed2k_view`, `network_view`, `advanced_view`); helpers `setting_row`, `labeled_number`, `labeled_toggle`, `labeled_text`, `labeled_editor`, `labeled_pick`, `download_folder_row`, `group_title`. Apply button only for Download/BitTorrent/Network.
7. **src/ui/category_bar.rs** — render 6 dynamic buttons for `Page::Settings` using `settings_cat` (rename `_settings_cat` → `settings_cat`); active highlight via existing `active_filter` style.
8. **src/ui/add_dialog.rs** — `AddDialogState += torrent_path`; `open()` reset; torrent-pick sets `torrent_path`.
9. **src/i18n.rs + i18n/locales/{en,zh-CN}/main.ftl** — add Tr variants/keys/ftl; **remove dead**: `Tr::Basic`, `Tr::Lab`, `Tr::RestoreAutoCheck`, `Tr::AutoCheckDisabled` (+ their keys + ftl lines). Add: General, Download, BitTorrent, Ed2k, Network, ConnectionSegment, ResumeRetry, File, NotificationConfirm, AutoCleanup, BtSettings, BtRequireCrypto, EnableProxy, OtherProxyConfig, NavToTasksAfterAdd, DeleteTorrentAfterComplete, EngineDataDir, EngineSessionFile, EngineLogFile, ComingSoon, SelectPlaceholder. Keep Advanced/Proxy/UserAgent/Headers/etc.

## Risks / Notes
- **iced_aw dep weight**: non-optional `iced_fonts` pulled in; verify mirror availability; offline `cargo build` must still pass (deps cached).
- **NumberInput theming**: default `iced_widget::Theme` catalog is implemented by iced_aw; app's `iced::Theme` == `iced_widget::Theme`, so should render. If styling looks off, add a `style` fn. Verify visually.
- **Proxy clear semantics**: must send `all-proxy=""` (not omit) when disabled, else aria2 keeps prior proxy. Implemented in `to_aria2_task_options`.
- **Other clearable strings** (user_agent, bt_tracker): when empty, currently omitted — aria2 won't clear a previously-set value within a session. Acceptable for v1 (full clear happens on engine restart via spawn args). Document; do not over-engineer.
- **text_editor Content not Clone**: stored directly in `Remotrix` (no Clone required by iced app model). Confirmed.
- **Torrent add behavior change**: `file://` add_uri path removed; `.torrent` now uses `add_torrent`. Magnet/HTTP still use `add_uri`.
- **ED2K placeholder**: static "coming soon" text; no fields.
- **`setting_page::view` arg count grows** (editors): keep `#[allow(clippy::too_many_arguments)]`; if unwieldy, bundle view params into a small struct (optional).

## Validation
- `cargo fmt --check`, `cargo clippy --workspace` (no warnings), `cargo build` (offline deps cached).
- Manual:
  1. 通用: change theme/locale via pick_list → applies immediately, persisted.
  2. 下载: set max_concurrent=1, Apply → only 1 active task. NumberInput steppers +/- work; non-numeric input rejected.
  3. 下载: toggle nav-to-tasks on, add a task → jumps to Tasks page.
  4. 网络: toggle proxy off, Apply → aria2 no longer uses proxy (verify via a proxied URL failing). Toggle on + set proxy → applies.
  5. 网络: edit UA/headers in multi-line editors → Apply → reflected (e.g., server logs header).
  6. BitTorrent: add a .torrent (BrowseTorrent) → downloads (proves add_torrent works). Toggle delete-on-complete, re-add → .torrent deleted when BT completes.
  7. 高级: read-only path rows show aria2 data/session/log dirs; check-update/restart buttons work; auto-check toggler persists.
  8. ED2K page shows "coming soon".
  9. Row heights uniform; labels all align at 200px; no duplicate download folder.
  10. Old `settings.json` (pre-change) still loads (serde defaults fill new fields).
