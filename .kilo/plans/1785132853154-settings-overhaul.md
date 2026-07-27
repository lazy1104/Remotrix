# Settings Overhaul — Motrix Parity

## Goal
Expand the minimal settings page into a multi-category (Basic / Advanced / Lab) configuration matching Motrix, exposing the useful aria2 options. Fix the bug where `max_concurrent` is stored but never applied, apply saved settings at engine boot, and default the add-dialog `split` from settings.

## Resolved Decisions
- **Scope:** Full Motrix parity (Basic / Advanced / Lab categories).
- **Apply UX:** Keep explicit **Apply** button for aria2 engine options (numeric/text inputs) to avoid spamming aria2 RPC on each keystroke. Theme/locale/auto-check-update keep current immediate-apply. Toggles (`auto_file_renaming`, `allow_overwrite`, `continue`, `check_integrity`, `enable_dht`) also apply only on Apply (they are aria2 engine options), but persist immediately like the numeric fields.
- **Engine command:** Replace `EngineCmd::SetSpeedLimit` with a unified `EngineCmd::ApplyAria2Options { options: TaskOptions }` that issues a single `change_global_option` call. Remove dead `pending_speed_apply` field.
- **Boot apply:** After sidecar ready, apply saved settings once via the same path so restarts honor config.
- **Backward compat:** Keep existing flat top-level `Settings` fields (`download_dir`, `max_concurrent`, `download_limit_kb`, `upload_limit_kb`, `split`, `theme_mode`, `locale`, `update`). Add a new nested `Aria2Options` struct with `#[serde(default)]` so old `settings.json` files still load.
- **Spawn args:** Leave hardcoded aria2 spawn args (`--continue`, `--auto-file-renaming`, `--allow-overwrite`) as-is; `change_global_option` at boot overrides with user values.

## Out of Scope
- Per-task advanced options in the **New Download** dialog (keep dialog minimal; only default `split` from settings).
- Structured proxy UI (host/port/user/pass split). Single `all-proxy` string field only.
- Auto-fetching public BT-tracker list from network.
- Run-at-startup / system-tray settings.
- Settings "dirty" indicator and transient "Applied" toast (optional stretch, see Validation).

---

## 1. Data model — `src/config.rs`

Add nested struct (all `#[serde(default)]` so missing keys in old configs fall back):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aria2Options {
    // Basic — connection / file
    pub max_connection_per_server: u32,   // default 16 (matches current engine behavior)
    pub min_split_size: String,           // default "1M"  — verify against aria2 manual during impl
    pub auto_file_renaming: bool,         // default true
    pub allow_overwrite: bool,            // default false
    pub r#continue: bool,                 // default true   (resume partial)
    pub check_integrity: bool,            // default false
    // Advanced — per-task & misc
    pub max_download_limit_kb: u64,       // default 0 (unlimited)
    pub max_upload_limit_kb: u64,         // default 0
    pub lowest_speed_limit_kb: u64,       // default 0
    pub user_agent: String,               // default "" (aria2 default)
    pub headers: Vec<String>,             // default []   (each "Key: value")
    pub all_proxy: String,                // default ""   (e.g. http://user:pass@host:port)
    pub max_tries: u32,                   // default 5
    pub retry_wait: u32,                  // default 0  (seconds)
    pub connect_timeout: u32,             // default 60 (seconds)
    pub bt_tracker: String,               // default "" (comma-separated URLs)
    pub seed_ratio: f64,                  // default 1.0
    pub seed_time: u32,                   // default 0  (minutes; 0 = follow seed-ratio)
    pub enable_dht: bool,                 // default true
}
```
Add `pub aria2: Aria2Options` to `Settings` with `#[serde(default)]`. Implement `Default for Aria2Options` with the defaults above.

Add a method `Settings::to_aria2_task_options(&self) -> TaskOptions` that builds the `aria2_ws::TaskOptions` for `change_global_option`:

