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

To stop the loop, two complementary cleanups run on every spawn:

1. `strip_error_tasks_from_session` (`src/engine/mod.rs`, called from `Sidecar::spawn` right after
   the `session.txt` file is ensured to exist and *before* `--input-file` is passed to aria2) reads
   the DB's error-state GIDs via `crate::db::error_gids` (`src/storage/db.rs`) and rewrites
   `session.txt` to drop every download entry whose `gid=` is in that set — byte-preserving for the
   rest, so `pause=true`, the URI's trailing tab, blank-line separators and all other options are
   kept intact. After the rewrite aria2 never sees the dead task in the first place.

2. `prune_error_tasks_in_aria2` (`src/engine/mod.rs`, called from `Sidecar::spawn` after the
   WebSocket `Client` is connected and before `Ok(Sidecar { .. })` is returned) calls
   `aria2.tell_stopped(-1, 1000)` and runs `force_remove` + `remove_download_result` on every
   entry whose status is `error` — using aria2 itself as the source of truth for the cases the DB
   missed (e.g. the SQLite status may lag by up to ~1 s, or stay stuck at `waiting` if the previous
   process was killed mid-flush). After removal the GID is gone from aria2's session save, so
   subsequent `--input-file` replays do not resurrect it.

The two paths cover each other's blind spots: the DB strip is deterministic but only knows what the
SQLite row already says, while the RPC prune is authoritative but runs only once before
`--input-file`-restored tasks finish their first re-attempt — either alone leaves a gap the other
fills. Together the cross-restart retry loop is closed for every `Error`-state task the DB has
ever recorded, plus any error tasks the DB hadn't yet flushed when the previous process died.

The DB row is intentionally kept (the UI still shows the task as `Error`, so the user can
right-click → Delete or hit "重新下载", which issues a fresh `aria2.addUri` and a brand-new GID).
Any failure in either prune path (DB read/parse glitch, RPC error, individual `force_remove` /
`remove_download_result` failure) logs `warn!` and is a no-op, so neither path can block engine
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

## Path overrides: `aria2/` and `logs/` nest under the effective app data dir
The effective path resolvers (`resolved_paths()` / `resolve_from_settings()` in
`src/config/paths.rs`) default `aria2` and `logs` to sub-directories of the *effective*
app data dir — i.e. the user `app_data_dir` override if set, otherwise the ProjectDirs
default. This means changing only 应用缓存目录 re-homes the aria2 binary/session cache and
the logs along with it; `migrate_paths` (`src/config/migrations.rs`) copies them over on
the next boot. An explicit `aria2_bin_dir` / `log_dir` override still wins over the nested
default. Path changes are staged as a pending change and only take effect after restart
via 保存并重启; the per-row 重置默认 restores the draft to the last-resolved value.

Gotcha: if the app data dir is placed on a `noexec` mount, the nested `aria2-next` binary
can no longer be executed. Point `aria2_bin_dir` at an exec-capable location as an escape
hatch.

## `default_app_data_dir()` does NOT honour `XDG_DATA_HOME`
`default_app_data_dir()` (`src/config/paths.rs`) does NOT honour `XDG_DATA_HOME` — only `data_home()`
does. The integration test harness works around this by redirecting `$HOME`, but prod callers that
set `XDG_DATA_HOME` while expecting ProjectDirs-style paths will silently land in the default
location. Track as a real-config bug.

## `remotrix://` OS custom URL scheme

The companion browser extension's popup "Open Application" button navigates to
`remotrix://open`. The URL is delivered to the desktop app by the OS handler
that each platform registers at first launch:

| Platform | Registration site | Effect |
|---|---|---|
| Linux   | `src/config/install.rs::install_desktop_file` rewrites `~/.local/share/applications/remotrix.desktop` with `MimeType=x-scheme-handler/<scheme>;` (one or more of `remotrix` / `magnet` / `ed2k`, depending on `Settings::url_schemes`) and `Exec="<exe>" %u`, then runs `xdg-mime default remotrix.desktop x-scheme-handler/<scheme>` for every enabled scheme. AppImage runtime ships its own entry, so the function no-ops there. All failures are `warn!`-logged and never block startup. | `xdg-open remotrix://open` (or `magnet:?xt=…`, `ed2k://…`) launches the binary with the URL as `argv[1]`. |
| Windows | `src/config/install.rs::apply_url_scheme_registration` (called from `apply_settings` in `src/app.rs`) and the start-up `install_url_scheme` keep `HKCU\Software\Classes\<scheme>` in sync with `Settings::url_schemes`. For each enabled scheme it writes the URL-Protocol layout (`(Default) = URL:<Scheme>`, a **`URL Protocol` empty-string value**, `DefaultIcon\(Default) = "<exe>",1`, `shell\open\command\(Default) = "<exe>" "%1"`) via `RegCreateKeyExW` + `RegSetValueExW`. Disabled schemes whose root default value starts with `URL:` are removed via `RegDeleteKeyW` (leaf `DefaultIcon` first, then the `shell` subtree, then the root). Writes target HKCU so ordinary users do not need elevation; system-wide registration (HKLM) is left to installer-level flows. | `start remotrix://open` (or `magnet:`, `ed2k:`) launches the binary with the URL as `argv[1]`. |
| macOS   | Bundle `Info.plist` must declare `CFBundleURLTypes` with `CFBundleURLSchemes = ["remotrix"]`. The repo currently ships **no macOS bundle** (release pipeline is deb/AppImage/NSIS only); the exact fragment is committed at `assets/macos-info.plist.fragment.xml` for future packaging integration. | When `.app` packaging lands, LaunchServices routes the URL via the `kAEGetURL` AppleEvent. |

The desktop CLI parser in `src/main.rs` treats **any** first argument starting
with `remotrix://` as the "open application" gesture (only the prefix is
inspected — the URL body is not parsed). With the app already running, a
second invocation is short-circuited by `app_single_instance::notify_if_running`
(`src/main.rs:34`) and the primary instance is woken via
`Message::ShowRequested`. The handler in `src/update/misc.rs::handle_show_requested`
delegates to `handle_activate_window`, which calls `restore_window_from_tray`
(restoring any hidden-to-tray or autostart-hidden window) and chains
`iced::window::request_user_attention(..., Critical)` + `gain_focus` —
so the running instance reliably raises the window to the front in every state
(tray-hidden, start-hidden, visible-but-occluded). When the app is not
running, the URL launches a fresh process whose `hidden_start` flag is forced
`false` by the `!open_requested` clause in `src/main.rs:52-53`, ensuring the
window appears even if `start_hidden_on_autostart` is set.

`install_desktop_file`'s content-equality short-circuit auto-rewrites the
file the first time it sees the new `MimeType`/`%u` lines — users do not need
to delete `~/.local/share/applications/remotrix.desktop` manually. Existing
installs upgrade silently on the next boot.

**Gotchas:**

- **A value named `URL Protocol` is mandatory on Windows.** Win10 1809+ / Win11
  validate that marker before dispatching a custom scheme. A registration that
  has a correct `(Default) = URL:<Scheme>` **and** a correct
  `shell\open\command` but no `URL Protocol` value is treated as *no handler
  at all*: clicking the link pops "需要使用新应用以打开此 … 链接" (an
  "how do you want to open this?" chooser) and the binary never launches.
  Always verify with `reg query "HKCU\Software\Classes\<scheme>" /s` — the
  `URL Protocol` row must be present with an empty data column. `HKCR` is the
  merged HKCU+HKLM view and is the more reliable place to look.
- **`(Default)` text is intentionally kept as `URL:<Scheme>`, not
  `URL:<Scheme> Protocol`.** The exact string is the ownership marker
  `remove_windows_scheme` compares against so a foreign handler (e.g. `magnet`
  owned by qBittorrent) is never deleted. Changing the written text would make
  every pre-existing install fail the equality check and leave keys behind on
  disable.
