# Remotrix — AI Agent Context

## Project Overview
Rust-native desktop download manager inspired by Motrix.app. Built with `iced` GUI framework and **aria2-next sidecar** (via `aria2-ws` RPC client).

| Component | Choice | Rationale |
|---|---|---|---|
| GUI | `iced 0.14` (+tokio, advanced, image, canvas) | Pure Rust, widget-based, multi-theme support |
| Engine | `aria2-next` sidecar + `aria2-ws 0.5` | C++ aria2 fork, JSON-RPC over WebSocket; spawned as subprocess |
| Async | `tokio 1.x` (full) | Shared runtime for engine + UI |
| Persistence | `rusqlite 0.32` (bundled) | Embedded SQLite for task metadata / progress |
| Themes | iced `Theme::custom` (built-in) + `dark-light 1.1` | Accent-color swatches; iced auto-generates light/dark palettes (primary + M3-style surface background) from the accent; system detection |
| i18n | `fluent-templates 0.14` | Fluent translations (zh/en) |
| File dialog | `rfd 0.15` | Native OS file picker |
| Config dirs | `directories 5` | XDG/user data paths |

## Architecture: aria2-next sidecar
- **iced UI loop** runs on the main thread
- **tokio runtime** manages the aria2-next subprocess + WebSocket RPC + progress polling
- Communication via `tokio::sync::mpsc` channels (unbounded)
- **GUI → Engine**: `EngineCmd` via `mpsc::Sender`
- **Engine → GUI**: `EngineEvent` via `mpsc::Receiver`, consumed by `iced::Subscription`
- `aria2_fetcher::ensure_aria2_next()` fetches the aria2-next binary at runtime from GitHub Releases (first launch), caches in `<data_dir>/aria2/`
- Engine degrades gracefully on fetch/spawn failure (no exit), retryable via `RetryAria2Fetch`
- Update check → background stage download → write `.pending-update` → next restart/engine restart applies pending update
- Task persistence via aria2 `--save-session`/`--input-file`
- `src/updater.rs` provides reusable `fetch_latest_release` / `ReleaseInfo` for both aria2 and future app updates

```rust
// --- Channel Protocol (must match between engine.rs and message.rs) ---
enum EngineCmd {
    AddDownload { urls: Vec<String>, save_dir: PathBuf, split: u16, advanced: TaskAdvancedOptions, bt_metadata_only: bool },
    AddTorrent { path: PathBuf, save_dir: PathBuf, split: u16, advanced: TaskAdvancedOptions, select_files: Option<Vec<u64>> },
    Pause(String), Resume(String),
    Remove { gid: String, delete_files: bool },
    PauseAll, ResumeAll,
    RemoveAll { delete_files: bool },
    Snapshot,
    PurgeResults(Vec<String>),
    ApplyAria2Options { options: TaskOptions },
    FollowTorrent { gid: String, path: PathBuf, save_dir: PathBuf, split: u16, advanced: TaskAdvancedOptions, delete_after: bool },
    SelectFiles { gid: String, files: Vec<u64> },
    FetchTaskDetails(String),
    ReaddTask { gid: String, url: String, save_dir: PathBuf, split: u16, paused: bool, bt_metadata_only: bool },
    Redownload { gid: String, url: String, save_dir: PathBuf, split: u16, bt_metadata_only: bool },
    Shutdown,
    ForceKill,
    CheckAria2Update,
    RetryAria2Fetch,
    RestartEngine,
    ResumeGids(Vec<String>),
    CheckMissingFiles,
    ReloadSchedules,
}
enum EngineEvent {
    Added { gid: String, name: String, url: String, dir: String, info_hash: Option<String> },
    Progress { gid: String, name: String, downloaded: u64, total: u64, speed: u64, upload_speed: u64, status: String, connections: u64, info_hash: Option<String> },
    TorrentAdded { gid: String, path: PathBuf },
    Removed(String),
    TaskDetails { gid: String, details: crate::task::TaskDetails },
    TaskDetailsFailed { gid: String },
    SelectFilesFailed { gid: String },
    EngineReady, SyncComplete, EngineStopped,
    Aria2Status { stage: String, message: String },
    Aria2Version { version: String },
    Aria2CheckResult { current: String },
    Aria2UpdateApplied { version: String },
    Aria2UpdateFailed { error: String },
    Aria2FetchFailed { error: String },
    GlobalSpeed { download: u64, upload: u64 },
    Aria2UpdateStaged { version: String },
    EngineDegraded { reason: String },
    FilesMissing { gids: Vec<String> },
}
```

