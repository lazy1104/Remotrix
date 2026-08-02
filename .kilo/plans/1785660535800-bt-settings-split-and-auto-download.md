# BT Settings Split + Auto-Download Magnet/Torrent Content

## Goal
1. Split the BitTorrent settings page into 3 sub-groups: **BT 设置 / 节点交换 / 做种**.
2. Add one combined toggle **自动下载磁力与种子内容** (`bt_auto_download`):
   - **Enabled** (default, preserves current behavior): magnet links download all files after metadata; a downloaded `.torrent` URL is auto-added as a new task that downloads all files.
   - **Disabled**: a magnet link fetches metadata only and completes (`bt-metadata-only=true`); a `.torrent` URL downloads just the file and no content task is auto-added.

Scope: only affects magnet links and `.torrent` URLs added via the add dialog / clipboard flow. Local `.torrent` file uploads (AddTorrent tab, explicit file selection) are unaffected.

## Files & Changes

### 1. `src/config.rs`
- Add to `Aria2Options` struct (near other bt fields, ~line 51):
  ```rust
  #[serde(default = "default_true")]
  pub bt_auto_download: bool,
  ```
- Add `bt_auto_download: true,` to `Default` impl (~line 148).
- No `apply_fields_equal` change needed — it compares `self.aria2 == other.aria2` (config.rs:434).

### 2. `src/message.rs`
- Add `BtAutoDownload` to `SettingKey` enum (~line 267).

### 3. `src/i18n.rs`
- Add `Tr` variants: `BtAutoDownload`, `NodeExchange`, `Seeding`.
- Add to `key()`: `"bt-auto-download"`, `"node-exchange"`, `"seeding"`.

### 4. `i18n/locales/zh-CN/main.ftl` + `i18n/locales/en/main.ftl`
Add to both:
- `bt-auto-download = 自动下载磁力与种子内容` / `Auto-download magnet & torrent content`
- `node-exchange = 节点交换` / `Node Exchange`
- `seeding = 做种` / `Seeding`

### 5. `src/ui/settings_page.rs` — rewrite `bittorrent_view` (lines 457-510)
Keep signature `bittorrent_view(fluent, settings, accent)`. Replace the single `BtSettings` group with 3 groups (using `group_title` + existing `labeled_toggle` / `labeled_text_input` / `labeled_number`, spacer of `Length::Fixed(16.0)` between groups, `column![].spacing(SPACE_SM)`):
- **BtSettings** (`Tr::BtSettings`): `BtAutoDownload` toggle, `BtRequireCrypto` toggle, `BtTracker` text input (keep existing placeholder logic from current line 485-494).
- **NodeExchange** (`Tr::NodeExchange`): `EnableDht`, `BtEnableLpd`, `EnablePeerExchange` toggles.
- **Seeding** (`Tr::Seeding`): `SeedRatio` number, `SeedTime` number.

### 6. `src/app.rs`
- `SettingChanged` match (~line 844, next to `EnablePeerExchange`): add
  `SettingKey::BtAutoDownload => state.settings.aria2.bt_auto_download = value == "true",`
- `Message::AddDownload` (~line 588-600): compute `let bt_metadata_only = !state.settings.aria2.bt_auto_download;` and pass it in `EngineCmd::AddDownload { .. }`.
- Follow-torrent block (lines 1241-1264): wrap the whole `if state.torrent_followed.insert(gid.clone()) { ... } else { remove }` logic in `if state.settings.aria2.bt_auto_download { ... }`. When disabled, do nothing (no auto-follow).
- `SyncComplete` path (lines 979-985) unchanged.

### 7. `src/engine.rs`
- Add field `bt_metadata_only: bool` to `EngineCmd::AddDownload` (lines 73-78).
- Add helper next to `is_torrent_url` (~line 368):
  ```rust
  pub(crate) fn is_magnet_url(url: &str) -> bool {
      url.trim_start().to_ascii_lowercase().starts_with("magnet:")
  }
  ```
- In `handle_client_cmd` AddDownload URL loop (~line 639, after the existing `is_torrent_url` follow-torrent insert): if `bt_metadata_only && is_magnet_url(&url)`, insert `("bt-metadata-only", Value::String("true".into()))` into `opts.extra_options`.

### 8. `AGENTS.md`
- Update the documented `EngineCmd` enum snippet (`AddDownload { urls, save_dir, split, advanced }` → add `bt_metadata_only`).

## Behavior / Edge Cases
- **Magnet + disabled**: `add_uri` gets `bt-metadata-only=true`; task shows complete once metadata is fetched; no file content downloaded, nothing else happens.
- **`.torrent` URL + disabled**: file downloads (existing `follow-torrent=false`), completes; `torrent_followed` not touched; no content task added. `.torrent` file remains on disk (independent of `delete_torrent_after_complete`, which only applies to the content-task path).
- **`ApplySettings`**: no engine restart needed — `bt-metadata-only` is a per-add option; app-side gating takes effect immediately.
- `revert_apply_settings` and `apply_fields_equal` work via the wholesale `aria2` copy/comparison — no extra edits.
- Only send site for `EngineCmd::AddDownload` is app.rs:593 (verified); no tests construct it.

## Validation
1. `cargo build`
2. `cargo clippy --workspace` (no warnings)
3. `cargo fmt --check`
4. Manual: toggle `bt_auto_download` off → Apply → add a magnet (expect metadata-only complete) and a `.torrent` URL (expect no follow-up task). Toggle on → Apply → both paths auto-download all files. Verify the 3 BT sub-groups render and all existing controls still work.
