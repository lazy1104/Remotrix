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
- Clipboard "Smart" webpage filter (`src/clipboard_watch.rs::probe_url` + `apply_webpage_filter`) issues a single HEAD/Range request per **extension-less** http/https URL the user copies, using the configured proxy + UA with a 5 s per-leg timeout (HEAD and the ranged-GET fallback are independent; worst-case ~10 s per URL). Network failures collapse to `Ambiguous` (we keep the URL and offer it — better than silently dropping a real download). Switching focus to the window no longer re-triggers the probe because `last_clipboard_hash` is now updated on every clipboard read regardless of filter outcome (`src/update/window.rs`). Existing installs upgrade silently into `Smart`; users who don't want probing can switch to `Off`/`Static` in Settings → Clipboard. Logging: every invocation emits an `info!("clipboard: filter done", mode, kept, dropped, elapsed_ms)` summary; per-URL probes are at `debug!`; timeouts at `warn!`. Raise verbosity with `RUST_LOG=remotrix=debug` (or `=trace`) when troubleshooting "why was my clipboard dialog slow / empty".

## Repo conventions — AGENTS.md sync
Any PR that introduces a new external dependency, a new env var, a new manual run step, a new failure mode, or a new operator-facing gotcha **must** update `AGENTS.md` in the same PR — never "I'll add docs later". Code-only PRs that need a doc tweak but skip it are rejected at review. The `Build / Check Commands` and `Risks to Watch` sections are the two places that catch the most landmines; new env vars also need a one-line mention under whichever section they apply to.