## aria2-ws API Reference
- `aria2_ws::Client::connect(url, token)` — connect to WebSocket RPC; token is `Option<&str>` (rpc-secret)
- `add_uri(uris, options, position, callbacks)` → `Result<String>` (GID)
- `pause(gid)`, `unpause(gid)`, `remove(gid)`, `force_remove(gid)`, `shutdown()` → `Result<()>`
- `tell_status(gid)` → `Result<Status>` with fields: `gid`, `status` (TaskStatus enum), `total_length`, `completed_length`, `download_speed` (all `u64`), `dir`, `files`, `bittorrent`
- `tell_active()` → `Result<Vec<Status>>`, `tell_waiting(offset, num)`, `tell_stopped(offset, num)`
- `change_global_option(options: TaskOptions)` for global speed limits
- `subscribe_notifications()` → `broadcast::Receiver<Notification>` (Start/Pause/Complete/Error/Stop/BtComplete)
- `TaskStatus` variants: `Active`, `Waiting`, `Paused`, `Complete`, `Error`, `Removed`
- `TaskOptions`: header, split, all_proxy, dir, out, gid, continue, auto_file_renaming, max_download_limit, max_connection_per_server, max_tries, timeout, extra_options (Map)
- `Status.status` is `aria2_ws::response::TaskStatus` — serialized as lowercase string matching standard aria2 status strings

Quick start pattern:
```rust
use aria2_ws::{Client, TaskOptions};
let client = Client::connect("ws://127.0.0.1:6800/jsonrpc", Some("secret")).await?;
let opts = TaskOptions { split: Some(4), ..Default::default() };
let gid = client.add_uri(vec!["http://..."], Some(opts), None, None).await?;
let status = client.tell_status(&gid).await?;
```

## Cargo.toml Dependencies
```toml
[package] name = "remotrix" version = "0.1.0" edition = "2021" license = "MIT"
[dependencies]
aria2-ws = "0.5"
iced = { version = "0.14", features = ["tokio", "advanced", "canvas", "svg"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
directories = "5"
rfd = "0.15"
image = { version = "0.24", default-features = false, features = ["png"] }
dark-light = "1.1"
fluent-templates = "0.14.0"
futures = "0.3"
base64 = "0.22"
hex = "0.4"
num-traits = "0.2"
iced_aw = { version = "0.14", default-features = false, features = ["time_picker"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
sha2 = "0.10"
rusqlite = { version = "0.32", features = ["bundled"] }
chrono = { version = "0.4", default-features = false, features = ["clock"] }
open = "5"
libc = "0.2"
fontdb = "0.23"

[build-dependencies]
iced_lucide = "0.1"
```

## Code Conventions
- **Module structure**: `src/` with flat top-level modules (`app.rs`, `config.rs`, `db.rs`, `engine.rs`, `aria2_fetcher.rs`, `updater.rs`, `message.rs`, `task.rs`, `i18n.rs`, `clipboard_watch.rs`, `logging.rs`, `scheduler.rs`, `torrent_meta.rs`, `trackers.rs`) + `ui/` subdirectory
- **UI pattern**: Each page is a `fn` returning `iced::Element<'_, Message, Theme>`; no widget OOP wrappers
- **Time pickers** (`Settings > Download > Speed Limits`): `iced_aw` clock component (`time_picker` feature) wrapped in `src/ui/components/time_picker.rs`; the wrapper re-seeds iced_aw state on the open transition via `tree.children[0].state` so reopening shows the committed value.
- **Theme**: single accent color → iced `Theme::custom` palette generation (`src/ui/theme.rs`), with the background derived as an M3-style surface from the accent hue (`surface_from_seed`); colors read from `iced::Theme::extended_palette()`, no hardcoded palette constants.
- **Naming**: `snake_case` for fns/vars, `PascalCase` for types/enums, `SCREAMING_SNAKE` for constants
- **Error handling**: Use `String` errors in engine layer, map to `EngineEvent::EngineStopped` for fatal
- **No comments** in source code unless explaining a non-obvious design decision
- **Imports**: Group as `std` → external crates → `crate::` (blank-line separated)

