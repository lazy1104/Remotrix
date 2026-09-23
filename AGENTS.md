# Remotrix — AI Agent Context

## Project Overview
Rust-native desktop download manager inspired by Motrix.app. Built with `iced` GUI framework and **aria2-next sidecar** (via `aria2-ws` RPC client).

| Component | Choice | Rationale |
|---|---|---|---|
| GUI | `iced 0.14` (+tokio, advanced, image, canvas) | Pure Rust, widget-based, multi-theme support |
| Engine | `aria2-next` sidecar + `aria2-ws 0.5` | C++ aria2 fork, JSON-RPC over WebSocket; spawned as subprocess |
| Async | `tokio 1.x` (full) | Shared runtime for engine + UI |
| Persistence | `rusqlite 0.40` (bundled + fallible_uint) | Embedded SQLite for task metadata / progress |
| Themes | iced `Theme::custom` (built-in) + `dark-light 2.0` | Accent-color swatches; iced auto-generates light/dark palettes (primary + M3-style surface background) from the accent; system detection |
| i18n | `fluent-templates 0.15` | Fluent translations (zh/en) |
| File dialog | `rfd 0.17` | Native OS file picker |
| Config dirs | `directories 6` | XDG/user data paths |

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
dark-light = "2.0"
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

## Code Conventions
- **Module structure**: `src/` with flat top-level modules (`app.rs`, `config.rs`, `db.rs`, `engine.rs`, `extension_api.rs`, `aria2_fetcher.rs`, `updater.rs`, `message.rs`, `task.rs`, `i18n.rs`, `clipboard_watch.rs`, `logging.rs`, `scheduler.rs`, `torrent_meta.rs`, `trackers.rs`) + `ui/` subdirectory
- **UI pattern**: Each page is a `fn` returning `iced::Element<'_, Message, Theme>`; no widget OOP wrappers
- **Time pickers** (`Settings > Download > Speed Limits`): `iced_aw` clock component (`time_picker` feature) wrapped in `src/ui/components/time_picker.rs`; the wrapper re-seeds iced_aw state on the open transition via `tree.children[0].state` so reopening shows the committed value.
- **Theme**: single accent color → iced `Theme::custom` palette generation (`src/ui/theme.rs`), with the background derived as an M3-style surface from the accent hue (`surface_from_seed`); colors read from `iced::Theme::extended_palette()`, no hardcoded palette constants.
- **Naming**: `snake_case` for fns/vars, `PascalCase` for types/enums, `SCREAMING_SNAKE` for constants
- **Error handling**: Use `String` errors in engine layer, map to `EngineEvent::EngineStopped` for fatal
- **No comments** in source code unless explaining a non-obvious design decision
- **Imports**: Group as `std` → external crates → `crate::` (blank-line separated)
- **Path resolution**: All on-disk paths must go through the resolver functions in `src/config.rs` (`aria2_bin_dir()`, `session_dir()`, `db_path()`, `log_dir()`, `config_file_path()`). Modules must not call `directories::*` directly — the resolvers honour user path overrides from `Settings::paths` and the migration step in `main()`.

## Commit Messages (bilingual, simple)
- **Format**: `type(scope): 中文描述 (English summary)` — one concise line, both in the subject.
- The subject is the changelog source (`release-changelog-builder-action` reads commit messages), so it must be self-explanatory in Chinese for end users, with a short English parenthetical for machine/GitHub readability.
- **Types** (Conventional Commits): `feat`, `fix`, `perf`, `refactor`, `style`, `docs`, `build`, `ci`, `chore`, `test`, `revert`.
- **Scope**: the affected module, e.g. `ui`, `engine`, `notify`, `tray`, `update`, `settings`, `proxy`, `i18n`, `logging`, `packaging`.
- Keep the subject under ~72 chars. Body only when needed to explain "why" in either language.
- Examples:
  - `feat(notify): 添加 Windows Toast 通知支持 (add Windows toast notifications)`
  - `fix(engine): 为 RPC 调用添加超时机制防止阻塞 (add timeouts to RPC calls)`
  - `refactor(settings): 合并通知与确认设置分组 (merge notification & confirm settings)`
