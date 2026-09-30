# Remotrix — Architecture & Dependencies

Part of the AI context doc set; index in `AGENTS.md`. Read this before touching the engine layer,
the RPC protocol, build/runtime fetch, or the release profile.

---

## Architecture: aria2-next sidecar
- **iced UI loop** runs on the main thread
- **tokio runtime** manages the aria2-next subprocess + WebSocket RPC + progress polling
- Communication via `tokio::sync::mpsc` channels (unbounded)
- **GUI → Engine**: `EngineCmd` via `mpsc::Sender`
- **Engine → GUI**: `EngineEvent` via `mpsc::Receiver`, consumed by `iced::Subscription`
- `aria2_fetcher::ensure_aria2_next()` fetches the aria2-next binary at runtime from GitHub Releases (first launch), caches in `<data_dir>/aria2/`
- Engine degrades gracefully on fetch/spawn failure (no exit), retryable via `RetryAria2Fetch`
- Update check is **app-layer orchestrated** (`app.rs check_updates`): fetch releases via `updater` → non-silent/app updates open a dialog; silent aria2 auto-stages via `DownloadAria2Update` → `.pending-update` → next restart/engine restart applies pending update. App self-update stages a raw binary (`app_updater`) and swaps on relaunch.
- Task persistence via aria2 `--save-session`/`--input-file`
- `src/extension_api.rs` runs a Salvo HTTP server on `127.0.0.1:<port>` (default `29110`) implementing the `motrix-next-extension` (MIT, reused unmodified) protocol: `/ping` (no auth), `/stat`, `/add`, `/pause-all`, `/resume-all` (Bearer auth when a secret is configured). `/add` auto-submits via `EngineCmd::AddExternalDownload` (from_browser=true → toast + optional system notification) or, when auto-submit is off, routes the request into the Add dialog via an mpsc message channel + `Message::Extension(ExtensionMsg::ShowAddDialog)`. Shared `GlobalStatCache` is refreshed by the app on `GlobalSpeed`/task changes.
- `src/updater.rs` provides reusable `fetch_latest_release` / `ReleaseInfo` for both aria2 and future app updates

```rust
// --- Channel Protocol (must match between engine.rs and message.rs) ---
enum EngineCmd {
    AddDownload { urls: Vec<String>, save_dir: PathBuf, split: u16, advanced: TaskAdvancedOptions, bt_metadata_only: bool },
    AddExternalDownload { urls: Vec<String>, save_dir: PathBuf, split: u16, advanced: TaskAdvancedOptions, headers: Vec<(String, String)>, bt_metadata_only: bool },
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
    DownloadAria2Update { version: String, asset_name: String, download_url: String, sha256: Option<String> },
    RetryAria2Fetch,
    RestartEngine,
    ResumeGids(Vec<String>),
    CheckMissingFiles,
    ReloadSchedules,
}
enum EngineEvent {
    Added { gid: String, name: String, url: String, dir: String, info_hash: Option<String>, advanced: TaskAdvancedOptions, from_browser: bool },
    Progress { gid: String, name: String, downloaded: u64, total: u64, speed: u64, upload_speed: u64, status: String, connections: u64, info_hash: Option<String> },
    TorrentAdded { gid: String, path: PathBuf },
    Removed(String),
    TaskDetails { gid: String, details: crate::task::TaskDetails },
    TaskDetailsFailed { gid: String },
    SelectFilesFailed { gid: String },
    EngineReady, SyncComplete, EngineStopped,
    Aria2Status { stage: String, message: String },
    Aria2Version { version: String },
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
[package] name = "remotrix" version = "0.1.2" edition = "2021" license = "MIT"
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
directories = "6"
rfd = "0.17"
image = { version = "0.25", default-features = false, features = ["png"] }
mundy = { version = "0.2", default-features = false, features = ["color-scheme", "accent-color", "async-io"] }
fluent-templates = "0.15.1"
futures = "0.3"
base64 = "0.23"
hex = "0.4"
num-traits = "0.2"
iced_aw = { version = "0.14", default-features = false, features = ["time_picker"] }
reqwest = { version = "0.13", default-features = false, features = ["rustls", "json"] }
sha2 = "0.11"
rusqlite = { version = "0.40", features = ["bundled", "fallible_uint"] }
chrono = { version = "0.4", default-features = false, features = ["clock"] }
open = "5"
libc = "0.2"
fontdb = "0.24"
system-fonts = "0.1"  # locale-aware font preset used to auto-pick the system UI sans family on first launch

[build-dependencies]
iced_lucide = "0.1"
```

## Build Process (build.rs)
- Build-time only generates the icon module (`iced_lucide::build`)
- **No network access** during build — offline `cargo build` always succeeds
- aria2-next binary is fetched at **runtime** by `aria2_fetcher::ensure_aria2_next()`:
  - First launch: downloads from GitHub Releases (`AnInsomniacy/aria2-next`) to `<data_dir>/aria2/`
  - Cached across runs with `.installed` version/sha256 tracking
  - Supports `ARIA2_BIN` env var to skip download entirely
- Update workflow: app layer fetches releases (`updater::fetch_releases_since`) → non-silent/dialog selects → `EngineCmd::DownloadAria2Update` → `aria2_fetcher::stage_update_from` writes `.pending-update` → next engine restart applies. App self-update: `app_updater::stage_app_update` → `.pending-update` → `apply_pending_app_update` swaps on relaunch.

## Release profile
- `[profile.release]` (Cargo.toml) is **Aggressive**: `lto="fat"`, `codegen-units=1`, `panic="abort"`,
  `strip="symbols"`, `debug=false`, `overflow-checks=true`.
- Tradeoffs: `panic="abort"` removes Rust panic **backtraces** (errors still surface via `EngineEvent`);
  `strip="symbols"` removes debug symbols; fat LTO increases build time for a smaller binary.