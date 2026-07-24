# Remotrix — AI Agent Context

## Project Overview
Rust-native desktop download manager inspired by Motrix.app. Built with `iced` GUI framework and **aria2-next sidecar** (via `aria2-ws` RPC client).

| Component | Choice | Rationale |
|---|---|---|
| GUI | `iced 0.14` (+tokio feature) | Pure Rust, widget-based, dark theme support |
| Engine | `aria2-next` sidecar + `aria2-ws 0.5` | C++ aria2 fork with JSON-RPC over WebSocket; spawned as subprocess |
| Async | `tokio 1.x` full features | Shared runtime for engine and UI |
| File dialog | `rfd 0.15` | Native OS file picker |
| Config dirs | `directories 5` | XDG/user data paths |

## Architecture: aria2-next sidecar
- **iced UI loop** runs on the main thread
- **tokio runtime** manages the aria2-next subprocess + WebSocket RPC + progress polling
- Communication via `tokio::sync::mpsc` channels (unbounded)
- **GUI → Engine**: `EngineCmd` via `mpsc::Sender`
- **Engine → GUI**: `EngineEvent` via `mpsc::Receiver`, consumed by `iced::Subscription`
- `aria2_fetcher::ensure_aria2_next()` fetches the aria2-next binary at runtime from GitHub Releases (first launch), caches in `<exe_dir>/aria2/` (falls back to `<data_dir>/aria2/`)
- Engine degrades gracefully on fetch/spawn failure (no exit), retryable via `RetryAria2Fetch`
- Update check → background stage download → write `.pending-update` → next restart/engine restart applies pending update
- Task persistence via aria2 `--save-session`/`--input-file`
- `src/updater.rs` provides reusable `fetch_latest_release` / `ReleaseInfo` for both aria2 and future app updates

```rust
// --- Channel Protocol (must match between engine.rs and message.rs) ---
enum EngineCmd {
    AddDownload { urls: Vec<String>, save_dir: PathBuf, split: u16 },
    Pause(String), Resume(String), Remove(String),
    PauseAll, ResumeAll, RemoveAll, Snapshot,
    SetSpeedLimit { download: Option<u64>, upload: Option<u64> },
    Shutdown,
    CheckAria2Update,
    RetryAria2Fetch,
    RestartEngine,
}
enum EngineEvent {
    Added { gid: String, name: String },
    Progress { gid: String, downloaded: u64, total: u64, speed: u64, status: String },
    Removed(String), EngineReady, EngineStopped,
    Aria2Status { stage: String, message: String },
    Aria2Version { version: String },
    Aria2CheckResult { current: String, latest: Option<String> },
    Aria2UpdateApplied { version: String },
    Aria2UpdateFailed { error: String },
    Aria2FetchFailed { error: String },
    Aria2UpdateStaged { version: String },
    EngineDegraded { reason: String },
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
[package] name = "remotrix" version = "0.1.0" edition = "2021"
[dependencies]
aria2-ws = "0.5"
iced = { version = "0.14", features = ["tokio", "advanced", "image"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
directories = "5"
rfd = "0.15"
base64 = "0.22"
futures = "0.3"
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls"] }
sha2 = "0.10"

[build-dependencies]
iced_lucide = "0.1"
```

## Code Conventions
- **Module structure**: `src/` with flat top-level modules (`app.rs`, `engine.rs`, `task.rs`, `message.rs`, `config.rs`) + `ui/` subdirectory
- **UI pattern**: Each page is a `fn` returning `iced::Element<'_, Message, Theme>`; no widget OOP wrappers
- **Theme**: Custom `Theme` struct (not iced built-in themes) with Motrix-dark palette (defined in `ui/theme.rs`)
- **Naming**: `snake_case` for fns/vars, `PascalCase` for types/enums, `SCREAMING_SNAKE` for constants
- **Error handling**: Use `String` errors in engine layer, map to `EngineEvent::EngineStopped` for fatal
- **No comments** in source code unless explaining a non-obvious design decision
- **Imports**: Group as `std` → external crates → `crate::` (blank-line separated)

## Theme Constants (Motrix Dark)
```rust
const BG_PRIMARY: Color = Color::from_rgb(0.12, 0.12, 0.18);     // #1e1e2e
const BG_SIDEBAR: Color = Color::from_rgb(0.09, 0.09, 0.15);    // #181825
const BG_CARD: Color = Color::from_rgb(0.15, 0.15, 0.25);       // #252540
const ACCENT: Color = Color::from_rgb(0.29, 0.56, 0.85);        // #4A90D9
const PROGRESS: Color = Color::from_rgb(0.30, 0.69, 0.31);      // #4CAF50
const SPEED: Color = Color::from_rgb(0.55, 0.76, 0.29);         // #8BC34A
const ERROR: Color = Color::from_rgb(0.96, 0.26, 0.21);         // #F44336
const PAUSED: Color = Color::from_rgb(1.00, 0.60, 0.00);        // #FF9800
const TEXT_PRIMARY: Color = Color::from_rgb(1.0, 1.0, 1.0);
const TEXT_SECONDARY: Color = Color::from_rgb(0.63, 0.63, 0.69); // #A0A0B0
const BORDER: Color = Color::from_rgb(0.18, 0.18, 0.27);        // #2D2D44
```

## Build / Check Commands
```bash
cargo build                    # debug build (downloads aria2-next binary)
cargo build --release          # release build
cargo run --                   # run app
cargo clippy --workspace       # lint (no warnings allowed)
cargo fmt --check              # formatting check
```

## Build Process (build.rs)
- Build-time only generates the icon module (`iced_lucide::build`)
- **No network access** during build — offline `cargo build` always succeeds
- aria2-next binary is fetched at **runtime** by `aria2_fetcher::ensure_aria2_next()`:
  - First launch: downloads from GitHub Releases (`AnInsomniacy/aria2-next`) to `<exe_dir>/aria2/`
  - Falls back to `<data_dir>/aria2/` when exe dir is not writable
  - Cached across runs with `.installed` version/sha256 tracking
  - Supports `ARIA2_BIN` env var to skip download entirely
- Update workflow: `updater::fetch_latest_release()` → background stage download → write `.pending-update` → next restart/engine restart applies pending update

## Risks to Watch
- `aria2-next` GitHub Releases may be temporarily unavailable → `ensure_aria2_next()` error at runtime with clear message; `ARIA2_BIN` env var fallback or manual binary placement
- Large task lists may lag iced → use `scrollable` + cap visible items
- No system tray support in iced → defer or use `tray-icon` crate separately
- `Secret` passed as CLI argument visible in `ps` on debug builds — acceptable (random per-session, local only)