- A `.gitmessage` commit template is provided; enable it with `git config core.template .gitmessage`.

## Build / Check Commands
```bash
cargo build                    # debug build (no network; aria2-next fetched at runtime)
cargo build --release          # release build
cargo run --                   # run app
cargo clippy --workspace       # lint (no warnings allowed)
cargo fmt --check              # formatting check
cargo packager --release --config packager.toml --formats deb,appimage   # local Linux packaging
cargo test --test download_e2e -- --nocapture   # integration tests for the download pipeline
# Refresh ED2K default bootstrap: re-download assets/ed2k-bootstrap/{server.met,nodes.dat}
# (e.g. from https://upd.emule-security.org/) and rebuild — files go through
# `include_bytes!` at compile time (src/ed2k_bootstrap.rs).
```
The `download_e2e` binary needs a real `aria2-next` binary on disk (set
`ARIA2_BIN=/path/to/aria2-next` or have it on `$PATH`); without one, every
test logs `skip: ARIA2_BIN not set` and returns `Ok(())` so the suite stays
green on machines without aria2 (CI, fresh dev boxes). Tests are serialised
via `#[serial(aria2)]` and require **Linux or macOS** — Windows isn't
supported because the `directories` crate reads `SHGetKnownFolderPath`
there, ignoring the `$HOME` redirect the harness uses for temp-dir
isolation. Some tests are `#[ignore]`-d as follow-ups (BT seeder
plumbing, hang-endpoint timeout, session-replay); see `tests/download_e2e.rs`
for the per-test reasoning.
Run `/check-docs` (Kilo command) to audit README.md and this file against the codebase.

## Release profile
- `[profile.release]` (Cargo.toml) is **Aggressive**: `lto="fat"`, `codegen-units=1`, `panic="abort"`,
  `strip="symbols"`, `debug=false`, `overflow-checks=true`.
- Tradeoffs: `panic="abort"` removes Rust panic **backtraces** (errors still surface via `EngineEvent`);
  `strip="symbols"` removes debug symbols; fat LTO increases build time for a smaller binary.

## Packaging / CI
- `packager.toml` configures **cargo-packager** (schema follows cargo-packager 0.11.x; `Packager.toml` is
  the default filename, we use `packager.toml` + `--config` explicitly).
- `version` in `packager.toml` **must stay in sync** with `Cargo.toml` (currently `0.1.2`).
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

## Versioning Policy

Remotrix follows SemVer 2.0.0 in spirit, but is currently in the `0.x` phase (SemVer §4: any `MINOR` bump
may include breaking changes until `1.0.0`).

**Segment semantics**
- `MAJOR` — currently `0`; the first API/UX-stable release will be `1.0.0`. Until then, the project is
  "initial development" regardless of PATCH tag.
- `MINOR` — **new user-visible functionality** since the last release (features, panels, settings pages,
  new download sources, reworked flows). Also the right place when scope grew beyond what a `-beta.N`
  cycle was meant to cover.
- `PATCH` — **user-invisible or near-invisible fixes** only: bug fixes, regression repairs, copy/string
  corrections, dependency bumps that do not change behavior. Not for new features.
- `-prerelease` (`-alpha.N`, `-beta.N`, `-rc.N`) — the **same target version** at a later maturity
  stage. `0.2.0-beta.2` is NOT a new version on top of `0.2.0-beta.1`; both are the same `0.2.0`, just
  later snapshots. Always compare against the base (`0.2.0`), never against the previous prerelease.

**When to use a `-beta.N` prerelease tag**
- The release is feature-complete for its target `MINOR.PATCH` but not yet considered stable enough for
  the default auto-update channel.
- Use `beta.1` for the first public test; `beta.2`/`beta.3` for follow-up fixes that stay on the same
  target. Drop the prerelease tag when cutting the final release (e.g., `0.2.0-beta.3` → `0.2.0`).
