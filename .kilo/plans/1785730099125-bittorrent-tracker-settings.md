# BitTorrent Tracker Server Settings (motrix-next style)

## Goal
Replace the current single-line `bt-tracker` input in BitTorrent settings with a full motrix-next-style tracker management panel:
multi-line tracker editor, preset + custom tracker sources, one-click sync from those sources, auto-sync with frequency, last-sync time, and live tracker count. Trackers are applied to aria2 as the global `bt-tracker` option (comma-separated).

## Reference (motrix-next)
Local repo: `/home/caoyucong/workspace/_github/motrix-next` (commit `a60fb80a release: v3.9.7-beta.10`; identical to the temp clone at /tmp/kilo/motrix-next).
- `src/components/preference/BitTorrent.vue` — tracker management UI
- `src/composables/useBtPreference.ts` — `btTracker` stored comma-separated, shown newline-separated; sync REPLACES the whole list
- `src/main.ts` `syncBtTrackersIfDue(startup)` — auto-sync at startup + interval
- `src/shared/utils/tracker.ts` / `src-tauri/src/commands/tracker.rs` — fetch each source URL (`?t=<now>` cache-buster, 30s timeout), collect bodies + per-URL failures
- `src/shared/constants.ts` — sources + defaults (see below)
- `src/shared/utils/syncSchedule.ts` — `checkSyncDue`: disabled→false; `intervalHours<=0`→sync only at startup; `lastSync<=0`→true; else `now-last >= interval*3600*1000`

### Confirmed defaults (from local repo, `DEFAULT_APP_CONFIG` / `constants.ts:218-231,368-377`)
| Field | Default |
|---|---|
| `TRACKER_SOURCE_OPTIONS` | `("ngosang","trackerslist","https://ngosang.github.io/trackerslist/trackers_best.txt")`, `("XIU2","TrackersListCollection","https://cf.trackerslist.com/best.txt")` |
| `trackerSource` (sources) | both preset URLs selected |
| `customTrackerUrls` (custom_urls) | `[]` |
| `btTracker` (editor content) | `''` |
| `btTrackerAutoSync` (auto_sync) | **`true`** |
| `btTrackerSyncIntervalHours` (sync_interval_hours) | `24` |
| `lastSyncTrackerTime` (last_sync_time) | `0` (→ `None`) |
| `MAX_BT_TRACKER_LENGTH` | `6144` |
| Sync frequency picker | `0`=every startup, `6`=6h, `12`=12h, `24`=daily, `168`=weekly |

## Design Decisions
1. **Storage**: keep `Aria2Options.bt_tracker: String` holding the raw editor text (newline-separated, backward-compatible with existing comma-separated values — both normalize to comma on apply). Add a new `TrackerPrefs` struct on `Settings` for management prefs (`sources`, `custom_urls`, `auto_sync`, `sync_interval_hours`, `last_sync_time`).
2. **Sync semantics**: mirror motrix-next — sync REPLACES the tracker list with fetched data (no merge). Documented in the UI implicitly by "Sync Trackers" behavior; no data-loss dialog (matches motrix-next).
3. **Engine apply**: reuse existing path — `to_aria2_task_options()` puts `bt-tracker` (comma) in `extra_options`; `ApplySettings` / sync send `EngineCmd::ApplyAria2Options`. Engine boot also applies saved config (`engine.rs:1363`), so a startup sync that runs before engine-ready is still applied at boot. No `engine.rs` changes.
4. **Auto-sync scheduling**: `CheckTrackerAutoSync { startup }` message — fired once at app init (`startup: true`) and on a 1-hour subscription tick (`startup: false`). Guard: skip when `settings.aria2.bt_tracker != applied_settings.aria2.bt_tracker` (user has unapplied tracker edits) or a sync is in flight.
5. **Dirty state**: after a successful sync, update BOTH `settings` and `applied_settings` for `aria2.bt_tracker` + `tracker` so the tracker change does not falsely mark settings dirty (the engine already has the value).
6. **Out of scope**: tracker probing/blocklists/peer max/ports (motrix-next extras not requested).