## Build / Check Commands
```bash
cargo build                    # debug build (no network; aria2-next fetched at runtime)
cargo build --release          # release build
cargo run --                   # run app
cargo clippy --workspace       # lint (no warnings allowed)
cargo fmt --check              # formatting check
cargo packager --release --config packager.toml --formats deb,appimage   # local Linux packaging
```
Run `/check-docs` (Kilo command) to audit README.md and this file against the codebase.

## Release profile
- `[profile.release]` (Cargo.toml) is **Aggressive**: `lto="fat"`, `codegen-units=1`, `panic="abort"`,
  `strip="symbols"`, `debug=false`, `overflow-checks=true`.
- Tradeoffs: `panic="abort"` removes Rust panic **backtraces** (errors still surface via `EngineEvent`);
  `strip="symbols"` removes debug symbols; fat LTO increases build time for a smaller binary.

## Packaging / CI
- `packager.toml` configures **cargo-packager** (schema follows cargo-packager 0.11.x; `Packager.toml` is
  the default filename, we use `packager.toml` + `--config` explicitly).
- `version` in `packager.toml` **must stay in sync** with `Cargo.toml` (currently `0.1.0`).
- `deb.depends` is intentionally minimal (`libc6`, `libgcc-s1`): `ldd` shows iced 0.14 links only the C
  runtime — GTK/X11/Vulkan are loaded via `dlopen` at runtime, so they can't be enforced as deb deps.
  Vulkan is a runtime requirement (see README).
- Windows NSIS needs `assets/icon.ico` (committed; generated from `icon.png`). Regenerate with:
  `python3 -c "from PIL import Image; Image.open('assets/icon.png').convert('RGBA').save('assets/icon.ico', format='ICO', sizes=[(16,16),(32,32),(48,48),(64,64),(128,128),(256,256)])"`.
- CI: `.github/workflows/release.yml` builds Linux (deb+appimage) and Windows (nsis) natively and uploads
  artifacts; on tag push it attaches installers to a GitHub Release. Requires a git remote to run.
- All runtime assets are compile-time embedded — packages ship only the binary. aria2-next is NOT bundled
  (fetched at runtime).
- The app also installs a per-user `.desktop` at runtime (`src/config.rs` `install_desktop_file()`); a
  packaged `.deb` provides its own desktop entry, so the runtime one may overlap — handle if this becomes
  an issue.

## Build Process (build.rs)
- Build-time only generates the icon module (`iced_lucide::build`)
- **No network access** during build — offline `cargo build` always succeeds
- aria2-next binary is fetched at **runtime** by `aria2_fetcher::ensure_aria2_next()`:
  - First launch: downloads from GitHub Releases (`AnInsomniacy/aria2-next`) to `<data_dir>/aria2/`
  - Cached across runs with `.installed` version/sha256 tracking
  - Supports `ARIA2_BIN` env var to skip download entirely
- Update workflow: `updater::fetch_latest_release()` → background stage download → write `.pending-update` → next restart/engine restart applies pending update

## Risks to Watch
- `aria2-next` GitHub Releases may be temporarily unavailable → `ensure_aria2_next()` error at runtime with clear message; `ARIA2_BIN` env var fallback or manual binary placement
- Large task lists may lag iced → use `scrollable` + cap visible items
- No system tray support in iced → defer or use `tray-icon` crate separately
- `Secret` passed as CLI argument visible in `ps` on debug builds — acceptable (random per-session, local only)