- Prerelease tags are the **only** mechanism (besides the `beta_channel` setting) to opt a release out
  of GitHub's "Latest" badge and out of the default in-app auto-update channel
  (`src/config.rs` → `UpdateSettings::beta_channel`, default `false`; consumed in `src/app.rs:2215`).
  `release.yml` auto-sets `prerelease: ${{ contains(github.ref_name, '-') }}` — no `-` means Latest.

**Decision rules for the next version (apply in order)**
1. **New user-visible feature** (or a reworked/expanded scope that users will notice)?
   → bump **MINOR**: `0.2.x` → `0.3.0`.
   - Want the cycle gated by the beta channel first? Land it as `0.3.0-beta.1`, iterate `-beta.2`/…,
     then promote to `0.3.0` when ready.
2. **Only bug fixes / tightening / dep bumps that do not change UX**?
   → bump **PATCH**: `0.2.0` → `0.2.1`.
   - Same-cycle fix while still in a beta cycle? Bump the **prerelease number**
     (`0.2.0-beta.1` → `0.2.0-beta.2`), **not** the PATCH.
3. **Promoting a beta cycle to stable**?
   → drop the prerelease tag: `0.2.0-beta.N` → `0.2.0`. Do **not** introduce a new `-rc.1` in between
     unless the project explicitly adopts an RC gate.

**Anti-patterns (do not do)**
- Adding a new feature and bumping only PATCH (`0.2.0` → `0.2.1`). PATCH is for user-invisible fixes.
- Treating `0.2.0-beta.2` as "a new version on top of `0.2.0-beta.1`". Same base, same target.
- Mixing a new feature and a bugfix into one PATCH bump. Split into MINOR + PATCH or land in separate
  releases.
- Tagging `v0.2.0` while still iterating and wanting it gated: without `-` in the tag, GH marks it
  Latest and the in-app updater (with `beta_channel = false`) will offer it to all users.
- Skipping directly from a `-beta` to the next `MINOR` (e.g., landing new features on
  `0.2.0-beta.3` and tagging `v0.3.0`): cut a fresh `0.3.0-beta.1` so the beta channel still gates the
  new feature work.

## Version Upgrade (releasing a new version)
- **Keep `Cargo.lock` in sync with `Cargo.toml`.** Bumping `[package] version` in `Cargo.toml` does NOT
  auto-update the root package's own `version` field inside `Cargo.lock` (a non-registry path dep). If you
  commit only `Cargo.toml`, CI's `cargo build --profile dist --locked` fails with:
  `error: cannot update the lock file ... because --locked was passed`. Symptom of a stale lock: `Cargo.lock`
  root `remotrix` version differs from `Cargo.toml`.
- **Recommended order when bumping a version (e.g. 0.1.x → 0.1.y):**
  1. Bump `version` in `Cargo.toml` (prefer `cargo set-version --workspace` from `cargo-edit`; it keeps
     `Cargo.toml`/`Cargo.lock` in sync and tags nothing).
  2. Run `cargo build` (or `cargo update -p remotrix`) once so `Cargo.lock`'s root `remotrix` version is
     regenerated to match.
  3. Verify with `git diff` that both `Cargo.toml` and `Cargo.lock` changed, and confirm
     `cargo metadata --format-version 1` succeeds **without modifying** `Cargo.lock` (run `git status` after).
  4. Bump `version` in `packager.toml` to the same value (must stay in sync; see above).
  5. Commit, then push a tag `v<version>`. `release.yml` triggers on tag push and runs `--locked`, so the
     lock **must** already be in sync — do not create the tag before steps 1–4.
- **A tag must point to a commit whose lock is in sync.** If a release fails, do NOT just re-tag the same
  commit: recreate the tag on the commit that actually fixed the lock, or cut a fresh `v0.1.y`. Retagging a
  stale commit (e.g. `git tag -f v0.1.2 <old-stale-commit>`) will re-fail the same way.

## Build Process (build.rs)
- Build-time only generates the icon module (`iced_lucide::build`)
- **No network access** during build — offline `cargo build` always succeeds
- aria2-next binary is fetched at **runtime** by `aria2_fetcher::ensure_aria2_next()`:
  - First launch: downloads from GitHub Releases (`AnInsomniacy/aria2-next`) to `<data_dir>/aria2/`
  - Cached across runs with `.installed` version/sha256 tracking
  - Supports `ARIA2_BIN` env var to skip download entirely
