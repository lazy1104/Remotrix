# Plan: Refresh README for the aria2-next sidecar architecture

## Goal & Scope (confirmed with user: FULL refresh)

`README.md` is stale: it still describes the abandoned `aria2-core` Rust engine.
Rewrite it to reflect the current **aria2-next sidecar + `aria2-ws` RPC** architecture
and fix every other section that drifted (dependency versions, tech stack, code layout,
features, config/data dirs, roadmap). Single-file documentation change only — no source edits.

## Verified discrepancies (README vs actual code)

| Section | README (stale) | Actual (verified) |
|---|---|---|
| Engine | `aria2-core 0.2.3` (Rust rewrite) | aria2-next sidecar (C++ aria2 fork) driven via `aria2-ws 0.5` WebSocket RPC; spawned as subprocess with a tokio supervisor (`src/engine.rs`) |
| GUI | `iced 0.13` | `iced 0.14` (+tokio, advanced, image, **canvas**) |
| System theme | `dark-light 2` | `dark-light 1.1` |
| Themes | Motrix dark/light palette (custom `Theme` struct) | `opaline 0.4` builtin themes via iced adapter (`src/ui/theme.rs`); no hardcoded palette constants |
| Persistence | not mentioned | `rusqlite 0.32` (bundled) SQLite in `src/db.rs`; meta + progress flushed every 1s |
| Updater | not mentioned | `src/aria2_fetcher.rs` + `src/updater.rs`: runtime fetch from GitHub Releases (`AnInsomniacy/aria2-next`), sha256 verify, `.installed`/`.pending-update` cache, `ARIA2_BIN` override, auto-update check + staged apply on restart |
| Logging | not mentioned | `tracing` + `tracing-appender 0.2` daily rolling file logs |
| Magnet | claimed as supported | NOT implemented (engine has `AddDownload`/`AddTorrent` only; `add_magnet` unused) → move to roadmap |
| Code layout | missing 3 top-level + 7 ui files | add `aria2_fetcher.rs`, `db.rs`, `updater.rs`, `ui/{mod,icon,icons,category_bar,details_dialog,piece_map,sort,about_dialog}.rs` |

## Confirmed facts (to encode verbatim)

- **Config path** (`ProjectDirs::from("dev","remotrix","Remotrix")`, verified against `directories 5.0.1` source + on-disk files):
  - Linux: `~/.config/remotrix/settings.json` (Linux uses only lowercased `application` = `remotrix`)
  - macOS: `~/Library/Application Support/dev.remotrix.Remotrix/settings.json` (bundle id `dev.remotrix.Remotrix`)
  - Windows: `%APPDATA%\remotrix\Remotrix\config\settings.json` (path `remotrix\Remotrix`, config subdir)
- **Data dir** (db, `aria2/` binary cache+session, `logs/`):
  - Linux: `~/.local/share/remotrix/`
  - macOS: `~/Library/Application Support/dev.remotrix.Remotrix/`
  - Windows: `%APPDATA%\remotrix\Remotrix\data\`
- **Sidebar nav**: Tasks / New / About / Settings (icon buttons, `src/ui/sidebar.rs`)
- **Task filters** (`category_bar.rs`): All / Downloading / Completed (with counts)
- **Settings categories**: General / Download / BitTorrent / eD2k / Network / Advanced
- **Sort fields**: added time, name, size, progress, status (asc/desc)
- **Details dialog tabs**: Summary / Activity / Files; `TaskDetails` = bitfield, num_pieces, piece_length, files, upload_speed, num_seeders, info_hash, error_code, error_message; `piece_map.rs` = BT piece canvas
- **Build**: `build.rs` only generates the iced_lucide icon module (`fonts/icons.toml`); **no network at build time**. aria2-next binary is fetched at **runtime** on first launch.
- **Dependencies** (from `Cargo.toml`): aria2-ws 0.5, iced 0.14, tokio 1 (full), serde/serde_json, anyhow, tracing + tracing-subscriber 0.3 + tracing-appender 0.2, directories 5, rfd 0.15, image 0.24, dark-light 1.1, fluent-templates 0.14, futures 0.3, base64 0.22, hex 0.4, num-traits 0.2, iced_aw 0.14 (number_input, drop_down), reqwest 0.12 (rustls, json), sha2 0.10, opaline 0.4 (builtin-themes, iced), rusqlite 0.32 (bundled), chrono 0.4 (clock), open 5. Build-dep: iced_lucide 0.1.
- **License**: GPL-2.0-or-later (confirmed in `Cargo.toml`).

## Implementation steps

1. Overwrite `README.md` with the content in the "Proposed README" section below (single Write).
2. Run `cargo fmt --check` and `cargo clippy --workspace` to confirm no accidental source touches (README change alone needs neither, but a clean build verifies nothing else moved).
3. Eyeball the rendered markdown for the ASCII diagram alignment.

## Out of scope

- `AGENTS.md` is also partly stale (its "Theme Constants (Motrix Dark)" block no longer matches `theme.rs`, which now uses opaline). User asked only for README; leave AGENTS.md untouched unless requested.
- No source code changes. No new screenshots.

## Validation

- `cargo build` still compiles (README edit must not break anything).
- Grep README for `aria2-core` → expect 0 hits.
- Grep README for `0.13` / `dark-light 2` → expect 0 hits.
- Confirm config-path lines match the "Confirmed facts" above.

---

## Proposed README (full content to write to `README.md`)

```markdown
# Remotrix