| Settings field | aria2 option key | TaskOptions field / placement |
|---|---|---|
| `max_concurrent` | `max-concurrent-downloads` | `extra_options` (string) |
| `split` | `split` | `TaskOptions.split` (i32) |
| `aria2.max_connection_per_server` | `max-connection-per-server` | `TaskOptions.max_connection_per_server` (i32) |
| `aria2.min_split_size` | `min-split-size` | `extra_options` (string) |
| `aria2.auto_file_renaming` | `auto-file-renaming` | `TaskOptions.auto_file_renaming` |
| `aria2.allow_overwrite` | `allow-overwrite` | `extra_options` ("true"/"false") |
| `aria2.r#continue` | `continue` | `TaskOptions.r#continue` |
| `aria2.check_integrity` | `check-integrity` | `TaskOptions.check_integrity` |
| `download_limit_kb` | `max-overall-download-limit` | `extra_options` (bytes string; "0"=unlimited) |
| `upload_limit_kb` | `max-overall-upload-limit` | `extra_options` (bytes string) |
| `aria2.max_download_limit_kb` | `max-download-limit` | `TaskOptions.max_download_limit` (bytes string) |
| `aria2.max_upload_limit_kb` | `max-upload-limit` | `extra_options` (bytes string) |
| `aria2.lowest_speed_limit_kb` | `lowest-speed-limit` | `TaskOptions.lowest_speed_limit` (bytes string) |
| `aria2.user_agent` | `user-agent` | `extra_options` (omit if empty) |
| `aria2.headers` | `header` | `TaskOptions.header` (only if non-empty) |
| `aria2.all_proxy` | `all-proxy` | `TaskOptions.all_proxy` (only if non-empty) |
| `aria2.max_tries` | `max-tries` | `TaskOptions.max_tries` (i32) |
| `aria2.retry_wait` | `retry-wait` | `extra_options` (string) |
| `aria2.connect_timeout` | `connect-timeout` | `extra_options` (string) |
| `aria2.bt_tracker` | `bt-tracker` | `extra_options` (string; only if non-empty) |
| `aria2.seed_ratio` | `seed-ratio` | `extra_options` (string) |
| `aria2.seed_time` | `seed-time` | `extra_options` (string; only if >0) |
| `aria2.enable_dht` | `enable-dht` | `extra_options` ("true"/"false") |

KB/s → bytes: `kb * 1024`, rendered as decimal string; `0` → `"0"` (aria2 treats 0 as unlimited). Import `aria2_ws::TaskOptions` in `config.rs` (already a dependency).

## 2. Engine — `src/engine.rs`

- Replace variant `EngineCmd::SetSpeedLimit { download, upload }` with `EngineCmd::ApplyAria2Options { options: TaskOptions }`.
- In `handle_client_cmd`, handle `ApplyAria2Options` by calling `client.change_global_option(options).await`; emit nothing (or log). Map error to a log/warn.
- In `on_sidecar_ready` (or right after, in the supervisor boot path), after the existing `EngineReady`/`Aria2Version` events, apply saved settings: `let opts = crate::config::load().to_aria2_task_options(); let _ = client.change_global_option(opts).await;`. Do this in a spawned task like the existing `sync_existing_tasks` spawn so it doesn't block readiness. Do the same after every successful `boot(...)` (initial, retry, restart) — simplest: put it inside `on_sidecar_ready`.
- Update the supervisor: it currently reads only `config::load().download_dir` for `SidecarConfig`; leave that, the boot-apply handles the rest.
- `EngineCmd::ApplyAria2Options` must NOT fall into the `_ =>` "engine degraded" branch — add it as an explicit arm (it requires a live client, so route through `handle_client_cmd` when `sidecar` is present, else emit `EngineDegraded`).

## 3. Messages — `src/message.rs`

- Extend `SettingKey` with new variants:
  `MaxConnectionPerServer, MinSplitSize, AutoFileRenaming, AllowOverwrite, Continue, CheckIntegrity, MaxDownloadLimit, MaxUploadLimit, LowestSpeedLimit, UserAgent, Headers, AllProxy, MaxTries, RetryWait, ConnectTimeout, BtTracker, SeedRatio, SeedTime, EnableDht`.
- Extend `SettingsCategory` enum: `Basic, Advanced, Lab` (remove single `General`).
- Keep existing `Message::ApplySettings`. Theme/locale/auto-check-update keep their dedicated messages.

## 4. App state + update — `src/app.rs`

- Remove `pending_speed_apply` field and its initialization.
- `Message::SettingChanged(key, value)`: add arms for each new `SettingKey`, mutating `state.settings.aria2.*` (or top-level for none). Parse numerics with `unwrap_or` fallback to current value; `min_split_size`, `user_agent`, `all_proxy`, `bt_tracker` are raw strings; `headers` is a multi-line string → split on newlines, trim, filter empty. For `seed_ratio` parse `f64`.
  - Note: `download_limit_kb`/`upload_limit_kb`/`max_concurrent`/`split` keep existing arms.
- `Message::ApplySettings`: `config::save(&state.settings)`; build `let opts = state.settings.to_aria2_task_options();` and `send(EngineCmd::ApplyAria2Options { options: opts })`. Remove old `SetSpeedLimit` block.
- `Message::SetSettingsCategory(cat)`: already exists; just store `state.settings_cat = cat`.
- `Message::RestoreAutoCheck` stays. Add no new message for the auto-check toggle (reuse `RestoreAutoCheck` for enable; for disabling add `Message::DisableAutoCheck` that calls `settings.update.set_ignored("aria2-next", true)` + save, OR generalize `RestoreAutoCheck` into `SetAutoCheck(bool)`). Recommendation: replace `RestoreAutoCheck` with `SetAutoCheck(bool)` for symmetry, update the view to a `toggler`.
- `view()` settings branch: pass `state.settings_cat` to `settings_page::view` so it renders the active category. Update the call site signature.

## 5. UI — settings page `src/ui/settings_page.rs`