- Update workflow: app layer fetches releases (`updater::fetch_releases_since`) → non-silent/dialog selects → `EngineCmd::DownloadAria2Update` → `aria2_fetcher::stage_update_from` writes `.pending-update` → next engine restart applies. App self-update: `app_updater::stage_app_update` → `.pending-update` → `apply_pending_app_update` swaps on relaunch.

## Risks to Watch
- `aria2-next` GitHub Releases may be temporarily unavailable → `ensure_aria2_next()` error at runtime with clear message; `ARIA2_BIN` env var fallback or manual binary placement
- Large task lists may lag iced → use `scrollable` + cap visible items
- No system tray support in iced → defer or use `tray-icon` crate separately
- `Secret` passed as CLI argument visible in `ps` on debug builds — acceptable (random per-session, local only)
- `tests/download_e2e.rs` runs against a live aria2-next sidecar — release validation requires the developer to run the suite locally with `ARIA2_BIN` set; CI is intentionally NOT wired up (the binary is an external dep).
- `default_app_data_dir()` (config.rs:1088) does NOT honour `XDG_DATA_HOME` — only `data_home()` does. The integration test harness works around this by redirecting `$HOME`, but prod callers that set `XDG_DATA_HOME` while expecting ProjectDirs-style paths will silently land in the default location. Track as a real-config bug.
- `assets/ed2k-bootstrap/{server.met,nodes.dat}` are compile-time embedded via `include_bytes!` (`src/ed2k_bootstrap.rs`). On first engine start, `ensure_cache()` copies them into `<data_dir>/ed2k-bootstrap/` only if missing (no overwrite). When `ed2k_server_list` / `ed2k_node_list` are blank in Settings → ED2K, `inject_managed_bootstrap_args` (`engine.rs` spawn path) appends `--ed2k-server-list` / `--ed2k-node-list` pointing at the cache, so aria2-next can bootstrap the ED2K network out-of-the-box. User-set paths always win. `sync_once` overwrites the cache on success and takes effect on the next engine restart; it is intentionally not wired to auto-restart. Default sync URLs are `https://upd.emule-security.org/{server.met,nodes.dat}` (`config.rs::default_ed2k_*_url`). To restore bundled defaults after a sync, delete `<data_dir>/ed2k-bootstrap/{server.met,nodes.dat}`.
- Legacy `http://www.gruk.org/{server.met,nodes.dat}` URLs persisted in pre-emule-security installs cannot be overwritten by `#[serde(default)]` (the field is present, so the default does not fire). `config.rs::load()` runs `fix_dead_ed2k_bootstrap_urls(&mut settings)` after the `theme_color` migration, which rewrites only those two exact strings to the current emule-security mirrors and re-saves the file. User-custom URLs are preserved. Search (`ed2k_search_start`) and `ed2k://|file|` downloads (`add_download_internal`) inject the cached `ed2k-server-list` / `ed2k-node-list` via `inject_managed_bootstrap_options` whenever the corresponding Settings → ED2K fields are blank, so the network boots even before a manual sync completes.
- `aria2.ed2kSearch` RPC option contract is build-dependent: older aria2-next silently ignores unknown keys, newer builds throw `Unknown option: <key>` from `gatherRequestOption`. The motrix-next `fileType` + `minSourceCount` shape is therefore not portable. We temporarily hide the ED2K Search UI section (`src/ui/settings_page.rs:1789-1792`) — engine plumbing (`EngineCmd::Ed2kSearchStart/Cleanup`, `EngineEvent::Ed2kSearchStarted/Results/Failed`, `ed2k_search_start/cleanup`) is intentionally retained for a one-line revert once the contract stabilizes. ED2K downloads (`aria2.addUri` with `ed2k://|file|` links) and Settings → ED2K panel are unaffected and continue to work. Search timeout in Settings → ED2K → "Search timeout" is currently UI-inert (no surface to use it from); the engine-side poll-deadline plumbing from `Ed2kSearchStart::timeout_secs` is also retained.
- `sync_once` updates the bootstrap cache on success, but aria2's in-memory ED2K state is bootstrapped at engine startup with whatever was on disk at that moment. **Changes only take effect after the engine is restarted.** This is intentionally not auto-wired (see Round-1 note); document the restart step in user-facing help. The Settings → ED2K hint now mentions this.
- Clipboard "Smart" webpage filter (`src/clipboard_watch.rs::probe_url` + `apply_webpage_filter`) issues a single HEAD/Range request per **extension-less** http/https URL the user copies, using the configured proxy + UA with a 5 s per-leg timeout (HEAD and the ranged-GET fallback are independent; worst-case ~10 s per URL). Network failures collapse to `Ambiguous` (we keep the URL and offer it — better than silently dropping a real download). Switching focus to the window no longer re-triggers the probe because `last_clipboard_hash` is now updated on every clipboard read regardless of filter outcome (`src/update/window.rs`). Existing installs upgrade silently into `Smart`; users who don't want probing can switch to `Off`/`Static` in Settings → Clipboard. Logging: every invocation emits an `info!("clipboard: filter done", mode, kept, dropped, elapsed_ms)` summary; per-URL probes are at `debug!`; timeouts at `warn!`. Raise verbosity with `RUST_LOG=remotrix=debug` (or `=trace`) when troubleshooting "why was my clipboard dialog slow / empty".
- **Auto-retrying error tasks across restarts.** aria2's `--input-file`/`--save-session` (`src/engine.rs:426-429`) restore every saved entry on startup and rewrite it every 5 s. When a task transitions to `Error` *without* being paused, aria2's session write omits `pause=true`, so the next startup re-adds the same dead task → it errors again → written again. The per-session `max-tries` / `retry-wait` settings (`src/ui/settings_page.rs:995-1009`) do **not** cap cross-restart retries — each engine lifetime gets its own 5-tries budget. To stop the loop, `prune_error_tasks_in_aria2` (`src/engine.rs:324`, called from `Sidecar::spawn` after the WebSocket `Client` is connected and before `Ok(Sidecar { .. })` is returned) calls `aria2.tell_stopped(-1, 1000)` and runs `force_remove` + `remove_download_result` on every entry whose status is `error` — using aria2 itself as the source of truth (the SQLite row can lag by up to ~1 s, or stay stuck at `waiting` if the previous process was killed mid-flush, so filtering by DB was unreliable). After removal the GID is gone from aria2's session save, so subsequent `--input-file` replays do not resurrect it. The DB row is intentionally kept (the UI still shows the task as `Error`, so the user can right-click → Delete or hit "重新下载", which issues a fresh `aria2.addUri` and a brand-new GID). Any failure in the prune path (RPC error, individual `force_remove` / `remove_download_result` failure) logs `warn!` and is a no-op, so a flaky WebSocket or unresponsive RPC cannot block engine spawn.
- **Top border as background progress.** The non-maximized outer frame swaps the static 1px hairline for a 2px `iced::widget::canvas` gradient bar whenever any of `engine_ui.update_check_in_flight`, `engine_ui.aria2_downloading`, `app_update_in_flight`, `restart.engine_restart_in_progress`, `settings_ui.syncing_trackers` (BT tracker sync), `settings_ui.syncing_bootstrap` (ED2K bootstrap sync), or an `aria2_status` stage other than `ready` is true (`is_background_busy` in `src/app.rs`; renderer in `src/ui/border_bar.rs`). A terminal engine failure (`aria2_fetch_error.is_some()`) **does not** drive the bar — the bar animates only while work is genuinely in progress; terminal degraded state surfaces via the sticky error toast and the optional system notification instead. The bar uses `theme::primary` and `theme::border_color` so it tracks theme changes for free. A 60 fps `BorderAnimTick` subscription (`src/app.rs`) is mounted **only while busy** — once all signals clear it drops to `Subscription::none()`, so CPU returns to idle immediately. Maximized windows still draw no border at all. Phase is derived from `crate::ui::animation::cycle` with `PERIOD = 1500 ms`; the gradient itself is drawn at 2× window width and shifted left by `phase * width`, giving a seamless left-to-right loop.
- **Known limitation — pending-update promote can delete the working binary.** `aria2_fetcher::apply_pending_update` (`src/updaters/aria2_fetcher.rs`) deletes the previously installed aria2-next binary as part of promoting a `.pending-update` whose `sha256` matches the staged file. SHA verification protects against corruption, not against a valid file that is not a working aria2-next — such a staged binary will still pass and nuke the working install. A robustness hardening (pre-promote `spawn` smoke test, or backup-and-rollback) is a known follow-up but not yet implemented. Test fixtures for pending-update flows must use real, executable binaries (or sandbox paths that don't shadow an existing install).
- **Toolbar bulk actions are filter-scoped.** The `All` / `Downloading` toolbar's `StartAll` / `PauseAll` / `DeleteAll` / `RemoveAllRecords` handlers in `src/update/task.rs` no longer call aria2's `*All` RPC variants (which are truly global and would nuke completed/failed tasks). They iterate `state.tasks` for `Active | Waiting | Paused` gids and send per-gid `EngineCmd::Pause / Resume / Remove { delete_files }`, then call `clear_specific_local` (`src/app.rs`) to remove only those gids from the local DB. The `Completed` / `Failed` filter tabs only render Refresh + Sort + Clear Records — `StartAll` / `PauseAll` / `DeleteAll` / `RemoveAllRecords` still short-circuit as a defence-in-depth against message injection (extension API, clipboard, etc.). `ClearCompleted` (`Tr::ClearList` / "清空记录") picks the predicate by filter: `Failed` → only `Error`, others → `Completed | Removed`. If the global aria2 `PauseAll` / `ResumeAll` / `RemoveAll` RPC ever needs to be wired up again, do not bypass `task_filter` here.
- **`silent_update_scope` replaces `aria2_silent_update`.** Settings → Auto Update's old aria2-only toggle is now a 4-option dropdown (Off / Engine / App / Both) driven by `SilentUpdateScope` (`src/config/settings.rs`). `Default::Both` covers fresh installs that have no JSON migration path; `config::load()` runs `migrate_legacy_silent_update_scope` once on first read to map the old `aria2_silent_update: bool` field onto `silent_update_scope` (`true → "engine"`, `false → "off"`) and drops the legacy key from disk so subsequent starts skip the migration. The `UpdateResult` handler in `src/update/settings/mod.rs` dispatches `silent_applied` by `offer.component`: Aria2 keeps the silent `send_download_aria2_update(..., true)` flow; App routes through the extracted `kick_off_app_update(state, &offer, /*show_downloading_toast=*/ false)` so silent app downloads skip the "正在下载更新" toast/notify and rely on the existing `AppUpdateReady` sticky toast with the "更新" action button — application (AppImage relaunch, deb open, Windows installer launch) **always** waits for the user to click, matching the aria2 "no auto-restart" semantics.
- **Update-check diagnostics.** The in-app "立即检查 / Check Now" path now emits `tracing` lines to the daily `remotrix.<date>.log`: `update check started` (scope/silent/beta/current versions, `src/app.rs check_updates`), per-component `release fetched` / `release fetch failed` with the fetched + current version and `component`, `update check complete` (offers/silent/errors/pending, `src/update/settings/mod.rs UpdateResult`), and `update fetch: releases/latest|releases list` + `release matched` (tag/version/asset, `src/updaters/updater.rs`), plus `is_background_busy: true` / `is_background_busy: false` which names which term (e.g. `aria2_downloading`) keeps the top border bar lit. The busy log fires only on **transition** — once per change of the active set, throttled by a process-local `OnceLock<Mutex<...>>` (`src/app.rs`) so the previous 60fps spam while a single download was active collapses to one line at start and one at end. Logging is gated by the app log level (Settings → Log, default `warn`); set it to `debug` or `trace` to see these lines and to confirm what a manual check actually fetched. When a release's embedded `assets` array comes back empty (GitHub snapshot desync, e.g. assets uploaded after publish) `release_from_json` falls back to that release's `assets_url` (`/releases/{id}/assets`) for the authoritative list and logs `update fetch: inline assets empty; fell back to assets_url` at `warn` (with tag/version/asset_count), so silent missed updates no longer look like "已是最新" and the fallback frequency is auditable. The `assets_url` call is one extra GET per affected release (zero in steady state). All lines are instrumentation-only — no behavior change beyond the fallback itself.
- **No bundled CJK font.** The binary no longer embeds a CJK fallback font (`fonts/HarmonyOS_Sans_SC_Regular.ttf` removed, `src/main.rs` no longer `include_bytes!`s it). When `Settings.font_family` is empty (the picker option 「系统默认」, or first launch with no config) `src/main.rs` resolves `effective_font_family` transiently per boot via `src/ui/system_default_font::query()`, which calls the platform-native API: Linux `fc-match -f "%{family}\n" sans-serif`, macOS `/usr/bin/defaults read -g AppleSystemUIFont`, Windows `SystemParametersInfoW(SPI_GETNONCLIENTMETRICS).lfMessageFont` (via the existing `windows` crate with the `Win32_UI_WindowsAndMessaging` feature). The resolved name is **not** persisted to `settings.json`, so OS font changes (Linux fontconfig user prefs, GNOME Tweaks, Windows control panel, macOS System Settings) take effect on next launch without an app upgrade. `config::migrate_font_family` (`src/config/migrations.rs`) is only responsible for the one-time legacy `"HarmonyOS Sans SC"` → `""` rewrite so upgrade users land on the OS-following branch. Users who picked a specific family in the picker stay locked to that name until they re-select 「系统默认」. Linux and macOS `native_query` falls back to `font_autopick::pick_default_family` (`system_fonts::find_for_system_locale`, locale-aware fontdb enumeration) when the OS-native path fails; Windows has no fallback because `SPI_GETNONCLIENTMETRICS` is the source of truth. Linux on minimal containers (e.g. `scmilinux`, Alpine, `debian:slim`) may not ship `fc-match`; the fallback log line `fc-match not found on PATH; falling back to font_autopick` lets users trace it. On a system without any installed sans-serif the chosen family may not cover CJK glyphs and tasks render as `.notdef` (tofu); users can install `fonts-noto-cjk` or pick a CJK family in Settings → Appearance → Font. Verify the binary shrank with `strings target/release/remotrix | grep -c HarmonyOSSansSC` — should be 0 (was nonzero before this change).

## Release notes
- Release notes are **not** auto-generated. They come from the hand-maintained `CHANGELOG.md` at the repo root (Keep a Changelog 1.1.0 format; top section is always `## [Unreleased]` with `### Added / ### Changed / ### Fixed / ### Removed` sub-headings).
- `.github/workflows/release.yml` only uploads the existing file as an artifact and passes it to `softprops/action-gh-release@v2` as the release body. The auto-generated config (`.github/changelog-config.json`) has been removed.
- **Definition of done for any user-visible change** (feat / fix / UI tweak / behavior change): before committing, the agent responsible for the change must append a bullet to the matching sub-section under `## [Unreleased]` in `CHANGELOG.md`. Use user-facing language (no `feat(ui):` style prefixes), one line per change; if a change spans multiple categories, write one line under each. Pure internal refactors, chores, or dependency bumps with no behavior change may be skipped.
- A change that *only* updates `CHANGELOG.md` itself should not be made; the file is committed alongside the code change it describes. A stray `docs(release):` commit re-ordering or rewording older entries is fine and does not enter the current release body.

## Repo conventions — AGENTS.md sync
Any PR that introduces a new external dependency, a new env var, a new manual run step, a new failure mode, or a new operator-facing gotcha **must** update `AGENTS.md` in the same PR — never "I'll add docs later". Code-only PRs that need a doc tweak but skip it are rejected at review. The `Build / Check Commands` and `Risks to Watch` sections are the two places that catch the most landmines; new env vars also need a one-line mention under whichever section they apply to.
