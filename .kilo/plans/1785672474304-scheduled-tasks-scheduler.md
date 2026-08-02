# Scheduled Tasks: Speed-Limit Window (UI) + Missing-File Cleanup (croner)

## Goal
Two scheduled-task mechanisms, both config-driven, no schedule-management UI:

1. **定时限速 (scheduled speed limit)** — a daily time window managed from the Settings page:
   - New toggle **启用定时限速** in the settings page's Speed Limits section.
   - When enabled, two **HH:MM time pickers** appear: 开始时间 / 结束时间 (new custom time-picker widget, hour+minute only).
   - Window semantics (confirmed): inside `[start, end)` the existing global DownloadLimit/UploadLimit values apply; outside the window there is **no limit (0)**. Toggle off = current always-on behavior.
2. **丢失文件清理 (missing-file cleanup)** — fixed cron entries written in `settings.json` (e.g. `0/30 * * * * *` → 30s), evaluated by the engine scheduler.

No `iced_aw` (removed from Cargo.toml; AGENTS.md is stale). Use `croner` for cron parsing + a small built-in scheduler task in the engine. The 30s UI subscription in `app.rs` is removed; the startup one-shot check stays gated by `remove_task_if_files_missing`.

## Config model
### New `src/scheduler.rs` (pure model + helpers, no engine deps)
```rust
pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub cron: String,          // croner 6-field, local time, e.g. "0/30 * * * * *"
    pub action: ScheduledAction,
}

#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduledAction { CheckMissingFiles }   // extensible; speed limiting is now the UI window

pub fn parse_cron(expr: &str) -> Result<croner::Cron, _>   // Cron::new(expr).with_seconds_optional().parse()
pub fn parse_hhmm(s: &str) -> Option<(u8, u8)>
pub fn in_speed_window(start: &str, end: &str, now: &chrono::DateTime<chrono::Local>) -> bool
```
Window rule (minutes-since-midnight): `start==end` → always in window; `start<end` → `start <= t < end`; `start>end` (crosses midnight, e.g. 23:00→07:00) → `t >= start || t < end`.

### `src/config.rs`
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeedLimitSchedule {
    #[serde(default)] pub enabled: bool,
    #[serde(default = "default_schedule_start")] pub start: String, // "23:00"
    #[serde(default = "default_schedule_end")]   pub end: String,   // "07:00"
}
```
- `Settings` gains: `#[serde(default)] pub schedules: Vec<ScheduledTask>` and `#[serde(default)] pub speed_limit_schedule: SpeedLimitSchedule` (both in `Default`).
- Add `impl Settings { pub fn effective_task_options(&self) -> TaskOptions }`: `to_aria2_task_options()` then, if `speed_limit_schedule.enabled && !in_speed_window(...)` set `extra_options["max-overall-download-limit"] = "0"` and `["max-overall-upload-limit"] = "0"`.
- Include `speed_limit_schedule` in `apply_fields_equal` so the settings dirty/Apply/Reset logic covers the new fields.

## New time-picker component — `src/ui/components/time_picker.rs`
Function-based, reusing existing primitives (no new dependency):
```rust
pub fn time_picker<'a>(
    value: &'a str,                        // "HH:MM"
    open: bool,
    on_toggle: Message,                    // open/close
    on_change: impl Fn(String) -> Message + 'a,   // emits "HH:MM"
    width: Length,
) -> Element<'a, Message, iced::Theme, iced::Renderer>
```
- Underlay: button styled like `theme::input_layout`/`theme::grouped_input_layout` field showing `HH:MM` + `icon::chevron_down()`; `on_press = on_toggle`. (Optional: add a `clock` glyph in `src/ui/icon.rs` via `render(codepoint)` using the lucide "clock" codepoint from `ALL_ICONS`; otherwise just the chevron.)
- Overlay: `row![ hour_stepper, text(":"), minute_stepper ]` built with existing `number_stepper` (hour `0..=23`, minute `0..=59`, step 1, width ~64px), each emitting `on_change(format!("{:02}:{:02}", h, m))`.
- Wrap in existing `DropDown` primitive (`src/ui/components/drop_down.rs`): `DropDown::new(underlay, overlay, open).on_dismiss(on_toggle.clone()).width(width)`.
- Register `pub mod time_picker;` in `src/ui/components/mod.rs`.

## Settings UI — `src/ui/settings_page.rs` + `src/ui/sidebar.rs` + `src/message.rs`
- `SettingsUiState` gains `schedule_start_picker_open: bool`, `schedule_end_picker_open: bool` (init `false` in `new`).
- `SettingKey` additions: `SpeedLimitScheduleEnabled`, `ScheduleStart`, `ScheduleEnd`.
- `Message` additions: `ToggleScheduleStartPicker`, `ToggleScheduleEndPicker`.
- In `download_view`, Speed Limits section (after the global DownloadLimit/UploadLimit rows): `labeled_toggle(Tr::EnableScheduledSpeedLimit, settings.speed_limit_schedule.enabled, SettingKey::SpeedLimitScheduleEnabled)`; when enabled, two `setting_row`s:
  - 开始时间 → `time_picker(&settings.speed_limit_schedule.start, settings_ui.schedule_start_picker_open, Message::ToggleScheduleStartPicker, |s| Message::SettingChanged(SettingKey::ScheduleStart, s), Length::Fixed(160.0))`
  - 结束时间 → same for end.
  - Optional small hint text: window reuses the Download/Upload limit values; outside the window no limit applies.