## Implementation Steps

### 1. New module `src/trackers.rs` (+ `mod trackers;` in `src/main.rs`)
Pure, unit-testable helpers (follow test style in `src/scheduler.rs`):
- `pub const MAX_TRACKER_LENGTH: usize = 6144;`
- `pub fn parse_lines(body: &str) -> Vec<String>` — split on `\r?\n`, trim, skip empty and `#`-prefixed lines, dedup preserving order.
- `pub fn parse_trackers(text: &str) -> Vec<String>` — split on newlines **and** commas, normalize, dedup.
- `pub fn to_comma(text: &str) -> String` — `parse_trackers` joined with `,`.
- `pub fn to_lines(text: &str) -> String` — `parse_trackers` joined with `\n` (used when seeding the editor from legacy comma-separated config).
- `pub fn count(text: &str) -> usize`.
- `pub fn reduce(value: String) -> String` — if len > `MAX_TRACKER_LENGTH`, truncate and cut back to the last `,` (mirror motrix `reduceTrackerString`).
- `pub fn sync_due(auto_sync: bool, interval_hours: u32, last_sync: Option<i64>, startup: bool, now_ms: i64) -> bool` — mirrors `checkSyncDue`.
- `pub async fn fetch_sources(urls: &[String]) -> (Vec<String>, Vec<(String, String)>)` — reqwest client (30s timeout), GET each url with `?t=<now_ms>` appended; returns (raw bodies, (url, reason) failures).
- Unit tests for all pure fns (`parse_lines`, dedup, comma/newline round-trip, `reduce`, `sync_due`).

### 2. `src/config.rs`
- Add constants:
  ```rust
  pub const TRACKER_SOURCE_OPTIONS: &[(&str, &str, &str)] = &[
      ("ngosang", "trackerslist", "https://ngosang.github.io/trackerslist/trackers_best.txt"),
      ("XIU2", "TrackersListCollection", "https://cf.trackerslist.com/best.txt"),
  ];
  ```
- Add `TrackerPrefs` struct + `Default`, with serde defaults per field:
  ```rust
  pub struct TrackerPrefs {
      #[serde(default = "default_tracker_sources")] pub sources: Vec<String>,
      #[serde(default)] pub custom_urls: Vec<String>,
      #[serde(default = "default_true")] pub auto_sync: bool,
      #[serde(default = "default_tracker_sync_interval")] pub sync_interval_hours: u32,
      #[serde(default)] pub last_sync_time: Option<i64>, // epoch millis
  }
  ```
  `default_tracker_sources()` → the two preset URLs; `default_tracker_sync_interval()` → 24; `default_true()` → `true`. **Defaults mirror motrix-next: auto_sync = true (24h), both presets selected, no custom URLs, empty tracker list, no last-sync time.**
- Add `#[serde(default)] pub tracker: TrackerPrefs` to `Settings` + init in `Settings::default()`.
- `apply_fields_equal`: add `&& self.tracker == other.tracker`.
- In `to_aria2_task_options`, change the `bt_tracker` insert to use `crate::trackers::to_comma(&self.aria2.bt_tracker)` (still only when non-empty).

### 3. `src/message.rs`
- Remove `SettingKey::BtTracker`; add `SettingKey::TrackerAutoSync`, `SettingKey::TrackerSyncInterval`.
- Add messages:
  ```rust
  BtTrackerEditor(iced::widget::text_editor::Action),
  SyncTrackers,
  TrackersSynced { fetched: Vec<String>, failures: Vec<(String, String)> },
  TrackerSourceToggled { source: String, enabled: bool },
  TrackerCustomInputChanged(String),
  TrackerCustomAdd,
  TrackerCustomRemove(String),
  CheckTrackerAutoSync { startup: bool },
  ```

