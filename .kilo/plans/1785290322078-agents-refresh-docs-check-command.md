# Plan: Refresh stale AGENTS.md + add a `/check-docs` report-only command

## Goal & Scope

Two deliverables, both documentation/config only (no `src/` edits):

1. **Refresh `AGENTS.md`** - fix every section that drifted from the actual code (enums, deps,
   theming, module list, build comment). Targeted edits; preserve the sections that are still
   accurate (aria2-ws API ref, architecture narrative, build.rs process, risks).
2. **Add `.kilo/command/check-docs.md`** - a Kilo slash command that starts an AI conversation to
   audit `README.md` + `AGENTS.md` against the codebase and **report-only** (no edits) what is stale.

Confirmed with user: command is **report-only** (reports discrepancies + proposed fixes; does not
edit files unless the user then asks).

## Repo state constraint

`src/app.rs`, `src/engine.rs`, `src/main.rs` have uncommitted in-flight edits (from a prior session).
Touch **only** `AGENTS.md` and the new `.kilo/command/check-docs.md`. Do not modify `src/*`,
`README.md` (already refreshed), `Cargo.toml`, or `AGENTS.md`'s still-accurate sections.

---

## Part A - AGENTS.md refresh (targeted edits)

All facts below were verified against the real source on disk. Apply each as an `edit` (match the
exact `oldString`, replace with `newString`). Line numbers refer to the current `AGENTS.md`.

### A1. Project Overview table (lines 6-12) - expand with new components

Replace the 5-row table with:

```
| Component | Choice | Rationale |
|---|---|---|
| GUI | `iced 0.14` (+tokio, advanced, image, canvas) | Pure Rust, widget-based, multi-theme support |
| Engine | `aria2-next` sidecar + `aria2-ws 0.5` | C++ aria2 fork, JSON-RPC over WebSocket; spawned as subprocess |
| Async | `tokio 1.x` (full) | Shared runtime for engine + UI |
| Persistence | `rusqlite 0.32` (bundled) | Embedded SQLite for task metadata / progress |
| Themes | `opaline 0.4` (builtin-themes, iced) + `dark-light 1.1` | Multiple light/dark palettes; system detection |
| i18n | `fluent-templates 0.14` | Fluent translations (zh/en) |
| File dialog | `rfd 0.15` | Native OS file picker |
| Config dirs | `directories 5` | XDG/user data paths |
```

### A2. EngineCmd enum (lines 28-37) - match `src/engine.rs:15-41`

Actual variants: `SetSpeedLimit` is GONE; `ApplyAria2Options`, `AddTorrent`, `FetchTaskDetails` added.

```rust
enum EngineCmd {
    AddDownload { urls: Vec<String>, save_dir: PathBuf, split: u16 },
    AddTorrent { path: PathBuf, save_dir: PathBuf, split: u16 },
    Pause(String), Resume(String), Remove(String),
    PauseAll, ResumeAll, RemoveAll, Snapshot,
    ApplyAria2Options { options: TaskOptions },
    FetchTaskDetails(String),
    Shutdown,
    CheckAria2Update,
    RetryAria2Fetch,
    RestartEngine,
}
```

### A3. EngineEvent enum (lines 38-50) - match `src/engine.rs:44-95`

`Added` gained `url`/`dir`; `Progress` gained `connections`; `TaskDetails`/`TaskDetailsFailed` added.

