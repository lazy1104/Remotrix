# Reorder About dialog: Core Dependencies section

## Goal
Reorder the About dialog so that directly under the app version number there is a
"核心依赖 / Core Dependencies" section header, followed by the iced version and then
the aria2-next version.

## Current layout (src/ui/about_dialog.rs body)
1. Logo + "ReMotrix" title
2. Tagline
3. App version `gui_text` (copyable) — `Remotrix {CARGO_PKG_VERSION}`
4. Engine version `engine_text` (copyable) — `Engine: aria2-next v{v}`
5. Built-with iced `iced_text` — `Built with iced 0.14`
6. License notice

## Target layout
1. Logo + "ReMotrix" title
2. Tagline
3. App version `gui_text` (copyable)
4. **"核心依赖 / Core Dependencies"** header (new, small secondary text)
5. iced version `iced_text`
6. aria2-next version `engine_text` (copyable) — `Engine: aria2-next v{v}`
7. License notice

## Changes

### 1. src/i18n.rs
- Add enum variant `CoreDependencies` (place near `AboutBuiltWith` / `AboutTagline`).
- Add match arm in `Tr::key()`: `Tr::CoreDependencies => "core-dependencies"`.

### 2. i18n/locales/en/main.ftl
- Add line near `about-tagline` / `about-built-with`:
  `core-dependencies = Core Dependencies`

### 3. i18n/locales/zh-CN/main.ftl
- Add line near `about-tagline` / `about-built-with`:
  `core-dependencies = 核心依赖`

### 4. src/ui/about_dialog.rs
- Add a new `core_dependencies` header element between the app-version
  `copyable_text` and the iced `iced_text` block, styled like the tagline:
  `text(fluent.get(Tr::CoreDependencies)).size(FONT_SMALL).style(theme::style::text::secondary)`
- Move the `engine_text` (aria2-next) `copyable_text` push to AFTER the
  `iced_text` push, so order becomes: version → core-deps header → iced → aria2-next.
- `engine_text` and `iced_text` content/format stay unchanged (still copyable /
  still uses `Tr::AboutBuiltWith`).

## Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual: open About dialog, confirm order is App version → Core Dependencies →
  iced → aria2-next, and copy buttons still work.

## Notes / Risks
- No signature changes to `view(...)`; only body reordering plus new i18n key.
- Keep existing copyable behavior for the aria2-next line.
