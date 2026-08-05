# Remotrix

A Rust-native desktop download manager inspired by [Motrix](https://motrix.app/), built with the
[`iced`](https://github.com/iced-rs/iced) GUI framework and an [`aria2-next`](https://github.com/AnInsomniacy/aria2-next)
sidecar engine driven over WebSocket JSON-RPC ([`aria2-ws`](https://crates.io/crates/aria2-ws)).

> The name "Remotrix" is a portmanteau of **Rust** + **Motrix**.

## Why Remotrix?

Remotrix started as a learning project. I liked Motrix / Motrix-next's design, but the Tauri-based
Motrix-next did not run properly on my Windows 10 machine and had severe performance problems on my
Linux machine. I wanted to learn Rust and the `iced` GUI framework, so I decided to build a native
Rust download manager from scratch, using Motrix-next as the design reference. The app is developed
with AI assistance.

## Features

- **Native Rust UI** — pure-Rust rendering via `iced 0.14`, no Electron / browser stack
- **Multi-protocol downloads** — HTTP/HTTPS/FTP and BitTorrent (`.torrent` files) through aria2-next
- **Parallel segments** — configurable split / max-connections per server
- **Global & per-task speed limits** — separate download / upload caps, persisted to disk
- **Embedded persistence** — task metadata and progress are stored in a local SQLite database and survive restarts
- **Self-managing engine** — aria2-next is fetched at runtime from GitHub Releases (sha256-verified, cached, self-healing), with automatic update checks and staged background updates applied on the next restart
- **Frameless window** — custom title bar with minimize / maximize / close controls and a close-confirmation dialog
- **Theming** — pick an accent color (a wrapping row of swatches); iced auto-generates the full light / dark palette from it, including a M3-style surface background derived from the accent hue, and the app can follow the system appearance (`dark-light` detection)
- **Internationalization** — auto-detects `zh_CN` / `en_US` from the system locale, switchable in Settings
- **Task details** — summary / activity / files tabs with a BitTorrent piece-completion map
- **Sorting & filters** — sort by added time, name, size, progress, or status; filter by All / Downloading / Completed
- **File logging** — daily rolling logs written under the data directory

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

- **First launch** — `aria2_fetcher::ensure_aria2_next()` downloads the matching platform asset from
  `AnInsomniacy/aria2-next` GitHub Releases into `<data_dir>/aria2/`, verifies its sha256, records
  `.installed`, and marks it executable. Subsequent launches hit the cache.
- **Override** — set `ARIA2_BIN=/path/to/aria2-next` to skip the download entirely (useful for development).
- **Self-healing** — if `.installed` is missing/corrupt, the directory is scanned for a cached binary
  and `.installed` is rebuilt.
- **Updates** — `updater::fetch_latest_release()` compares versions; a newer release is downloaded in
  the background and a `.pending-update` marker is written. The pending binary replaces the active one
  on the next app / engine restart.
- **Degraded mode** — if fetch or spawn fails, the engine does not exit; the UI surfaces the error and
  offers retry (`RetryAria2Fetch`) / restart (`RestartEngine`).

### Code layout

```
src/
├── main.rs               # entry, logging init, window settings
├── app.rs                # Remotrix state, update(), view(), subscription()
├── config.rs             # Settings (serde) load/save, aria2 option mapping, path helpers
├── db.rs                 # SQLite persistence (rusqlite): task meta + progress flush
├── engine.rs             # EngineBridge: spawn tokio supervisor + aria2-next sidecar, mpsc channels
├── aria2_fetcher.rs      # runtime fetch / cache / verify aria2-next binary, staged updates
├── updater.rs            # GitHub Releases lookup, ReleaseInfo, platform slug
├── message.rs            # Message enum + page / filter / sort / setting enums
├── task.rs               # DownloadTask model, formatters, TaskDetails / TaskFile
├── i18n.rs               # Locale detection + Fluent translations
├── clipboard_watch.rs    # clipboard link detection (http/ftp/magnet/ed2k/bt) for auto-add
├── logging.rs            # tracing init, daily rolling file logs, runtime log levels
├── scheduler.rs          # speed-limit schedule window + weekday helpers
├── torrent_meta.rs       # .torrent metadata parsing (name, files, size)
├── trackers.rs           # BT tracker list parse / reduce / merge
└── ui/
    ├── mod.rs            # ui module re-exports
    ├── theme.rs          # accent-color → iced palette generation, ThemeMode, widget styles
    ├── icon.rs           # iced_lucide icon font module (build-generated)
    ├── icons.rs          # icon glyph constants + layout widths
    ├── dims.rs           # shared dimension constants
    ├── title_bar.rs      # custom frameless title bar + window controls
    ├── resize_frame.rs   # custom resize handles for the frameless window
    ├── close_dialog.rs   # close-confirmation overlay
    ├── confirm_dialog.rs # generic confirm overlay
    ├── sidebar.rs        # nav: Tasks / New / About / Settings
    ├── category_bar.rs   # task filters (All / Downloading / Completed) + settings categories
    ├── task_list.rs      # download cards with progress, actions, sort menu
    ├── add_dialog.rs     # new-download overlay (url / torrent / split / advanced)
    ├── details_dialog.rs # task details: summary / activity / files tabs
    ├── sort.rs           # task sorting comparators
    ├── about_dialog.rs   # about / engine info overlay
    ├── settings_page.rs  # general, download, bittorrent, ed2k, network, advanced, appearance
    └── components/       # reusable widgets (piece_map, path_picker, time_picker, toast, ...)
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

To use a local aria2-next binary instead of the auto-download:

```bash
ARIA2_BIN=/path/to/aria2-next cargo run --
```

## Checks

```bash
cargo test --workspace     # run tests
cargo clippy --workspace   # lint (no warnings allowed)
cargo fmt --check          # formatting check
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
speed limits, theme mode + selected light/dark themes, locale, auto-update preferences, and a full set
of aria2 options (max-connection-per-server, min-split-size, auto-file-renaming, allow-overwrite,
continue, check-integrity, user-agent, headers, proxy, retries, timeouts, bt-tracker, seed ratio/time,
DHT, and more).

## Tech Stack

| Component | Choice | Rationale |
|---|---|---|
| GUI | `iced 0.14` (+tokio, advanced, canvas, svg) | Pure Rust, widget-based, dark theme support |
| Engine | `aria2-next` sidecar + `aria2-ws 0.5` | C++ aria2 fork, JSON-RPC over WebSocket, spawned as subprocess |
| Async | `tokio 1.x` (full) | Shared runtime for engine + UI |
| Persistence | `rusqlite 0.32` (bundled) | Embedded SQLite for task metadata / progress |
| Themes | iced `Theme::custom` (built-in) | Accent-color swatches; iced auto-generates light/dark palettes |
| i18n | `fluent-templates 0.14` | Fluent translations (zh / en) |
| System theme | `dark-light 1.1` | Detect system dark / light preference |
| File dialog | `rfd 0.15` | Native OS file picker |
| HTTP client | `reqwest 0.12` (rustls, json) | GitHub Releases fetch / updater |
| Hashing | `sha2 0.10` | aria2-next binary checksum verification |
| Icons | `iced_lucide 0.1`, `iced_aw 0.14` | Icon font + time picker |
| Image | `image 0.24` (png) | App icon loading |
| Logging | `tracing` + `tracing-appender 0.2` | Rolling file logs |
| Config dirs | `directories 5` | XDG / user data paths |
| Fonts | `fontdb 0.23` | System font-family enumeration for Settings |
| Process | `libc 0.2` | SIGTERM/SIGKILL stale aria2-next processes |
| Time | `chrono 0.4` (clock) | Timestamp formatting |

## Roadmap

- [x] Dual-loop engine bridge + aria2-next sidecar supervisor
- [x] Basic UI: sidebar, category bar, task list, add dialog, settings
- [x] Frameless window + custom title bar
- [x] i18n (zh / en) + accent-color theme + system auto theme
- [x] SQLite task persistence
- [x] aria2-next runtime auto-fetch + auto-update
- [x] Task details dialog (piece map, files, BT info)
- [ ] System tray integration (currently stubbed: "Minimize to tray" is coming soon)
- [ ] Magnet link support
- [ ] Drag-and-drop file / task support
- [ ] Polished app icon

## Acknowledgements

- [aria2-next](https://github.com/AnInsomniacy/aria2-next) by AnInsomniacy — the download engine at the core. It is a separate, independently licensed program (GPL-2.0-or-later) that is **not bundled** with Remotrix; it is downloaded from its GitHub Releases at runtime.

## License

MIT. See the `LICENSE` file for the full license text.