```rust
enum EngineEvent {
    Added { gid: String, name: String, url: String, dir: String },
    Progress { gid: String, downloaded: u64, total: u64, speed: u64, status: String, connections: u64 },
    Removed(String),
    TaskDetails { gid: String, details: crate::task::TaskDetails },
    TaskDetailsFailed { gid: String },
    EngineReady, EngineStopped,
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

### A4. Cargo.toml Dependencies block (lines 74-94) - match actual `Cargo.toml`

Missing: `anyhow`, `tracing-appender`, `image`, `dark-light`, `fluent-templates`, `hex`, `num-traits`,
`iced_aw`, `opaline`, `rusqlite`, `chrono`, `open`. `iced` missing `canvas`; `reqwest` missing `json`.
Also add `license` to the `[package]` line.

```toml
[package] name = "remotrix" version = "0.1.0" edition = "2021" license = "GPL-2.0-or-later"
[dependencies]
aria2-ws = "0.5"
iced = { version = "0.14", features = ["tokio", "advanced", "image", "canvas"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
directories = "5"
rfd = "0.15"
image = "0.24"
dark-light = "1.1"
fluent-templates = "0.14.0"
futures = "0.3"
base64 = "0.22"
hex = "0.4"
num-traits = "0.2"
iced_aw = { version = "0.14", default-features = false, features = ["number_input", "drop_down"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
sha2 = "0.10"
opaline = { version = "0.4", default-features = false, features = ["builtin-themes", "iced"] }
rusqlite = { version = "0.32", features = ["bundled"] }
chrono = { version = "0.4", default-features = false, features = ["clock"] }
open = "5"

[build-dependencies]
iced_lucide = "0.1"
```

### A5. Code Conventions (lines 96-103) - fix module list + theme line

- **Module structure** bullet: replace the parenthetical module list with the full current set:
  `app.rs`, `config.rs`, `db.rs`, `engine.rs`, `aria2_fetcher.rs`, `updater.rs`, `message.rs`,
  `task.rs`, `i18n.rs` (+ `ui/` subdirectory).
- **Theme** bullet: change from "Custom `Theme` struct ... Motrix-dark palette" to:
  "`opaline` builtin themes loaded via the iced adapter (`src/ui/theme.rs`); colors read from
  `iced::Theme::extended_palette()`, no hardcoded palette constants."

### A6. "Theme Constants (Motrix Dark)" block (lines 105-118) - REMOVE entirely

No such `const` block exists in `src/ui/theme.rs` anymore. Delete the whole section (heading +
fenced rust block). Do not replace with anything.

### A7. Build / Check Commands (lines 120-127) - fix misleading comment + add docs-check note

- `cargo build` comment currently says "(downloads aria2-next binary)" - FALSE. Build needs no
  network; aria2-next is fetched at runtime. Change to `# debug build (no network; aria2-next fetched at runtime)`.
- Append one line after the fenced block:
  `Run /check-docs (Kilo command) to audit README.md and this file against the codebase.`

### A8. aria2-ws API Reference (lines 53-72) - PRESERVE (still accurate)

Verified against `src/engine.rs`: `add_uri(urls, Some(options), None, None)`, `tell_status`,
`tell_active`, `tell_waiting(-1, 1000)`, `tell_stopped(-1, 1000)`, `change_global_option`,
`subscribe_notifications`, `unpause`, `force_remove` all match. Leave as-is. (Optional: add
`unpause_all()`/`pause_all()` to the method list - low priority.)

---

## Part B - New file `.kilo/command/check-docs.md` (report-only)

Create `.kilo/command/` dir (does not exist yet) and write this exact content:

```markdown
---
description: Audit README.md & AGENTS.md for staleness vs the codebase (report-only)
agent: code
---
Audit the project documentation for drift against the actual source code. This is a **report-only**
check: do NOT edit any files. Read the docs and the sources, then output a findings report. The user
decides what to fix afterwards.

## Docs to audit
- `README.md`
- `AGENTS.md`

## Sources of truth (read and compare against)
- `Cargo.toml` - crate names, versions, feature flags, license
- `src/engine.rs` - `EngineCmd` / `EngineEvent` enum variants + fields
- `src/message.rs` - `Page`, `TaskFilter`, `SettingsCategory`, `SortField`, `DetailsTab`, `SettingKey`
- `src/ui/theme.rs` - theming approach (opaline + extended_palette, NOT a custom palette)
- `src/ui/mod.rs` + `src/ui/*.rs` - module / file layout
- `src/aria2_fetcher.rs`, `src/updater.rs` - aria2-next lifecycle (cache, sha256, .installed, .pending-update, ARIA2_BIN)
- `src/db.rs`, `src/config.rs` - persistence + config/data dir paths
- `src/main.rs`, `build.rs` - fonts, logging (tracing-appender), build-time behavior (no network)

## What to verify
1. **Dependencies** - every crate + version + feature in `Cargo.toml` is reflected; no stale/missing entries.
2. **Channel protocol** - `EngineCmd`/`EngineEvent` variants and fields in `AGENTS.md` match `src/engine.rs` exactly.
3. **Code layout** - every file under `src/` is listed with an accurate one-line description; no phantom/missing files.
4. **Theming** - docs say `opaline` builtin themes via the iced adapter, NOT a custom Motrix palette / hardcoded `Color` constants.
5. **Config/data paths** - Linux/macOS/Windows paths match `directories` (`ProjectDirs::from("dev","remotrix","Remotrix")`).
6. **Build behavior** - docs state build needs NO network; aria2-next is fetched at runtime.
7. **Architecture** - sidecar + aria2-ws RPC + mpsc channels + 1Hz polling + SQLite flush.

## Output format
For each discrepancy:
- **File**: README.md / AGENTS.md
- **Section**: heading or line
- **Doc says**: current (stale) text
- **Code says**: correct fact + `source_file:line`
- **Suggested fix**: replacement text

End with: `N discrepancies found (X in README.md, Y in AGENTS.md).` If current: "Documentation is up to date."
```

Notes for the implementer:
- The command runs in the main session (no `subtask: true`) so it is an interactive "AI dialogue"
  as requested. `agent: code` routes to the default code agent.
- Filename `check-docs.md` -> invoked as `/check-docs`.

---

## Validation

1. `cargo build` still compiles (guards against accidental `src/` touches; README/AGENTS edits can't
   break it). Expect the pre-existing unused-variable warning in `src/ui/sidebar.rs:11` only.
2. Grep `AGENTS.md` for stale markers - expect 0 hits each:
   - `SetSpeedLimit` (removed variant)
   - `Motrix` (stale theme naming)
   - `dark-light 2` / `0.13` (stale versions)
   - `aria2-core` (must never appear)
3. Grep `AGENTS.md` for new correct markers - expect >=1 hit each:
   - `ApplyAria2Options`, `AddTorrent`, `FetchTaskDetails`
   - `opaline`, `rusqlite`, `tracing-appender`
4. Confirm `/check-docs` is discoverable: file exists at `.kilo/command/check-docs.md` with valid
   YAML frontmatter (`description`, `agent`).
5. Eyeball: the edited `EngineCmd`/`EngineEvent` blocks in `AGENTS.md` are byte-identical (modulo
   formatting) to `src/engine.rs:15-95`.

## Out of scope

- No `src/*`, `Cargo.toml`, `build.rs`, or `README.md` changes.
- No fixing of the `src/ui/sidebar.rs` unused-variable warning (pre-existing, unrelated).
- The `/check-docs` command only reports; it does not auto-apply fixes.
- `AGENTS.md`'s aria2-ws API reference, architecture narrative, build.rs process, and risks sections
  are still accurate - leave them untouched unless A8's optional `unpause_all` addition is desired.
