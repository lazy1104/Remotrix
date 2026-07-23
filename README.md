# Remotrix

A Rust-native desktop download manager inspired by [Motrix](https://motrix.app/), built with the [`iced`](https://github.com/iced-rs/iced) GUI framework and the [`aria2-core`](https://crates.io/crates/aria2-core) download engine.

> The name "Remotrix" is a portmanteau of **Rust** + **Motrix**.

## Features

- **Native Rust UI** — pure-Rust rendering via `iced`, no Electron / browser stack
- **Multi-protocol downloads** — HTTP/HTTPS, FTP, BitTorrent & magnet links (via `aria2-core`)
- **Parallel segments** — configurable split / max-connections per task
- **Global speed limits** — separate download / upload caps, persisted to disk
- **Frameless window** — custom title bar with minimize / maximize / close controls and a close-confirmation dialog
- **Internationalization** — auto-detects `zh_CN` / `en_US` from the system locale, switchable in Settings
- **Light / Dark theme** — choose manually or follow the system appearance (`dark-light` detection)

## Screenshots

Screenshots pending — see `assets/icon.png` for the app logo.

## Architecture

Remotrix uses a **dual event loop** design:

- The **iced UI loop** runs on the main thread
- A **tokio runtime** drives an `aria2-core::DownloadEngine` on a background thread
- The two halves communicate over `tokio::sync::mpsc` channels:
  - GUI → Engine: `EngineCmd` via `mpsc::Sender`
  - Engine → GUI: `EngineEvent` polled by an `iced::Subscription`

```
┌──────────────┐  EngineCmd   ┌──────────────────┐
│   iced UI    │ ───────────► │  DownloadEngine  │
│  (main loop) │              │  (tokio worker)  │
│              │ ◄─────────── │                  │
└──────────────┘  EngineEvent └──────────────────┘
                  (Subscription)
```

### Code layout

```
src/
├── main.rs              # entry, tracing init, window settings
├── app.rs              # Remotrix state, update(), view(), subscription()
├── config.rs          # Settings (serde) load/save to config dir
├── engine.rs          # EngineBridge: spawn tokio + aria2-core, mpsc
├── message.rs         # Message enum + route/key enums
├── task.rs            # DownloadTask model + formatters
├── i18n.rs            # Locale detection + Fluent translations
└── ui/
    ├── theme.rs       # Motrix dark/light palettes, ThemeMode
    ├── title_bar.rs   # custom frameless title bar + window controls
    ├── close_dialog.rs# close confirmation overlay
    ├── sidebar.rs     # nav: All / Downloading / Completed / Settings
    ├── task_list.rs   # download cards with progress, actions
    ├── add_dialog.rs  # new-download overlay (url / torrent / split)
    └── settings_page.rs # general, speed limits, appearance, about
```

## Build & Run

Requirements: a recent stable Rust toolchain (`rustup` recommended). X11/Wayland dev packages may be needed on Linux.

```bash
cargo build                # debug build
cargo run --               # launch app
cargo build --release      # release build (optimized, LTO thin)
```

## Checks

```bash
cargo test --workspace     # run tests
cargo clippy --workspace    # lint (no warnings allowed)
cargo fmt --check           # formatting check
```

## Configuration

Settings are persisted as JSON under the platform config dir:

- Linux: `~/.config/remotrix/settings.json`
- macOS: `~/Library/Application Support/dev.remotrix.Remotrix/settings.json`
- Windows: `%APPDATA%\remotrix\Remotrix\config\settings.json`

Persisted fields include the download folder, max concurrent downloads, speed limits, theme mode, and language.

## Tech Stack

| Component | Choice | Rationale |
|---|---|---|
| GUI | `iced 0.13` (+tokio, advanced) | Pure Rust, widget-based, dark theme support |
| Engine | `aria2-core 0.2.3` | Rust rewrite of aria2, tokio-based |
| Async | `tokio 1.x` (full) | Shared with aria2-core runtime |
| File dialog | `rfd 0.15` | Native OS file picker |
| Config dirs | `directories 5` | XDG / user data paths |
| System theme | `dark-light 2` | Detect system dark/light preference |
| Image | `image 0.24` | App icon loading |

## Roadmap

- [x] Workspace scaffolding + dual-loop engine bridge
- [x] Basic UI: sidebar, task list, add dialog, settings
- [x] Frameless window + custom title bar
- [x] i18n (zh / en) + light/dark auto theme
- [ ] System tray integration (currently stubbed: "Minimize to tray" is coming soon)
- [ ] torrent detailed management
- [ ] Drag-and-drop file/task support
- [ ] Polished app icon

## License

GPL-2.0-or-later. See the `license` field in `Cargo.toml`.