# Remotrix — Cross-Cutting Behavior Gotchas

Part of the AI context doc set; index in `AGENTS.md`. Read this before changing the engine spawn
path, the top-border UI, clipboard handling, the toolbar / task filter, the system font picker,
or the config-path resolvers.

---

## Auto-retrying error tasks across restarts
aria2's `--input-file`/`--save-session` (`src/engine/` install.rs around lines 426-429) restore every
saved entry on startup and rewrite it every 5 s. When a task transitions to `Error` *without* being
paused, aria2's session write omits `pause=true`, so the next startup re-adds the same dead task →
it errors again → written again. The per-session `max-tries` / `retry-wait` settings
(`src/ui/settings_page.rs:995-1009`) do **not** cap cross-restart retries — each engine lifetime gets
its own 5-tries budget.

To stop the loop, `prune_error_tasks_in_aria2` (`src/engine/install.rs` around line 324, called from
`Sidecar::spawn` after the WebSocket `Client` is connected and before `Ok(Sidecar { .. })` is returned)
calls `aria2.tell_stopped(-1, 1000)` and runs `force_remove` + `remove_download_result` on every
entry whose status is `error` — using aria2 itself as the source of truth (the SQLite row can lag
by up to ~1 s, or stay stuck at `waiting` if the previous process was killed mid-flush, so filtering
by DB was unreliable). After removal the GID is gone from aria2's session save, so subsequent
`--input-file` replays do not resurrect it.

The DB row is intentionally kept (the UI still shows the task as `Error`, so the user can
right-click → Delete or hit "重新下载", which issues a fresh `aria2.addUri` and a brand-new GID).
Any failure in the prune path (RPC error, individual `force_remove` / `remove_download_result`
failure) logs `warn!` and is a no-op, so a flaky WebSocket or unresponsive RPC cannot block engine
spawn.

## Top border as background progress
The non-maximized outer frame swaps the static 1px hairline for a 2px `iced::widget::canvas` gradient
bar whenever any of `engine_ui.update_check_in_flight`, `engine_ui.aria2_downloading`,
`app_update_in_flight`, `restart.engine_restart_in_progress`, `settings_ui.syncing_trackers` (BT
tracker sync), `settings_ui.syncing_bootstrap` (ED2K bootstrap sync), or an `aria2_status` stage
other than `ready` is true (`is_background_busy` in `src/app.rs`; renderer in
`src/ui/border_bar.rs`). A terminal engine failure (`aria2_fetch_error.is_some()`) **does not**
drive the bar — the bar animates only while work is genuinely in progress; terminal degraded state
surfaces via the sticky error toast and the optional system notification instead.

The bar uses `theme::primary` and `theme::border_color` so it tracks theme changes for free. A
60 fps `BorderAnimTick` subscription (`src/app.rs`) is mounted **only while busy** — once all signals
clear it drops to `Subscription::none()`, so CPU returns to idle immediately. Maximized windows still
draw no border at all. Phase is derived from `crate::ui::animation::cycle` with `PERIOD = 1500 ms`;
the gradient itself is drawn at 2× window width and shifted left by `phase * width`, giving a
seamless left-to-right loop.

## Clipboard "Smart" webpage filter
`src/integration/clipboard_watch.rs::probe_url` + `apply_webpage_filter` issues a single HEAD/Range
request per **extension-less** http/https URL the user copies, using the configured proxy + UA with
a 5 s per-leg timeout (HEAD and the ranged-GET fallback are independent; worst-case ~10 s per URL).
Network failures collapse to `Ambiguous` (we keep the URL and offer it — better than silently
dropping a real download). Switching focus to the window no longer re-triggers the probe because
`last_clipboard_hash` is now updated on every clipboard read regardless of filter outcome
(`src/update/window.rs`).

