# Remotrix — AI Agent Context

## Project Overview
Rust-native desktop download manager inspired by Motrix.app. Built with `iced` GUI framework and `aria2-core` download engine.

| Component | Choice | Rationale |
|---|---|---|
| GUI | `iced 0.13` (+tokio feature) | Pure Rust, widget-based, dark theme support |
| Engine | `aria2-core 0.2.3` | Rust rewrite of aria2, tokio-based |
| Async | `tokio 1.x` full features | Shared with aria2-core runtime |
| File dialog | `rfd 0.15` | Native OS file picker |
| Config dirs | `directories 5` | XDG/user data paths |

## Architecture Decision: Dual Event Loop
- **iced UI loop** runs on the main thread
- **tokio runtime** runs `aria2-core::DownloadEngine` on a background thread
- Communication via `tokio::sync::mpsc` channels (unbounded for simplicity)
- **GUI → Engine**: `iced::Command::run` sends `EngineCmd` via `mpsc::Sender`
- **Engine → GUI**: `iced::Subscription` polls `mpsc::Receiver` for `EngineEvent`

```rust
// --- Channel Protocol (must match between engine.rs and message.rs) ---
enum EngineCmd {
    AddDownload { urls: Vec<String>, options: DownloadOptions, save_dir: PathBuf },
    Pause(String), Resume(String), Remove(String),
    SetOption(String, String), Shutdown,
}
enum EngineEvent {
    Progress { gid: String, downloaded: u64, total: u64, speed: u64, status: String },
    Complete(String), Error(String, String), Added(String), Removed(String),
    EngineReady, EngineStopped,
}
```

## aria2-core API Reference
- `aria2_core::config::ConfigManager` — global settings (dir, split, speed limits)
- `aria2_core::request::request_group_man::RequestGroupMan` — task manager (add/pause/resume/remove tasks)
- `aria2_core::request::request_group::DownloadOptions` — per-task options (split, max connections, etc.)
- `aria2_core::request::request_group::GID` — task identifier (use `.value()` for string)
- `aria2_core::config::OptionValue` — enum: `Str(String)` or `Int(i64)`

Quick start pattern:
```rust
let mut config = ConfigManager::new();
config.set_global_option("dir", OptionValue::Str("./downloads".into())).await?;
let man = RequestGroupMan::new();
let opts = DownloadOptions { split: Some(4), ..Default::default() };
let gid = man.add_group(vec!["http://...".into()], opts).await?;
```

## Cargo.toml Dependencies
```toml
[package] name = "remotrix" version = "0.1.0" edition = "2021"
[dependencies]
aria2-core = "0.2.3"
iced = { version = "0.13", features = ["tokio"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
directories = "5"
rfd = "0.15"
```

## Code Conventions
- **Module structure**: `src/` with flat top-level modules (`app.rs`, `engine.rs`, `task.rs`, `message.rs`, `config.rs`) + `ui/` subdirectory
- **UI pattern**: Each page is a `fn` returning `iced::Element<'_, Message, Theme>`; no widget OOP wrappers
- **Theme**: Custom `Theme` struct (not iced built-in themes) with Motrix-dark palette (defined in `ui/theme.rs`)
- **Naming**: `snake_case` for fns/vars, `PascalCase` for types/enums, `SCREAMING_SNAKE` for constants
- **Error handling**: Use `anyhow::Result` in engine layer, map to `Message::Error(String)` for UI
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

## Project Status & Next Steps
- [Phase 1] Workspace scaffolding + `Cargo.toml` → create `src/main.rs`, `src/engine.rs`, `src/app.rs`
- [Phase 1] Implement `EngineBridge` with tokio runtime + mpsc channels
- [Phase 2] Build iced UI: `theme.rs`, `sidebar.rs`, `task_list.rs`, `add_dialog.rs`, `settings_page.rs`
- [Phase 2] Wire `app.rs` with `update()` → `view()` → `subscription()`
- [Phase 3] Connect UI ↔ Engine via Command/Subscription
- [Phase 4] Polish: icon, i18n, file dialogs

## Build / Check Commands
```bash
cargo build                    # debug build
cargo build --release          # release build
cargo run --                   # run app
cargo test --workspace         # all tests
cargo clippy --workspace       # lint (no warnings allowed)
cargo fmt --check              # formatting check
```

## Logo (Pending Designer)
Style: Motrix diamond + Rust gear + iced rounded. Element: letter "R". Colors: `#E05A33` (Rust orange-red) + `#4A90D9` (Motrix blue). Output: 1024×1024 PNG, 256×256 ICO, SVG source. Place in `assets/icon.png`.

## Risks to Watch
- `iced` "tokio" feature may have rough edges → fallback: wrap `Runtime::block_on` in `Command::perform`
- `aria2-core` is pre-1.0 (v0.2.3) → pin exact version, isolate behind `EngineBridge` trait
- Large task lists may lag iced → use `scrollable` + cap visible items
- No system tray support in iced → defer or use `tray-icon` crate separately
- `rsa` vendor (`vendor/rsa`) remains necessary: aria2-core → aria2-protocol → russh 0.59 locks `rsa = "0.10.0-rc.12"` (pre-release exact match, Cargo won't auto-upgrade). Upstream `rc.18` has the fix natively, but russh 0.59 pins `rc.12`. One-line patch in `vendor/rsa/src/encoding.rs:230` adapts `pkcs8::Error::KeyMalformed` → `KeyMalformed(KeyError::Invalid)`. Removing vendor would require patching russh to 0.62, which is disproportionate. Keep vendor as-is; reassess when aria2-core upgrades to more recent russh.
