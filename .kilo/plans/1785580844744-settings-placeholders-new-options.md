# Settings page: missing options (BT LPD/PEX, file-allocation, disk-cache) + input placeholders

## Context (verified against code)

- **User-Agent default already exists**: `default_user_agent()` → `Remotrix/{version}` (src/config.rs:77),
  wired via `#[serde(default = "default_user_agent")]` (config.rs:31) and `impl Default` (config.rs:126),
  and pre-filled into the settings editor at src/app.rs:88. User confirmed: **keep `Remotrix/{version}`**,
  no value change. Only remaining UA work is a placeholder for the emptied/cleared case.
- **All existing `Settings`/`Aria2Options` fields already have UI**; nothing is orphaned.
- User chose to add these missing settings (global HTTP auth was NOT selected):
  1. BitTorrent: Enable LPD (`bt-enable-lpd`) and Enable PEX (`enable-peer-exchange`)
  2. Advanced: `file-allocation` (none/prealloc/falloc) and `disk-cache` (MB)
- iced 0.14.2 `text_editor` supports `.placeholder()` (verified in iced_widget-0.14.2 source), so the
  UA editor can keep its `text_editor` and just gain a placeholder.
- Placeholders needed on the 5 plain text inputs (BT tracker, ED2K server, proxy address/username/password)
  plus the UA editor.

## Task 1 — `src/message.rs`: new SettingKey variants

Add to `enum SettingKey` (near existing BT/proxy keys):

```rust
BtEnableLpd,
EnablePeerExchange,
FileAllocation,
DiskCache,
```

## Task 2 — `src/config.rs`: new `Aria2Options` fields

1. Add fields to `Aria2Options` (after `bt_require_crypto`):
   ```rust
   #[serde(default = "default_true")]
   pub bt_enable_lpd: bool,
   #[serde(default = "default_true")]
   pub enable_peer_exchange: bool,
   #[serde(default = "default_file_allocation")]
   pub file_allocation: String,
   #[serde(default = "default_disk_cache_mb")]
   pub disk_cache_mb: u64,
   ```
2. Add default fns next to the other `default_*` fns:
   ```rust
   fn default_file_allocation() -> String {
       "prealloc".into()
   }
   fn default_disk_cache_mb() -> u64 {
       16
   }
   ```
3. `impl Default for Aria2Options`: init the 4 fields
   (`bt_enable_lpd: true`, `enable_peer_exchange: true`, `file_allocation: default_file_allocation()`,
   `disk_cache_mb: default_disk_cache_mb()`).
4. `Settings::to_aria2_task_options` — add to the `extra` map (style like the existing `enable-dht` block):
   ```rust
   extra.insert(
       "bt-enable-lpd".into(),
       Value::String(if self.aria2.bt_enable_lpd { "true" } else { "false" }.into()),
   );
   extra.insert(
       "enable-peer-exchange".into(),
       Value::String(if self.aria2.enable_peer_exchange { "true" } else { "false" }.into()),
   );
   extra.insert("file-allocation".into(), Value::String(self.aria2.file_allocation.clone()));
   extra.insert("disk-cache".into(), Value::String(format!("{}M", self.aria2.disk_cache_mb)));
   ```
   (aria2 defaults mirrored: LPD/PEX true, prealloc, 16M.)

## Task 3 — `src/i18n.rs`: new `Tr` variants + key mappings

Add variants to `enum Tr` and matching arms in `Tr::key()`:

- `BtEnableLpd` → `"bt-enable-lpd"`
- `EnablePeerExchange` → `"enable-peer-exchange"`
- `FileAllocation` → `"file-allocation"`
- `FileAllocationNone` → `"file-allocation-none"`
- `FileAllocationPrealloc` → `"file-allocation-prealloc"`
- `FileAllocationFalloc` → `"file-allocation-falloc"`
- `DiskCache` → `"disk-cache"`
- `Performance` → `"performance"`
- `BtTrackerPlaceholder` → `"bt-tracker-placeholder"`
- `Ed2kServerPlaceholder` → `"ed2k-server-placeholder"`
- `ProxyAddressPlaceholder` → `"proxy-address-placeholder"`
- `ProxyUsernamePlaceholder` → `"proxy-username-placeholder"`
- `ProxyPasswordPlaceholder` → `"proxy-password-placeholder"`
- `UserAgentPlaceholder` → `"user-agent-placeholder"`

## Task 4 — `i18n/locales/en/main.ftl` and `zh-CN/main.ftl`

Append new lines to both files:

en:
```ftl
bt-enable-lpd = Enable Local Peer Discovery (LPD)
enable-peer-exchange = Enable Peer Exchange (PEX)
file-allocation = File allocation
file-allocation-none = None
file-allocation-prealloc = Preallocate
file-allocation-falloc = Allocate (falloc)
disk-cache = Disk cache (MB)
performance = Performance
bt-tracker-placeholder = udp://tracker.opentrackr.org:1337/announce, udp://open.demonii.com:1337/announce
ed2k-server-placeholder = ed2k://|server|host|port|/ (comma-separated)
proxy-address-placeholder = http://127.0.0.1:7890 or socks5://127.0.0.1:1080
proxy-username-placeholder = Username (optional)
proxy-password-placeholder = Password (optional)
user-agent-placeholder = Leave empty to use the engine default
```