A Rust-native desktop download manager inspired by [Motrix](https://motrix.app/), built with the
[`iced`](https://github.com/iced-rs/iced) GUI framework and an [`aria2-next`](https://github.com/AnInsomniacy/aria2-next)
sidecar engine driven over WebSocket JSON-RPC ([`aria2-ws`](https://crates.io/crates/aria2-ws)).

> The name "Remotrix" is a portmanteau of **Rust** + **Motrix**.

## Features

- **Native Rust UI** — pure-Rust rendering via `iced 0.14`, no Electron / browser stack
- **Multi-protocol downloads** — HTTP/HTTPS/FTP and BitTorrent (`.torrent` files) through aria2-next
- **Parallel segments** — configurable split / max-connections per server
- **Global & per-task speed limits** — separate download / upload caps, persisted to disk
- **Embedded persistence** — task metadata and progress are stored in a local SQLite database and survive restarts
- **Self-managing engine** — aria2-next is fetched at runtime from GitHub Releases (sha256-verified, cached, self-healing), with automatic update checks and staged background updates applied on the next restart
- **Frameless window** — custom title bar with minimize / maximize / close controls and a close-confirmation dialog
- **Theming** — multiple light / dark themes via [`opaline`](https://crates.io/crates/opaline), or follow the system appearance (`dark-light` detection)
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
├── main.rs               # entry, tracing / file-logging init, window settings
├── app.rs                 # Remotrix state, update(), view(), subscription()
├── config.rs              # Settings (serde) load/save, aria2 option mapping, path helpers
├── db.rs                  # SQLite persistence (rusqlite): task meta + progress flush
├── engine.rs              # EngineBridge: spawn tokio supervisor + aria2-next sidecar, mpsc channels
├── aria2_fetcher.rs       # runtime fetch / cache / verify aria2-next binary, staged updates
├── updater.rs             # GitHub Releases lookup, ReleaseInfo, platform slug
├── message.rs             # Message enum + page / filter / sort / setting enums
├── task.rs                # DownloadTask model, formatters, TaskDetails / TaskFile
├── i18n.rs                # Locale detection + Fluent translations
└── ui/
    ├── mod.rs             # ui module re-exports
    ├── theme.rs           # opaline theme loading, ThemeMode, widget styles
    ├── icon.rs            # iced_lucide icon font module (build-generated)
    ├── icons.rs           # icon glyph constants + layout widths
    ├── title_bar.rs       # custom frameless title bar + window controls
    ├── close_dialog.rs    # close-confirmation overlay
    ├── sidebar.rs         # nav: Tasks / New / About / Settings
    ├── category_bar.rs    # task filters (All / Downloading / Completed) + settings categories
    ├── task_list.rs       # download cards with progress, actions, sort menu
    ├── add_dialog.rs      # new-download overlay (url / torrent / split)
    ├── details_dialog.rs  # task details: summary / activity / files tabs
    ├── piece_map.rs       # BitTorrent piece-completion canvas
    ├── sort.rs            # task sorting comparators
    ├── about_dialog.rs    # about / engine info overlay
    └── settings_page.rs   # general, download, bittorrent, network, advanced, appearance
```

## Build & Run

Requirements: a recent stable Rust toolchain (`rustup` recommended). X11/Wayland dev packages may be
needed on Linux. **No network access is required at build time** — `build.rs` only generates the icon
module; the aria2-next binary is fetched on first run.

```bash
cargo build                # debug build
cargo run --               # launch app (fetches aria2-next on first launch)
cargo build --release      # release build (optimized, LTO thin)
```

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
| GUI | `iced 0.14` (+tokio, advanced, image, canvas) | Pure Rust, widget-based, dark theme support |
| Engine | `aria2-next` sidecar + `aria2-ws 0.5` | C++ aria2 fork, JSON-RPC over WebSocket, spawned as subprocess |
| Async | `tokio 1.x` (full) | Shared runtime for engine + UI |
| Persistence | `rusqlite 0.32` (bundled) | Embedded SQLite for task metadata / progress |
| Themes | `opaline 0.4` (builtin-themes, iced) | Multiple light / dark palettes |
| i18n | `fluent-templates 0.14` | Fluent translations (zh / en) |
| System theme | `dark-light 1.1` | Detect system dark / light preference |
| File dialog | `rfd 0.15` | Native OS file picker |
| HTTP client | `reqwest 0.12` (rustls, json) | GitHub Releases fetch / updater |
| Hashing | `sha2 0.10` | aria2-next binary checksum verification |
| Icons | `iced_lucide 0.1`, `iced_aw 0.14` | Icon font + number input / drop-down |
| Image | `image 0.24` | App icon loading |
| Logging | `tracing` + `tracing-appender 0.2` | Rolling file logs |
| Config dirs | `directories 5` | XDG / user data paths |
| Time | `chrono 0.4` (clock) | Timestamp formatting |

## Roadmap

- [x] Dual-loop engine bridge + aria2-next sidecar supervisor
- [x] Basic UI: sidebar, category bar, task list, add dialog, settings
- [x] Frameless window + custom title bar
- [x] i18n (zh / en) + opaline themes + system auto theme
- [x] SQLite task persistence
- [x] aria2-next runtime auto-fetch + auto-update
- [x] Task details dialog (piece map, files, BT info)
- [ ] System tray integration (currently stubbed: "Minimize to tray" is coming soon)
- [ ] Magnet link support
- [ ] Drag-and-drop file / task support
- [ ] Polished app icon

## License

GPL-2.0-or-later. See the `license` field in `Cargo.toml`.
```

## Notes for the implementer

- The fenced ``` block above contains the **exact** README body. When writing `README.md`, do **not**
  include the surrounding triple-backtick fence — write only the markdown content inside it.
- The ASCII architecture diagram uses literal spaces; preserve them exactly so it renders aligned.
- Keep the existing trailing newline at end of file.
