# Remotrix — Code, Commit, & Docs Conventions

Part of the AI context doc set; index in `AGENTS.md`. Read this before writing or reviewing any
Rust code, commit, or release-note entry.

---

## Code Conventions
- **Module structure**: `src/` is organised by domain. Top level: `app.rs`, `lib.rs`, `main.rs`
  plus the submodules `config/`, `core/`, `engine/`, `integration/`, `platform/`, `storage/`,
  `ui/`, `update/`, `updaters/`. Inside each submodule, files split by feature (e.g.
  `ui/{about,add,close,confirm,details,update}_dialog.rs`, `platform/{autostart,notify,tray,…}.rs`,
  `updaters/{app_updater,aria2_fetcher,download,updater}.rs`, `core/{i18n,logging,message,scheduler,task}.rs`,
  `storage/{db,ed2k_bootstrap,torrent_meta,trackers}.rs`).
- **UI pattern**: Each page is a `fn` returning `iced::Element<'_, Message, Theme>`; no widget OOP wrappers
- **Time pickers** (`Settings > Download > Speed Limits`): `iced_aw` clock component (`time_picker` feature) wrapped in `src/ui/components/time_picker.rs`; the wrapper re-seeds iced_aw state on the open transition via `tree.children[0].state` so reopening shows the committed value.
- **Theme**: single accent color → iced `Theme::custom` palette generation (`src/ui/theme.rs`), with the background derived as an M3-style surface from the accent hue (`surface_from_seed`); colors read from `iced::Theme::extended_palette()`, no hardcoded palette constants.
- **Naming**: `snake_case` for fns/vars, `PascalCase` for types/enums, `SCREAMING_SNAKE` for constants
- **Error handling**: Use `String` errors in engine layer, map to `EngineEvent::EngineStopped` for fatal
- **No comments** in source code unless explaining a non-obvious design decision
- **Imports**: Group as `std` → external crates → `crate::` (blank-line separated)
- **Path resolution**: All on-disk paths must go through the resolver functions in `src/config.rs` (`aria2_bin_dir()`, `session_dir()`, `db_path()`, `log_dir()`, `config_file_path()`). Modules must not call `directories::*` directly — the resolvers honour user path overrides from `Settings::paths` and the migration step in `main()`.

## Animation tokens (do not introduce new `*_MS` constants)
All transition timings live in `src/ui/animation.rs` as 7 named tokens aligned with the Microsoft
Fluent three-tier model (`ControlNormal/Fast/Faster`). The full table:

- `ANIM_HEAVY_ENTER_MS = 280` / `ANIM_HEAVY_EXIT_MS = 200` — prominent, owns the user's attention. Currently used by the task-card slide in/out.
- `ANIM_STANDARD_ENTER_MS = 220` / `ANIM_STANDARD_EXIT_MS = 160` — default tier for modal dialogs (Add / About / Close / Confirm / Update), toast cards, the speed HUD, the top-border progress bar, and the task progress bar. ~27% faster exit than enter.
- `ANIM_LIGHT_ENTER_MS = 180` / `ANIM_LIGHT_EXIT_MS = 120` — high-frequency UI feedback (sidebar filter pill, scroll-driven swaps). ~33% faster exit than enter.
- `ANIM_THEME_MS = 320` — continuous theme colour / light-dark transition, not an enter/exit pair.

Pair-selection rules: (1) is the new motion the user's primary attention target? → HEAVY. (2) does it interrupt the user (modal, toast, status message)? → STANDARD. (3) is it high-frequency UI feedback (filter pill, tab swap, hover affordance, ripple-equivalent)? → LIGHT. (4) is it a colour / palette transition rather than an element? → THEME. When in doubt, pick STANDARD. If none of the tokens fits, ask the user before adding a new one — do not invent local constants. Cross-cutting rules baked into the tokens: enter is always slower than exit by ~27-33% (mirrors iOS HIG / Material 3 / Fluent); the three standard easings (`ease_out_quad`, `ease_out_cubic`, `ease_in_out_quad` in the same file) cover the existing callsites and are the only easings to introduce; `ease_out_cubic` is the default for enter/exit scale animations. Every new overlay widget that takes the user's attention (new dialog kind, new toast kind, new drawer) should use `ANIM_STANDARD_*` rather than its own constant. The `Scale` widget (`src/ui/components/scale.rs`) and the dialog overlay scale (`src/ui/components/dialog.rs::BlockingOverlay::draw`) already do this — new overlays should follow the same pattern.

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

## Release notes
- Release notes are **not** auto-generated. They come from the hand-maintained `CHANGELOG.md` at the repo root (Keep a Changelog 1.1.0 format; top section is always `## [Unreleased]` with `### Added / ### Changed / ### Fixed / ### Removed` sub-headings).
- `.github/workflows/release.yml` only uploads the existing file as an artifact and passes it to `softprops/action-gh-release@v2` as the release body. The auto-generated config (`.github/changelog-config.json`) has been removed.
- **Definition of done for any user-visible change** (feat / fix / UI tweak / behavior change): before committing, the agent responsible for the change must append a bullet to the matching sub-section under `## [Unreleased]` in `CHANGELOG.md`. Use user-facing language (no `feat(ui):` style prefixes), one line per change; if a change spans multiple categories, write one line under each. Pure internal refactors, chores, or dependency bumps with no behavior change may be skipped.
- A change that *only* updates `CHANGELOG.md` itself should not be made; the file is committed alongside the code change it describes. A stray `docs(release):` commit re-ordering or rewording older entries is fine and does not enter the current release body.

## Repo conventions — docs sync
Any PR that introduces a new external dependency, a new env var, a new manual run step, a new failure mode, or a new operator-facing gotcha **must update the matching `docs/NN-*.md` file in the same PR** — never "I'll add docs later". Code-only PRs that need a doc tweak but skip it are rejected at review. `docs/03-build-test.md` (build commands / env vars) and the relevant gotchas chapter (`docs/05-update-flow.md`, `docs/06-ed2k.md`, or `docs/07-behavior-gotchas.md`) are the two places that catch the most landmines. When in doubt, add a one-line note in the appropriate file.