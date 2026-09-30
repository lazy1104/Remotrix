# Remotrix — Project Overview

Part of the AI context doc set; index in `AGENTS.md`. Read this when you need to know **what** the
project is and **what tech stack** it uses.

---

Rust-native desktop download manager inspired by Motrix.app. Built with `iced` GUI framework and
**aria2-next sidecar** (via `aria2-ws` RPC client).

| Component | Choice | Rationale |
|---|---|---|
| GUI | `iced 0.14` (+tokio, advanced, image, canvas) | Pure Rust, widget-based, multi-theme support |
| Engine | `aria2-next` sidecar + `aria2-ws 0.5` | C++ aria2 fork, JSON-RPC over WebSocket; spawned as subprocess |
| Async | `tokio 1.x` (full) | Shared runtime for engine + UI |
| Persistence | `rusqlite 0.40` (bundled + fallible_uint) | Embedded SQLite for task metadata / progress |
| Themes | iced `Theme::custom` (built-in) + `mundy 0.2` | Accent-color swatches; iced auto-generates light/dark palettes (primary + M3-style surface background) from the accent; system detection (ColorScheme + optional AccentColor via `mundy`) |
| i18n | `fluent-templates 0.15` | Fluent translations (zh/en) |
| File dialog | `rfd 0.17` | Native OS file picker |
| Config dirs | `directories 6` | XDG/user data paths |