- Change `view()` signature to accept `category: SettingsCategory` (drop the flat single-page layout). Keep the existing engine/update params.
- Build three sub-view fns: `basic_view`, `advanced_view`, `lab_view`, dispatched on `category`. Each returns the group(s) for that category plus the shared **Apply** button at the bottom (only show Apply for Basic/Advanced; Lab applies immediately).
- Reuse a helper `labeled_input(label, value, key)` and `labeled_toggle(label, value, key)` to reduce repetition (matches existing row pattern). Use `iced::widget::toggler` for booleans, `text_input` for strings/numerics, a multi-line `text_input` (or `text_editor` is overkill — use a single `text_input` for headers/bt-tracker with `;`/`,` separators; headers = one per line is awkward in a single-line input, so use `iced::widget::text_editor`? Simpler: instruct users comma-separated, single `text_input`). Decision: **headers** = comma-separated single text input parsed by splitting on `,`; **bt-tracker** = comma-separated single text input. Document in placeholder.
- **Basic** fields: download folder (browse), max concurrent, split, max-connection-per-server, min-split-size, auto-file-renaming (toggle), allow-overwrite (toggle), continue (toggle), check-integrity (toggle).
- **Advanced** fields: global download limit, global upload limit, per-task download limit, per-task upload limit, lowest-speed-limit, user-agent, headers, all-proxy, max-tries, retry-wait, connect-timeout, bt-tracker, seed-ratio, seed-time, enable-dht (toggle).
- **Lab** fields: theme (radio, existing), locale (radio, existing), auto-check-update (toggler bound to `SetAutoCheck`), engine section (aria2 version + check-update/restart buttons, existing logic).
- Keep `group_title` helper. Wrap each category body in the existing `scrollable` + container.

## 6. UI — category bar `src/ui/category_bar.rs`

- For `Page::Settings`, render three buttons: Basic / Advanced / Lab (localized), each `on_press(Message::SetSettingsCategory(...))`, highlighting the active one via existing `active_filter` style (replace the hardcoded `is_active = true` single item).

## 7. UI — add dialog `src/ui/add_dialog.rs`

- `AddDialogState::open(default_dir)` currently resets `split = 16`. Change to accept the default split from settings: extend `open()` signature to `open(&mut self, default_dir: PathBuf, default_split: u16)` and store it; reset `self.split = default_split`. Update call site in `app.rs` `Message::OpenAddDialog` to pass `state.settings.split`.

## 8. i18n — `src/i18n.rs` + `i18n/locales/{en,zh-CN}/main.ftl`

- Add `Tr` variants + `key()` mappings and the corresponding ftl entries (both en and zh-CN) for every new label/category: `Basic`, `Advanced`, `Lab`, `Split`, `MaxConnectionPerServer`, `MinSplitSize`, `AutoFileRenaming`, `AllowOverwrite`, `Continue`, `CheckIntegrity`, `PerTaskDownloadLimit`, `PerTaskUploadLimit`, `LowestSpeedLimit`, `UserAgent`, `Headers`, `Proxy`, `MaxTries`, `RetryWait`, `ConnectTimeout`, `BtTracker`, `SeedRatio`, `SeedTime`, `EnableDht`, `AutoCheckUpdate`.
- Keep existing keys; do not rename `General` (drop it or leave unused — prefer removing the unused variant to keep `Tr` exhaustive-clean; if removing, also drop its ftl line).

## 9. Validation

- `cargo fmt --check` passes.
- `cargo clippy --workspace` — no warnings (watch: `too_many_arguments` on `settings_page::view` — already `#[allow]`'d; if signature grows, split params into a small `SettingsView` struct or keep the allow).
- `cargo build` (offline) succeeds.
- Manual runtime checks (with aria2-next fetch available):
  1. Change `max_concurrent` to 1, Apply, add 3 tasks → only 1 active, 2 waiting.
  2. Set global download limit, Apply → speed reflects in task list.
  3. Toggle `auto_file_renaming` off, restart engine, re-add same file → behavior matches.
  4. Quit & relaunch → saved settings re-applied at boot (verify via aria2 `getGlobalOption` log or observed behavior).
  5. Add-dialog `split` defaults to the saved `split` value.
  6. Old `settings.json` (pre-change) still loads without panic (serde defaults fill `aria2.*`).

## 10. Risks / Notes
- `min-split-size` default value: verify against aria2 manual during impl (use `"1M"` if unsure; it only affects split granularity).
- `change_global_option` silently ignores options aria2 considers read-only at runtime — none of the exposed options are read-only, but if a future aria2-next build rejects one, the call still applies the rest (aria2 returns error for the whole call only on malformed input; individual unknown keys are ignored). Log the result.
- `header`/`bt-tracker` as comma-separated single-line inputs is a UX compromise; acceptable for v1.
- The `settings_page::view` arg list is already long; adding `category` is one more — keep the existing `#[allow(clippy::too_many_arguments)]`.
- `TaskOptions` import in `config.rs` couples config to `aria2_ws`; acceptable (already a workspace dep) and keeps the mapping testable in one place.
