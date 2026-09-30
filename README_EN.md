# Remotrix
> **[中文](README.md) | [English](README_EN.md)**


A Rust-native desktop download manager inspired by [Motrix-next](https://motrix-next.pages.dev/), built with the
[`iced`](https://github.com/iced-rs/iced) GUI framework and an [`aria2-next`](https://github.com/AnInsomniacy/aria2-next)
sidecar engine driven over WebSocket JSON-RPC ([`aria2-ws`](https://crates.io/crates/aria2-ws)).

> The name "Remotrix" is a portmanteau of **Rust** + **Motrix**.

> **⚠ Work in progress / unstable**
> Remotrix is an **AI-assisted learning project** currently in development. Features are **not complete and not fully verified**; some capabilities (resume, integrity checks, rate limiting, retry, and cross-platform compatibility) have not been thoroughly regression-tested and may contain bugs or risk data loss. **Use for testing only — do not download important files with it.** Issues and PRs are welcome.

## Why Remotrix?

Remotrix started as a learning project. I liked Motrix / Motrix-next's design, but the Tauri-based
Motrix-next did not run properly on my Windows 10 machine and had severe performance problems on my
Linux machine. I wanted to learn Rust and the `iced` GUI framework, so I decided to build a native
Rust download manager from scratch, using Motrix-next as the design reference. The app is developed
with AI assistance.

## Features

- **Downloads & engine** — self-managing aria2-next sidecar (runtime fetch, sha256 verify, cache, self-heal, auto-update); HTTP/HTTPS/FTP, BitTorrent, and Magnet links; configurable split / max-connections per server; global & per-task download / upload caps persisted to disk; task metadata and progress in a local SQLite database.
- **Window & desktop integration** — `iced 0.14` pure-Rust rendering, frameless window with custom resize handles; system tray (X11 full support, Wayland minimize-only, Windows `TaskbarCreated` watchdog); cross-platform system notifications (`notify-rust`, plus `win_toast` on Windows); `app-single-instance` focuses the existing window.
- **Theme & i18n** — accent-color swatches auto-derive the full light / dark palette including a M3-style surface background; optional HEX picker with HCT-based derivation; follow system dark / light mode and macOS 14+ / Windows 11 / GNOME accent color, with smooth transition animations; auto-detected `zh_CN` / `en_US`; default font follows the OS UI font (Linux `fc-match`, macOS NSFont, Windows SPI), with optional pinning.
- **Task browsing** — multi-tab task details (Summary / Activity / Files / Advanced) with a BT piece-completion map and tri-state checkbox file tree for per-task selection; sort by added time / name / size / progress / status; filter by All / Downloading / Completed / Failed, with count badges in the category capsules.
- **Clipboard & browser takeover** — auto-detect http/ftp/magnet/ed2k/bt links copied to the clipboard; a local Salvo HTTP server (default `127.0.0.1:29110`, hot-restartable) interoperates out of the box with the upstream [motrix-next-extension](https://github.com/AnInsomniacy/motrix-next-extension) (MIT, reused unmodified) to intercept browser downloads.
- **Extended protocols** — ED2K (automatic `server.met` / `nodes.dat` bootstrap + built-in search), Metalink (`.metalink` / `.meta4`), BT tracker auto-sync (preset sources + custom URLs + periodic sync), BT metadata-only downloads.
- **Automation & system integration** — HTTP / SOCKS5 global + per-task proxy with Basic Auth; scheduled speed limits by time window + weekday; scheduled shutdown / shutdown-when-all-done; cross-platform system sleep inhibition while downloads are active; launch at login (Linux `.desktop` / Windows `Run`); Settings → Advanced → Paths to override aria2 / app cache / log dirs with automatic data migration on next launch.
- **Updates** — a shared background downloader (throttling, retry, streaming sha256, Range resume) backs both aria2 and app updates; aria2 silent update + top-border progress bar; AppImage / deb / NSIS app self-update; silent-update scope selector (Off / Engine / App / App + Engine) — "App" silent only downloads, the action still requires the user to click the sticky toast's "Update" button; customizable RPC port with automatic fallback on conflict.

## Screenshots

Screenshots pending — see `assets/icon.png` for the app logo.

## Architecture

Remotrix uses a **dual event loop** design:

- The **iced UI loop** runs on the main thread
- A **tokio runtime** drives an engine **supervisor** on a background thread, which spawns the
  `aria2-next` subprocess and talks to it over a `aria2-ws` WebSocket client (random local port + per-session RPC secret)
- The two halves communicate over `tokio::sync::mpsc` channels:
  - GUI -> Engine: `EngineCmd` via `mpsc::Sender`
  - Engine -> GUI: `EngineEvent` polled by an `iced::Subscription`
- Progress arrives both from aria2 WebSocket **notifications** and a 1 Hz **polling** loop; the UI
  batches dirty tasks and flushes them to SQLite every second
- aria2 session state is persisted via `--save-session` / `--input-file` so in-flight tasks resume across restarts

```
┌──────────────┐  EngineCmd   ┌──────────────────────────┐
│   iced UI    │ ───────────► │  engine supervisor       │
│  (main loop) │              │  (tokio worker)          │
│              │ ◄─────────── │        │                 │
└──────────────┘  EngineEvent │        ├─ aria2-next     │
        ▲        (Subscription)│        │   (subprocess)  │
        └──────────────────────┤        └─ aria2-ws RPC   │
                                │           (WebSocket)    │
                                └──────────────────────────┘
```

### aria2-next lifecycle

- **First launch** — `updaters::aria2_fetcher::ensure_aria2_next()` downloads the matching platform asset from
  `AnInsomniacy/aria2-next` GitHub Releases into `<data_dir>/aria2/`, verifies its sha256, records
  `.installed`, and marks it executable. Subsequent launches hit the cache.
- **Override** — set `ARIA2_BIN=/path/to/aria2-next` to skip the download entirely (useful for development).
- **Self-healing** — if `.installed` is missing/corrupt, the directory is scanned for a cached binary
  and `.installed` is rebuilt.
- **Updates** — `updaters::updater::fetch_latest_release()` compares versions; a newer release is downloaded in
  the background and a `.pending-update` marker is written. The pending binary replaces the active one
  on the next app / engine restart.
- **Degraded mode** — if fetch or spawn fails, the engine does not exit; the UI surfaces the error and
  offers retry (`RetryAria2Fetch`) / restart (`RestartEngine`).

### Code layout

```
src/
├── main.rs                      # entry, logging init, window settings
├── lib.rs                       # library entry (lib + bin)
├── app.rs                       # Remotrix state, update(), view(), subscription()
├── config/
│   ├── mod.rs
│   ├── paths.rs                 # path resolvers (aria2_bin_dir/session_dir/db_path/log_dir/config_file_path)
│   ├── settings.rs              # Settings load/save, aria2 option mapping
│   ├── install.rs               # user-level .desktop / icon installation
│   └── migrations.rs            # one-shot settings migrations
├── core/
│   ├── mod.rs
│   ├── i18n.rs                  # locale detection + Fluent translations
│   ├── logging.rs               # tracing init, daily rolling logs, runtime log levels
│   ├── message.rs               # Message enum + page / filter / sort / settings enums
│   ├── scheduler.rs             # speed-limit schedule window + weekday helpers
│   └── task.rs                  # DownloadTask model, formatters, TaskDetails / TaskFile
├── engine/
│   ├── mod.rs                   # EngineBridge: spawn tokio supervisor + aria2-next sidecar, mpsc channels
│   ├── install.rs               # runtime wrapper around engine binary fetch / cache / verify
│   └── ed2k.rs                  # ED2K protocol hooks
├── integration/
│   ├── mod.rs
│   ├── clipboard_watch.rs       # clipboard link detection (http/ftp/magnet/ed2k/bt)
│   └── extension_api.rs         # browser extension API: Salvo HTTP server (/ping /stat /add /pause-all /resume-all)
├── platform/
│   ├── mod.rs
│   ├── autostart.rs             # launch at login (Linux .desktop / Windows Run)
│   ├── notify.rs                # notify-rust system notifications
│   ├── port_guard.rs            # RPC port conflict fallback
│   ├── power.rs                 # system sleep inhibition (systemd-inhibit / caffeinate / SetThreadExecutionState)
│   ├── shutdown.rs              # scheduled / all-tasks-complete shutdown
│   ├── tray.rs                  # ldtray: menu, minimize/close-to-tray, Wayland window mgmt
│   ├── tray_watchdog.rs         # Windows: listen for TaskbarCreated, rebuild tray icon
│   └── win_toast.rs             # Windows native toast (AUMID + Locate/Reveal)
├── storage/
│   ├── mod.rs
│   ├── db.rs                    # SQLite persistence (rusqlite): task meta + progress flush
│   ├── ed2k_bootstrap.rs        # server.met / nodes.dat auto-sync
│   ├── torrent_meta.rs          # .torrent metadata parsing (name, files, size)
│   └── trackers.rs              # BT tracker list parse / reduce / merge
├── update/
│   ├── mod.rs
│   ├── add.rs                   # add-task path
│   ├── engine.rs                # engine-side update state machine
│   ├── misc.rs
│   ├── task.rs                  # update task state machine
│   ├── window.rs                # update UI state machine
│   └── settings/
│       ├── mod.rs
│       └── network.rs           # update proxy config
├── updaters/
│   ├── mod.rs
│   ├── app_updater.rs           # app self-update (deb / AppImage / NSIS)
│   ├── aria2_fetcher.rs         # aria2 binary fetch / cache / verify / staging
│   ├── download.rs              # shared background downloader (throttling, retry, streaming sha256, Range resume)
│   └── updater.rs               # GitHub Releases lookup, ReleaseInfo, platform slug
└── ui/
    ├── mod.rs                   # ui module re-exports
    ├── view.rs                  # top-level view entry
    ├── animation.rs             # ANIM_* duration tokens
    ├── border_bar.rs            # top-border gradient progress bar
    ├── scroll_anim.rs
    ├── system_default_font.rs   # OS-native default font (fc-match / NSFont / SPI)
    ├── font_autopick.rs         # locale-aware fontdb fallback
    ├── color/
    │   ├── mod.rs
    │   ├── theme.rs             # accent-color → iced palette generation, ThemeMode
    │   └── hct.rs               # HCT perceptually-uniform color space
    ├── settings/
    │   ├── mod.rs
    │   ├── general.rs           # general settings
    │   ├── download.rs          # download settings
    │   ├── bittorrent.rs        # BT settings (incl. tracker sync)
    │   ├── ed2k.rs              # ED2K settings (incl. server.met / nodes.dat picker)
    │   ├── network.rs           # proxy settings
    │   └── advanced.rs          # advanced settings (paths, browser takeover, logs)
    ├── icon.rs                  # iced_lucide icon font module (build-generated)
    ├── icons.rs                 # icon glyph constants + layout widths
    ├── dims.rs                  # shared dimension constants
    ├── title_bar.rs             # custom frameless title bar + window controls
    ├── resize_frame.rs          # custom resize handles for the frameless window
    ├── close_dialog.rs          # close-confirmation overlay
    ├── confirm_dialog.rs        # generic confirm overlay
    ├── sidebar.rs               # nav: Tasks / New / About / Settings
    ├── category_bar.rs          # task filters (All / Downloading / Completed / Failed) + settings categories
    ├── task_list.rs             # download cards with progress, actions, sort menu
    ├── add_dialog.rs            # new-download overlay (url / torrent / split / advanced)
    ├── details_dialog.rs        # task details: summary / activity / files / Advanced tabs
    ├── sort.rs                  # task sorting comparators
    ├── about_dialog.rs          # about / engine info overlay
    ├── update_dialog.rs         # app / engine update overlay
    └── components/              # reusable widgets
        ├── dialog.rs / popover.rs / toast.rs                    # overlays / floating layer / toasts
        ├── path_picker.rs / color_picker.rs / font_picker.rs / tag_picker.rs / drop_down.rs / number_stepper.rs  # pickers
        ├── secret_input.rs / ctx_input.rs / ctx_menu.rs / copyable_text.rs / truncated_text.rs / translate.rs   # text input
        ├── file_drop_zone.rs / torrent_upload.rs / drop_overlay.rs  # drag and drop
        ├── file_tree.rs / tri_checkbox.rs / torrent_file_list.rs / piece_map.rs / key_value_list.rs / logo.rs  # task detail
        ├── speed_hud.rs / speed_popover.rs / shutdown_popover.rs  # HUD
        ├── status_dot.rs / spinner.rs / swatch_icon.rs / scale.rs  # decoration
        ├── expand.rs / slim_scrollable.rs / scroll_top_gap.rs / tooltip.rs  # layout
        └── mod.rs
```

## Build & Run

Requirements: a recent stable Rust toolchain (`rustup` recommended). X11/Wayland dev packages may be
needed on Linux. **No network access is required at build time** — `build.rs` only generates the icon
module; the aria2-next binary is fetched on first run.

```bash
cargo build                # debug build
cargo run --               # launch app (fetches aria2-next on first launch)
cargo build --release      # release build (aggressive: fat LTO, strip, panic=abort)
```

## Packaging

Installers are produced with [cargo-packager](https://github.com/crabnebula-dev/cargo-packager)
(`cargo install cargo-packager --locked`), configured by `packager.toml`. Release binaries are built
per-platform on GitHub Actions (`.github/workflows/release.yml`) — Linux `.deb`/`.AppImage`, Windows
NSIS `.exe` — and uploaded as build artifacts (attached to a GitHub Release on tag push).

```bash
cargo packager --release --config packager.toml --formats deb,appimage   # Linux
cargo packager --release --config packager.toml --formats nsis           # Windows
```

Packages contain only the binary (fonts, icons, and i18n are compile-time embedded). **Vulkan is
required at runtime** on Linux (iced/wgpu loads it via `dlopen`); the aria2-next binary is fetched at
runtime and is intentionally not bundled. `deb.depends` is minimal because iced links only the C
runtime — the deb cannot enforce the `dlopen`'d GTK/X11/Vulkan libraries.

> The versions in current GitHub Releases are **development builds**, used to exercise the update mechanism; they do not represent stable releases.

To use a local aria2-next binary instead of the auto-download:

```bash
ARIA2_BIN=/path/to/aria2-next cargo run --
```

## Checks

```bash
cargo test --workspace                                          # unit / integration tests
cargo test --test download_e2e -- --nocapture                   # end-to-end download tests (requires ARIA2_BIN)
cargo clippy --workspace                                        # lint (no warnings allowed)
cargo fmt --check                                               # formatting check
```

## Configuration

Settings are persisted as JSON under the platform config dir (`directories` crate,
`ProjectDirs::from("dev", "remotrix", "Remotrix")`):

- Linux: `~/.config/remotrix/settings.json`
- macOS: `~/Library/Application Support/dev.remotrix.Remotrix/settings.json`
- Windows: `%APPDATA%\remotrix\Remotrix\config\settings.json`

Runtime data (SQLite database, aria2-next binary cache + session, log files) lives under the data dir:

- Linux: `~/.local/share/remotrix/`
- macOS: `~/Library/Application Support/dev.remotrix.Remotrix/`
- Windows: `%APPDATA%\remotrix\Remotrix\data\`

Persisted settings include the download folder, max concurrent downloads, split, global & per-task
speed limits, theme mode + selected light/dark themes, follow-system-accent toggle, locale, font
choice, auto-update preferences and silent-update scope, close-to-tray, and a full set of aria2
options (max-connection-per-server, min-split-size, auto-file-renaming, allow-overwrite, continue,
check-integrity, user-agent, headers, proxy, retries, timeouts, bt-tracker, seed ratio/time, DHT,
and more).

Settings → Advanced → Paths lets you override the **aria2-next dir**, **app cache dir**, and
**log dir**. Changes do **not take effect immediately** — the UI shows a "pending changes" hint and
requires clicking **Save & restart** to commit; on next launch, if the new paths differ from the
previous `last_resolved`, Remotrix physically copies the matching data from the old paths (all files
in the aria2 bin dir, all files in the log dir, and the allow-listed children of the app cache dir:
`remotrix.db`, `ed2k-bootstrap/`, `ed2k-search/`) to the new paths. Files already present at the new
path are skipped individually; the old files are never deleted. Changing only the app cache dir
also migrates the default embedded aria2 and log dirs into the new cache dir; explicit overrides
for aria2 / log still win. The `ARIA2_BIN` env var has the highest priority and overrides the UI.

## Browser Takeover (Extension API)

Remotrix exposes a local HTTP server (default port `29110`, loopback only) that implements the full protocol expected by the upstream [motrix-next-extension](https://github.com/AnInsomniacy/motrix-next-extension) (MIT, reused unmodified):

- `GET /ping` — heartbeat (no auth)
- `GET /stat` — global stats (`downloadSpeed` / `numActive`, etc., mirroring aria2 `getGlobalStat` as strings)
- `POST /add` — add a download (referer / cookie / UA / request headers submitted with the task)
- `POST /pause-all` / `POST /resume-all` — global pause / resume

**Usage:**

1. Install `motrix-next-extension` from your browser's store (Remotrix does not bundle it).
2. In Remotrix Settings → "Advanced" → "Browser Takeover", enable it and copy the port and secret.
3. In the extension options page, point it at `http://127.0.0.1:<port>` and enter the secret; `checkConnection` should pass.

By default `/add` **silently auto-submits** the task and fires a system notification; you can switch to a confirmation dialog in Settings instead. **The secret is always required**: if none is set, one is auto-generated and persisted at startup; `/stat`, `/add`, `/pause-all`, and `/resume-all` all require `Authorization: Bearer <secret>` (the `/ping` heartbeat stays open). Only global pause / resume is exposed; manage individual tasks back in the Remotrix main window.

After changing **Enabled / port / secret / auto-submit**, clicking **Apply** **hot-restarts** the extension service immediately (no app restart needed) and shows a toast ("Restarting… / restarted / restart failed"); toggling the enable switch off stops the service and shows a "stopped" toast. This only happens on Apply — editing the toggles or inputs alone does not restart.

> Known limitation: the `motrixnext://` deep-link protocol is not implemented, so the "Open motrix-next" wake-up button is currently unavailable; download takeover goes over HTTP and is unaffected.

## Tech Stack

| Component | Choice | Rationale |
|---|---|---|
| GUI | `iced 0.14` (+tokio, advanced, canvas, svg, markdown) | Pure Rust, widget-based, dark theme support |
| Engine | `aria2-next` sidecar + `aria2-ws 0.5` | C++ aria2 fork, JSON-RPC over WebSocket, spawned as subprocess |
| Async | `tokio 1.x` (full) | Shared runtime for engine + UI |
| Persistence | `rusqlite 0.40` (bundled + fallible_uint) | Embedded SQLite for task metadata / progress |
| Theming | iced `Theme::custom` + `palette 0.7` | Accent-color → palette; HCT perceptually-uniform derivation for light/dark |
| System preferences | `mundy 0.2` (color-scheme + accent-color + async-io) | Unified system dark/light + OS accent color reading (replaces dark-light) |
| i18n | `fluent-templates 0.15` | Fluent translations (zh / en) |
| Fonts | `fontdb 0.24` + `system-fonts 0.1` | Locale-aware system font enumeration; OS-native default font API |
| File dialog | `rfd 0.17` | Native OS file picker |
| System tray | `ldtray 0.1` (pinned `=0.1.2`) | Tray icon + menu; Windows watchdog auto-restores after Explorer restarts |
| Notifications | `notify-rust 4.17` (tokio) | Cross-platform native notifications; Windows additionally uses `win_toast` (AUMID + Locate / Reveal) |
| Single instance | `app-single-instance 0.1` | Single-instance run, focuses existing window |
| HTTP client | `reqwest 0.13` (rustls, json, socks) | GitHub Releases fetch / updater / proxy |
| HTTP server | `salvo 0.95` (cors + http1 + server-handle) | Browser extension loopback API with hot-restart |
| Hashing | `sha2 0.11` | aria2-next / app-update binary checksum verification |
| Icons | `iced_lucide 0.1`, `iced_aw 0.14` | Icon font + time picker |
| Animations | `iced_anim =0.3.1` | 7 unified duration tokens |
| Image | `image 0.25` (png) | App icon loading |
| Logging | `tracing` + `tracing-appender 0.2` + `tracing-subscriber 0.3` | Rolling file logs + env-filter |
| Config dirs | `directories 6` | XDG / user data paths |
| Process | `libc 0.2` | SIGTERM/SIGKILL stale aria2-next processes |
| Time | `chrono 0.4` (clock) | Timestamp formatting |
| File / dir | `open 5` | Open file / directory / URL |
| Windows | `windows 0.62`, `libloading 0.9` | Taskbar watchdog, AUMID, native toasts, sleep inhibition |

## Roadmap

**Done**

- **Architecture & UI** — dual-loop engine bridge + aria2-next sidecar supervisor; basic UI (sidebar, category bar, task list, add dialog, settings); frameless window + custom title bar + resize handles; single-instance run; unified animation system (7 `ANIM_*` tokens covering cards / dialogs / progress bars / speed HUD / category capsules / toasts / overlays).
- **Engine & protocols** — aria2-next runtime fetch + sha256 verify + self-heal + auto-update; task metadata and progress persisted to SQLite; ED2K (automatic `server.met` / `nodes.dat` bootstrap + built-in search), Metalink, BT tracker auto-sync, BT metadata-only; HTTP / SOCKS5 global + per-task proxy with Basic Auth.
- **Desktop integration** — system tray (X11 full, Wayland minimize-only, Windows with `TaskbarCreated` watchdog); cross-platform system notifications (Windows additionally uses `win_toast`); clipboard link auto-detection; app-wide file drop with overlay hint.
- **Theme & localization** — i18n (zh / en); accent-color theme with system appearance / accent-color follow + smooth transition animations; custom accent color with HCT derivation; default font follows the OS (Linux `fc-match` / macOS NSFont / Windows SPI, with fontdb fallback).
- **Automation & system** — scheduled speed limits by time window + weekday; scheduled / all-tasks-complete shutdown; cross-platform system sleep inhibition while downloads are active; launch at login; Settings → Advanced → Paths to override aria2 / app cache / log dirs with automatic data migration on next launch.
- **Updates** — AppImage / deb / NSIS app self-update via the shared background downloader (throttling / retry / streaming sha256 / Range resume), surfacing a sticky toast with an "Update" action button that triggers apply on click; aria2 silent update with background staging + system-notification restart prompt; customizable RPC port with automatic free-port fallback; unified top-border progress bar for background tasks.
- **Settings expansion & UX details** — multi-tab task details (BT piece map, file tree, Advanced UA / cookie / headers); per-task tri-state checkbox file selection; category filter capsules with count badges; sort / path-picker history / right-click context menu; settings additions (auto-clean, concurrency cap, per-task limits, file allocation, log levels, changelog rendering, etc.); graceful shutdown; "About" dialog.
- **Tests** — unit tests for engine / task / scheduler and other core modules; `download_e2e` end-to-end (requires `ARIA2_BIN`).

**Planned**

- [ ] Download hardening & full testing — HTTP/HTTPS and BT work today, but haven't covered enough scenarios (resume, integrity check, speed limits, retry on failure, etc.); needs a full regression pass
- [ ] System-level unit tests — fill in unit / integration tests for modules not yet covered, such as `db.rs`, `app.rs`, the dialog views, `tray.rs`, and the Salvo handlers in `extension_api.rs`
- [ ] File associations — set the default program for opening file types (e.g. `.torrent`)
- [ ] `motrixnext://` deep-link protocol — wake the app when not running / website download buttons (the core HTTP interception does not depend on it)
- [ ] Wayland tray compatibility polish — under Wayland the window can only be minimized and not fully hidden to the tray; currently wakes via system notification; improve window-hiding / tray / notification behavior
- [ ] UI/UX polish — continuously refine visuals and interactions

## Acknowledgements

- [aria2-next](https://github.com/AnInsomniacy/aria2-next) by AnInsomniacy — the download engine at the core. It is a separate, independently licensed program (GPL-2.0-or-later) that is **not bundled** with Remotrix; it is downloaded from its GitHub Releases at runtime.
- [motrix-next-extension](https://github.com/AnInsomniacy/motrix-next-extension) (MIT) — the browser-side extension. Remotrix neither bundles nor modifies it; users install it from the store / upstream and it talks to Remotrix through the local extension API.

## License

MIT. See the `LICENSE` file for the full license text.