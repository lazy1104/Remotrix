# Remotrix — Update Flow (engine, app, diagnostics, edge cases)

Part of the AI context doc set; index in `AGENTS.md`. Read this before touching any update / silent
update / app self-update logic, the updater crate, or the `check_updates` path.

---

## App-layer orchestrated update check
Update check is orchestrated from the app layer (`app.rs check_updates`), not by aria2 or the
updater crate on their own:

- Fetch releases via `src/updater.rs` (`fetch_releases_since`) for both the engine (aria2) and the app.
- Non-silent + app updates open a dialog for the user to confirm.
- Silent aria2 updates auto-stage via `EngineCmd::DownloadAria2Update` → `aria2_fetcher::stage_update_from`
  writes `.pending-update` → next restart (or engine restart) applies the pending update.
- App self-update stages a raw binary via `app_updater::stage_app_update` → `.pending-update` →
  `apply_pending_app_update` swaps on relaunch.

`src/updater.rs` is reusable: `fetch_latest_release` / `ReleaseInfo` are shared between aria2
and future app-update flows.

## `silent_update_scope` replaces `aria2_silent_update`
Settings → Auto Update's old aria2-only toggle is now a 4-option dropdown (Off / Engine / App / Both)
driven by `SilentUpdateScope` (`src/config/settings.rs`). `Default::Both` covers fresh installs that
have no JSON migration path; `config::load()` runs `migrate_legacy_silent_update_scope` once on first
read to map the old `aria2_silent_update: bool` field onto `silent_update_scope`
(`true → "engine"`, `false → "off"`) and drops the legacy key from disk so subsequent starts skip the
migration. The `UpdateResult` handler in `src/update/settings/mod.rs` dispatches `silent_applied` by
`offer.component`: Aria2 keeps the silent `send_download_aria2_update(..., true)` flow; App routes
through the extracted `kick_off_app_update(state, &offer, /*show_downloading_toast=*/ false)` so silent
app downloads skip the "正在下载更新" toast/notify and rely on the existing `AppUpdateReady` sticky
toast with the "更新" action button — application (AppImage relaunch, deb open, Windows installer
launch) **always** waits for the user to click, matching the aria2 "no auto-restart" semantics.

## Update-check diagnostics
The in-app "立即检查 / Check Now" path now emits `tracing` lines to the daily
`remotrix.<date>.log`:

- `update check started` — scope/silent/beta/current versions (`src/app.rs check_updates`).
- Per-component `release fetched` / `release fetch failed` with the fetched + current version and `component`.
- `update check complete` — offers/silent/errors/pending (`src/update/settings/mod.rs UpdateResult`).
- `update fetch: releases/latest|releases list` + `release matched` — tag/version/asset (`src/updaters/updater.rs`).
- `is_background_busy: true` / `is_background_busy: false` — names which term (e.g. `aria2_downloading`)
  keeps the top-border bar lit.

The busy log fires only on **transition** — once per change of the active set, throttled by a
process-local `OnceLock<Mutex<...>>` (`src/app.rs`) so the previous 60fps spam while a single download
was active collapses to one line at start and one at end. Logging is gated by the app log level
(Settings → Log, default `warn`); set it to `debug` or `trace` to see these lines and to confirm what a
manual check actually fetched.

## `assets_url` fallback for missing release assets
When a release's embedded `assets` array comes back empty (GitHub snapshot desync, e.g. assets
uploaded after publish) `release_from_json` falls back to that release's `assets_url`
(`/releases/{id}/assets`) for the authoritative list and logs
`update fetch: inline assets empty; fell back to assets_url` at `warn` (with tag/version/asset_count),
so silent missed updates no longer look like "已是最新" and the fallback frequency is auditable.
The `assets_url` call is one extra GET per affected release (zero in steady state). All lines are
instrumentation-only — no behavior change beyond the fallback itself.

## Known limitation — pending-update promote can delete the working binary
`aria2_fetcher::apply_pending_update` (`src/updaters/aria2_fetcher.rs`) deletes the previously installed
aria2-next binary as part of promoting a `.pending-update` whose `sha256` matches the staged file.
SHA verification protects against corruption, not against a valid file that is not a working
aria2-next — such a staged binary will still pass and nuke the working install. A robustness hardening
(pre-promote `spawn` smoke test, or backup-and-rollback) is a known follow-up but not yet
implemented. Test fixtures for pending-update flows must use real, executable binaries (or sandbox
paths that don't shadow an existing install).