### 4. `src/i18n.rs` (+ both `i18n/locales/{en,zh-CN}/main.ftl`)
- Add `Fluent::get_args(&self, key: Tr, args: &fluent_templates::fluent_bundle::fluent::FluentArgs) -> String` using `LOCALES.lookup_with_args(...)`.
- New `Tr` variants (keys + en/zh-CN translations):
  `BtTrackers` ("bt-trackers"), `BtTrackerSourcePreset` ("bt-tracker-source-preset"), `BtTrackerSourceCustom` ("bt-tracker-source-custom"), `BtTrackerSourceCustomPlaceholder`, `BtTrackerSync` ("bt-tracker-sync"), `BtTrackerCount` ("bt-tracker-count" with `{ $count }` arg), `LastSyncTime` ("last-sync-time"), `BtTrackerInputTips` ("bt-tracker-input-tips"), `BtTrackerSyncSucceed` ("bt-tracker-sync-succeed"), `BtTrackerSyncPartial` ("bt-tracker-sync-partial" with `{ $ok } { $total } { $failed }` args), `BtTrackerSyncFailed` ("bt-tracker-sync-failed"), `BtTrackerSelectSource` ("bt-tracker-select-source"), `BtTrackerSourceInvalidUrl` ("bt-tracker-source-invalid-url"), `AutoSync` ("auto-sync"), `SyncFrequency` ("sync-frequency"), `IntervalEveryStartup`, `Interval6Hours`, `Interval12Hours`, `IntervalDaily`, `IntervalWeekly`.
- Update `bt-tracker-placeholder` to a single-line tracker hint.

### 5. `src/app.rs`
- `Remotrix` struct: add `bt_tracker_editor: text_editor::Content`, `syncing_trackers: bool`.
- `init()`: seed `bt_tracker_editor` with `trackers::to_lines(&settings.aria2.bt_tracker)`; `syncing_trackers: false`; change final return to `(state, Task::done(Message::CheckTrackerAutoSync { startup: true }))`.
- In the reset/apply-from-applied path (~line 245): re-seed `state.bt_tracker_editor = text_editor::Content::with_text(&trackers::to_lines(&state.settings.aria2.bt_tracker));`.
- Remove `SettingKey::BtTracker` handler (~line 1047).
- New handlers:
  - `BtTrackerEditor(action)` → perform; `settings.aria2.bt_tracker = editor.text()`.
  - `TrackerSourceToggled` → add/remove url in `settings.tracker.sources`.
  - `TrackerCustomInputChanged` → `state.settings_ui.custom_tracker_input = v`.
  - `TrackerCustomAdd` → trim; validate `http://`/`https://` (parse via `url::Url`-style check — use `reqwest::Url::parse`); on invalid → warning toast `BtTrackerSourceInvalidUrl`; else dedup-push into `custom_urls` and `sources`, clear input.
  - `TrackerCustomRemove(url)` → remove from `custom_urls` and `sources`.
  - `SyncTrackers` → guard `syncing_trackers`; if `sources` empty → warning toast `BtTrackerSelectSource`; else set flag and `Task::perform(trackers::fetch_sources(urls.clone()), |(fetched, failures)| Message::TrackersSynced { fetched, failures })`.
  - `TrackersSynced { fetched, failures }` → clear flag; build `lines = parse_lines` over all bodies (dedup); if no lines and failures non-empty → error toast `BtTrackerSyncFailed`, return; else set `text = to_lines(&lines.join("\n"))`, re-seed editor, set `settings.aria2.bt_tracker = text`, `applied_settings.aria2.bt_tracker = text`, `settings.tracker.last_sync_time = Some(now_ms)`, mirror `applied_settings.tracker = settings.tracker.clone()`, `config::save`, send `EngineCmd::ApplyAria2Options { options: effective_task_options() }`; toast: `BtTrackerSyncSucceed` (with count) if no failures, else `BtTrackerSyncPartial`.
  - `CheckTrackerAutoSync { startup }` → guards (in-flight, unapplied tracker edits via bt_tracker compare); `trackers::sync_due(...)`; if due, run the same fetch flow as `SyncTrackers`.