zh-CN:
```ftl
bt-enable-lpd = 启用本地对等发现 (LPD)
enable-peer-exchange = 启用对等交换 (PEX)
file-allocation = 文件分配方式
file-allocation-none = 不预分配
file-allocation-prealloc = 预分配
file-allocation-falloc = 立即分配 (falloc)
disk-cache = 磁盘缓存 (MB)
performance = 性能
bt-tracker-placeholder = udp://tracker.opentrackr.org:1337/announce, udp://open.demonii.com:1337/announce
ed2k-server-placeholder = ed2k://|server|主机|端口|/（逗号分隔）
proxy-address-placeholder = http://127.0.0.1:7890 或 socks5://127.0.0.1:1080
proxy-username-placeholder = 账号（可选）
proxy-password-placeholder = 密码（可选）
user-agent-placeholder = 留空则使用引擎默认
```

## Task 5 — `src/ui/settings_page.rs`: placeholders on inputs

1. `labeled_text_input` (settings_page.rs:824): add `placeholder: &'a str` param and chain
   `.placeholder(placeholder)` on the `text_input(...)` builder. Update call sites:
   - `bittorrent_view` BtTracker (settings_page.rs:464): `fluent.get(Tr::BtTrackerPlaceholder)`
   - `ed2k_view` Ed2kServer (settings_page.rs:497): `fluent.get(Tr::Ed2kServerPlaceholder)`
   - `proxy_fields` ProxyServer/ProxyUsername/ProxyPassword (settings_page.rs:606-623):
     `fluent.get(Tr::ProxyAddressPlaceholder)` / `ProxyUsernamePlaceholder` / `ProxyPasswordPlaceholder`
2. `labeled_editor` (settings_page.rs:842): add `placeholder: &'a str` param and chain
   `.placeholder(placeholder)` on the `text_editor(...)` builder. Update the UA call site in
   `network_view` (settings_page.rs:586) with `fluent.get(Tr::UserAgentPlaceholder)`.
   (UA keeps `text_editor`; placeholder only visible after the user clears the pre-filled value.)

## Task 6 — `src/ui/settings_page.rs`: new controls

1. `bittorrent_view` — after the `EnableDht` toggler (settings_page.rs:463) push:
   - `labeled_toggle(fluent.get(Tr::BtEnableLpd), settings.aria2.bt_enable_lpd, SettingKey::BtEnableLpd)`
   - `labeled_toggle(fluent.get(Tr::EnablePeerExchange), settings.aria2.enable_peer_exchange, SettingKey::EnablePeerExchange)`
2. `advanced_view` — after the `update_toggle` row and before the `Engine` group (settings_page.rs:769)
   add a `Performance` group:
   - `group_title(fluent, Tr::Performance, accent)`
   - file-allocation pick list via `labeled_pick`:
     options `[FileAllocationNone, FileAllocationPrealloc, FileAllocationFalloc]` mapped to
     `Labeled { value: "none"/"prealloc"/"falloc", label: fluent.get(...) }`,
     `selected: Some(settings.aria2.file_allocation.clone())`,
     `on_select: |opt| Message::SettingChanged(SettingKey::FileAllocation, opt.value)`.
   - `labeled_number(fluent.get(Tr::DiskCache), &settings.aria2.disk_cache_mb, 0..=u32::MAX, 1, SettingKey::DiskCache)`
   (`labeled_number` already supports `u64`/`u32`; reuse existing step-1 stepper.)

Note: dirty/reset already work automatically — `apply_fields_equal` compares the whole
`aria2 == aria2` struct (config.rs:376), which includes the new fields.

## Task 7 — `src/app.rs`: `SettingChanged` handlers

Add arms in the big `SettingKey` match (near the existing `BtRequireCrypto`/`EnableDht` arms):

```rust
SettingKey::BtEnableLpd => {
    state.settings.aria2.bt_enable_lpd = value == "true";
}
SettingKey::EnablePeerExchange => {
    state.settings.aria2.enable_peer_exchange = value == "true";
}
SettingKey::FileAllocation => {
    state.settings.aria2.file_allocation = value;
}
SettingKey::DiskCache => {
    if let Ok(n) = value.parse::<u64>() {
        state.settings.aria2.disk_cache_mb = n;
    }
}
```

## Risks / notes

- `to_aria2_task_options()` is applied both at engine boot (engine.rs:983 → `changeGlobalOption`) and on
  Apply (app.rs:788). Per-task `add_uri` currently builds a **minimal** `TaskOptions` (engine.rs:572) and
  does NOT include these new options. `changeGlobalOption` may reject per-download options such as
  `file-allocation`/`bt-enable-lpd` — the existing code already passes per-download options
  (`enable-dht`, `min-split-size`, …) through it and only warns on failure, so this is consistent.
  **Runtime verification required**: after Apply, confirm the 4 options reach the engine
  (aria2 RPC `getGlobalOption` or engine log). If they are rejected globally, fall back to seeding
  the per-task base in `handle_client_cmd::AddDownload` from `crate::config::load().to_aria2_task_options()`
  (overriding dir/split/concurrency + `advanced.apply`).
- `disk-cache` is serialized as `"<n>M"` (aria2 expects a size string, e.g. `16M`); `0M` disables it.
- No settings.json migration needed (serde defaults fill absent keys; no `deny_unknown_fields`).

## Validation

1. `cargo build` (offline)
2. `cargo clippy --workspace` (no new warnings)
3. `cargo fmt --check`
4. Manual:
   - BitTorrent page: LPD + PEX toggles present, default ON.
   - Advanced page: Performance group with file-allocation pick list (default Preallocate) and
     disk-cache stepper (default 16).
   - Placeholders render in BT tracker, ED2K server, proxy address/username/password, and UA editor
     (UA shows `Remotrix/0.1.0` by default; placeholder appears after clearing).
   - Applying settings marks dirty state including the new fields; reset reverts them.
   - If feasible, confirm the new options appear in aria2 `getGlobalOption`/engine logs after Apply.
