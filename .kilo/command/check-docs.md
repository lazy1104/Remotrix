---
description: Audit README.md and AGENTS.md for staleness vs the codebase (report-only)
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
- `src/ui/theme.rs` - theming approach (accent color → iced `Theme::custom` palette generation, NOT a custom palette / hardcoded `Color` constants)
- `src/ui/mod.rs` + `src/ui/*.rs` - module / file layout
- `src/aria2_fetcher.rs`, `src/updater.rs` - aria2-next lifecycle (cache, sha256, .installed, .pending-update, ARIA2_BIN)
- `src/db.rs`, `src/config.rs` - persistence + config/data dir paths
- `src/main.rs`, `build.rs` - fonts, logging (tracing-appender), build-time behavior (no network)

## What to verify
1. **Dependencies** - every crate + version + feature in `Cargo.toml` is reflected; no stale/missing entries.
2. **Channel protocol** - `EngineCmd`/`EngineEvent` variants and fields in `AGENTS.md` match `src/engine.rs` exactly.
3. **Code layout** - every file under `src/` is listed with an accurate one-line description; no phantom/missing files.
4. **Theming** - docs say a single accent color drives iced `Theme::custom` palette generation (primary + M3-style surface background derived from the accent), NOT `opaline` builtin themes, a custom Motrix palette, or hardcoded `Color` constants.
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