## App layer — `src/app.rs`
- `SettingChanged` handler: `SpeedLimitScheduleEnabled` → set bool; `ScheduleStart`/`ScheduleEnd` → set the string only if `parse_hhmm` validates it.
- `ToggleScheduleStartPicker` / `ToggleScheduleEndPicker` → flip the matching bool in `settings_ui`.
- `revert_apply_settings`: restore `speed_limit_schedule` from `applied_settings`.
- `ApplySettings` and `ApplyAndLeaveSettings`: build options via `settings.effective_task_options()` instead of `to_aria2_task_options()`; also send `EngineCmd::ReloadSchedules`.
- Remove the `missing_check` 30s `Subscription` (app.rs:2342-2346) and drop it from the batch; keep the `SyncComplete` one-shot check.

## Engine — `src/engine.rs`
1. `EngineCmd::ReloadSchedules`.
2. Refactor: extract the `EngineCmd::CheckMissingFiles` handler body into `pub(crate) fn trigger_missing_files_check(client: Client, event_tx: EventTx)` (keeps `MISSING_CHECK_IN_FLIGHT` guard + 30s `timeout` + emit `EngineEvent::FilesMissing`); both the cmd handler and scheduler call it.
3. Make `apply_global_options` `pub(crate)`.
4. New task `run_scheduler(client: Client, event_tx: EventTx) -> JoinHandle<()>`:
   - Reads `config::load()` once (schedules + speed_limit_schedule); parses enabled crons, skipping invalid ones with `tracing::warn!`.
   - 1s `interval` ticker; per cron entry track `last_run: DateTime<Local>` (init `now - 60s` for catch-up); `cron.find_next_occurrence(&last_run, false)`; if `next <= now` fire once via `tokio::spawn(...)` and set `last_run = now`.
   - Window logic: `inside = speed_limit_schedule.enabled && in_speed_window(...)`; only act on transitions. On enter → `changeGlobalOption` `max-overall-download-limit`/`max-overall-upload-limit` = `settings.download_limit_kb/upload_limit_kb * 1024`; on leave → both `"0"`. Log transitions. No startup apply needed (boot already applies `effective_task_options`).
   - `CheckMissingFiles` action → `trigger_missing_files_check(client.clone(), event_tx.clone())`.
5. Boot option application: replace `config::load().to_aria2_task_options()` with `config::load().effective_task_options()` in the `on_sidecar_ready` boot task (and in `boot`/`handle_check_update` if it applies options).
6. Supervisor lifecycle: add `scheduler_handle: Option<JoinHandle<()>>`; after each successful boot (initial, `RetryAria2Fetch`, `RestartEngine`, `restart_rx` crash-restart) abort + respawn scheduler with the current `sidecar.client`; abort on Shutdown and at loop teardown; handle `ReloadSchedules` by abort + respawn from fresh config.

## i18n
Add `Tr` keys + `i18n/locales/en/main.ftl` + `zh-CN/main.ftl` strings:
- `enable-scheduled-speed-limit` (启用定时限速)
- `schedule-start-time` (开始时间)
- `schedule-end-time` (结束时间)
- `schedule-hint` (窗口内应用当前限速值，窗口外不限速)

## Cargo.toml
Add `croner = "3"`. NOTE: first `cargo build` needs network to fetch `croner` (+ `derive_builder`, `strum`, chrono feature unification); later offline builds fine. `chrono` already has the `clock` feature needed for `Local::now()`.

## Validation
- `cargo build`, `cargo clippy --workspace` (no warnings), `cargo fmt --check`.
- Settings UI: toggle appears in Speed Limits; enabling reveals both HH:MM pickers; picker overlay opens/closes and edits persist (config.json written, survives restart).
- Window behavior (verify via `get_global_option`/UI speed): toggle off → limits always apply; toggle on 23:00-07:00 with DownloadLimit=512 → unlimited outside, 512 inside; non-crossing window (09:00-18:00) works; `start==end` → always inside; ApplySettings inside window keeps limits, outside zeroes them; toggle changes take effect immediately (ReloadSchedules).
- Cron: `{"cron":"0/30 * * * * *","action":{"type":"check_missing_files"}}` removes completed tasks with missing files (existing `FilesMissing` toast path) without overlap (in-flight guard); invalid cron warns and is skipped.
- Confirm the old 30s subscription is gone (no duplicate cleanup).

## Risks / notes
- Two mechanisms overlap on `max-overall-*-limit`: the cron action set now contains only `check_missing_files` (speed limiting is the UI window) to avoid conflicting limit writers.
- Window normalization lives in `effective_task_options()` (used by app + boot); `ReloadSchedules` on ApplySettings keeps the scheduler's inside/outside state in sync after config edits.
- Behavior change: with `remove_task_if_files_missing` on but no `check_missing_files` cron entry, periodic cleanup stops (only the startup one-shot check runs); the example config restores 30s cadence.
- AGENTS.md still lists `iced_aw` as a dependency; it is no longer present — out of scope, but the plan does not rely on it.