- `subscription()`: add `let tracker_auto_sync = if state.settings.tracker.auto_sync { iced::time::every(Duration::from_secs(3600)).map(|_| Message::CheckTrackerAutoSync { startup: false }) } else { Subscription::none() };` and push into the batch.
- `view()`: pass `bt_tracker_editor` + `syncing_trackers` through to `settings_page::view`.

### 6. `src/ui/settings_page.rs`
- `SettingsUiState`: add `pub custom_tracker_input: String` (seeded empty in `new`).
- `settings_page::view` signature: add `bt_tracker_editor: &'a text_editor::Content`, `syncing_trackers: bool`; forward to `bittorrent_view`.
- `labeled_editor`: add `height: f32` param; update the User-Agent call site (`80.0`) and new tracker editor (`140.0`).
- Rewrite `bittorrent_view`:
  1. `group_title(BtSettings)` + existing `BtAutoDownload` / `BtRequireCrypto` toggles.
  2. `group_title(BtTrackers)`:
     - Preset sources: one `labeled_checkbox` per `TRACKER_SOURCE_OPTIONS` with label `"owner/repository"`, checked = `settings.tracker.sources.contains(url)`, on_toggle → `Message::TrackerSourceToggled { source, enabled }`.
     - Custom source: `setting_row(BtTrackerSourceCustom, row[ text_input(custom_tracker_input, on_input=TrackerCustomInputChanged, on_submit=TrackerCustomAdd), button(icon::plus) → TrackerCustomAdd ])`.
     - Custom list: for each `custom_urls`, `setting_row(url, button(icon::x) → TrackerCustomRemove(url))`.
     - Sync row: `setting_row(BtTrackerSync, button(icon::refresh + text) disabled when `syncing_trackers` → SyncTrackers)`.
     - Meta line: secondary text `"{count} trackers · Last sync {time}"` (`count` from `trackers::count(&settings.aria2.bt_tracker)`; time via `chrono::Local.timestamp_millis_opt(last_sync_time).format(...)`, `—` when none).
     - Editor: `labeled_editor(BtTracker, bt_tracker_editor, Message::BtTrackerEditor, BtTrackerInputTips, 140.0)`.
     - `labeled_toggle(AutoSync, settings.tracker.auto_sync, SettingKey::TrackerAutoSync)`.
     - If `auto_sync`: `labeled_pick(SyncFrequency, [0=IntervalEveryStartup, 6=Interval6Hours, 12=Interval12Hours, 24=IntervalDaily, 168=IntervalWeekly], Some(interval), SettingChanged(SettingKey::TrackerSyncInterval, ...))`.
  3. Keep existing `NodeExchange` and `Seeding` groups unchanged.

### 7. Validation
- `cargo build` (offline — no network needed to build)
- `cargo test --lib` (trackers unit tests)
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual: add a custom source, toggle presets, Sync (observe count/last-sync/toast), auto-sync due check, confirm `bt-tracker` reflected in aria2 global options (`aria2.tellGlobalOption` / apply path), confirm no dirty-flag regression after sync, confirm legacy comma-separated `bt_tracker` in existing settings.json still normalizes correctly.

## Risks / Notes
- `cf.trackerslist.com` (XIU2) can be unreachable/geo-blocked in some networks → partial-failure warning toast is expected; ngosang source usually works. Sources are user-selectable checkboxes, so a failed preset can be unchecked.
- aria2's global `bt-tracker` affects newly added torrent/magnet downloads, not already-running ones (same as motrix-next).
- Long lists: `MAX_TRACKER_LENGTH` (6144) truncation applied on the comma string before sending to aria2 (via `reduce` inside `to_aria2_task_options`), matching motrix-next.
- Sync replaces the whole list (motrix-next behavior); users wanting to keep private trackers should re-add them after sync.