Existing installs upgrade silently into `Smart`; users who don't want probing can switch to
`Off`/`Static` in Settings → Clipboard. Logging: every invocation emits an
`info!("clipboard: filter done", mode, kept, dropped, elapsed_ms)` summary; per-URL probes are at
`debug!`; timeouts at `warn!`. Raise verbosity with `RUST_LOG=remotrix=debug` (or `=trace`) when
troubleshooting "why was my clipboard dialog slow / empty".

## Toolbar bulk actions are filter-scoped
The `All` / `Downloading` toolbar's `StartAll` / `PauseAll` / `DeleteAll` / `RemoveAllRecords`
handlers in `src/update/task.rs` no longer call aria2's `*All` RPC variants (which are truly global
and would nuke completed/failed tasks). They iterate `state.tasks` for `Active | Waiting | Paused`
gids and send per-gid `EngineCmd::Pause / Resume / Remove { delete_files }`, then call
`clear_specific_local` (`src/app.rs`) to remove only those gids from the local DB.

The `Completed` / `Failed` filter tabs only render Refresh + Sort + Clear Records — `StartAll` /
`PauseAll` / `DeleteAll` / `RemoveAllRecords` still short-circuit as a defence-in-depth against
message injection (extension API, clipboard, etc.). `ClearCompleted` (`Tr::ClearList` /
"清空记录") picks the predicate by filter: `Failed` → only `Error`, others → `Completed | Removed`.
**If the global aria2 `PauseAll` / `ResumeAll` / `RemoveAll` RPC ever needs to be wired up again, do
not bypass `task_filter` here.**

## No bundled CJK font
The binary does **not** embed a CJK fallback font (`fonts/HarmonyOS_Sans_SC_Regular.ttf` removed,
`src/main.rs` no longer `include_bytes!`s it). When `Settings.font_family` is empty (the picker
option 「系统默认」, or first launch with no config) `src/main.rs` resolves `effective_font_family`
transiently per boot via `src/ui/system_default_font::query()`, which calls the platform-native
API: Linux `fc-match -f "%{family}\n" sans-serif`, macOS `/usr/bin/defaults read -g
AppleSystemUIFont`, Windows `SystemParametersInfoW(SPI_GETNONCLIENTMETRICS).lfMessageFont` (via the
existing `windows` crate with the `Win32_UI_WindowsAndMessaging` feature).

The resolved name is **not** persisted to `settings.json`, so OS font changes (Linux fontconfig user
prefs, GNOME Tweaks, Windows control panel, macOS System Settings) take effect on next launch
without an app upgrade. `config::migrate_font_family` (`src/config/migrations.rs`) is only
responsible for the one-time legacy `"HarmonyOS Sans SC"` → `""` rewrite so upgrade users land on
the OS-following branch. Users who picked a specific family in the picker stay locked to that name
until they re-select 「系统默认」.

Linux and macOS `native_query` falls back to `font_autopick::pick_default_family`
(`system_fonts::find_for_system_locale`, locale-aware fontdb enumeration) when the OS-native path
fails; Windows has no fallback because `SPI_GETNONCLIENTMETRICS` is the source of truth. Linux on
minimal containers (e.g. `scmilinux`, Alpine, `debian:slim`) may not ship `fc-match`; the fallback
log line `fc-match not found on PATH; falling back to font_autopick` lets users trace it.

On a system without any installed sans-serif the chosen family may not cover CJK glyphs and tasks
render as `.notdef` (tofu); users can install `fonts-noto-cjk` or pick a CJK family in
Settings → Appearance → Font. Verify the binary shrank with
`strings target/release/remotrix | grep -c HarmonyOSSansSC` — should be 0 (was nonzero before this
change).

## `default_app_data_dir()` does NOT honour `XDG_DATA_HOME`
`default_app_data_dir()` (`src/config/paths.rs`) does NOT honour `XDG_DATA_HOME` — only `data_home()`
does. The integration test harness works around this by redirecting `$HOME`, but prod callers that
set `XDG_DATA_HOME` while expecting ProjectDirs-style paths will silently land in the default
location. Track as a real-config bug.