- `xdg-mime` may be missing on minimal containers; the failure is logged at
  `warn!` and the URL handler simply won't be the system-wide default until
  the user (or distro) sets it. The desktop file's `MimeType` line is still
  written, so desktop-environment browsers that honour per-file MIME still
  resolve the scheme.
- macOS `.app` bundles receive URL handoff via AppleEvent, **not** via
  `argv`; the current `argv[1]` parser in `src/main.rs` will not see the URL
  until macOS bundle packaging + an `NSApplication` URL handler are wired up.
  This is tracked as out-of-scope of the current task; the plist fragment is
  pre-committed so the macOS side only needs packaging integration.
- `HKEY_CURRENT_USER\Software\Classes` writes are unprivileged and succeed
  for any signed-in user, so the registration is reliable in everyday use.
  Only certain enterprise policies (SRP / AppLocker) can still block the
  write, in which case `apply_url_scheme_registration` logs a `warn!` and
  the OS-handoff path is degraded (manual launch still works). System-wide
  registration under `HKEY_LOCAL_MACHINE\Software\Classes` is intentionally
  not done at runtime; if needed, the installer (NSIS) should drop the same
  keys there during install / upgrade.
- `apply_url_scheme_registration` only deletes scheme keys under HKCU whose
  default value matches the `URL:<scheme>` marker that this app wrote, so
  pointing `magnet` at qBittorrent (or any other handler) is preserved
  across Remotrix upgrades.

## Extension API `/add` trusts the browser extension with absolute paths and headers

The browser extension HTTP API (`src/integration/extension_api.rs`) is bound to
`127.0.0.1` and gated by a per-install `Bearer <secret>` (`settings.extension.secret`),
so the only reachable client is the locally-installed browser extension. Inside that
trust boundary, `/add` honours the optional `dir` / `path` field directly as the
task's `save_dir` and forwards `referer` / `cookie` / `userAgent` /
`requestHeaders` straight to aria2. **No directory allow-list is enforced**: any
absolute path the extension posts is used as-is.

Operator-visible side-effects of trusting the extension:

- The task detail's "保存位置" mirrors exactly what the extension sent. If the
  extension posts `dir=/home/<user>/Pictures`, the task writes there; if it
  posts nothing, the task falls back to `settings.download_dir`. There is no
  server-side override, by the user's direction (see `.kilo/plans/1790761084217-savedir-reconcile-plan.md`).
- The `/add` response now echoes a `received` map (`dir`, `filename`,
  `referer`, `cookie`, `userAgent`, `requestHeaders: <n>`) so the extension can
  diagnose "I sent it, server dropped it" cases without log-diving.
- Tasks whose `dir` came from a sandboxed Save-As (Chromium / Firefox via
  XDG Document Portal → `/run/user/1000/doc/<id>/`) will fail with
  `Failed to open the file ... No such file or directory` because the portal
  mount is gone by the time aria2 writes. That is a browser-side limitation
  — Remotrix cannot resolve portal paths to durable directories. Self-built
  extension UX that lets the user pick a real folder is tracked separately in
  `.kilo/plans/1789268216072-extension-save-path.md`.
- CDN hotlink-protected assets (Referer / Cookie / Signed-URL) need the
  extension to forward the relevant headers; otherwise aria2's request gets
  403/302 and the task ends in `error`. When that happens the GUI now shows
  a toast containing the aria2 `error_code` / `error_message`, so the
  diagnostic no longer lives only in the system notification / engine log.

## Anchored popovers toggle via their anchor button only when closed
The speed-limit / shutdown / custom-color popovers (`src/ui/components/popover.rs`) are
`Length::Fill` layers above the UI. Clicking their anchor button while open fires both the
popover's outside-click close AND the button's toggle, which net close-then-reopen and make
the button unable to close it. The anchor buttons are therefore made inert (`on_press = None`)
while their popover is open (`src/ui/components/speed_hud.rs`, `src/ui/sidebar.rs`,
`src/ui/settings/general.rs`); closing is handled by the popover's outside-click handler. Keep
this contract: an open popover's anchor button must not publish a toggle. The right-click context
menu has no persistent anchor button and is unaffected.