# Remotrix — AI Agent Context

Rust-native desktop download manager inspired by Motrix.app. Built with `iced` GUI + `aria2-next`
sidecar via `aria2-ws` RPC. See `docs/00-overview.md` for the full tech stack.

---

## Docs index (read on demand)

Detailed per-concern reference lives under `docs/`. **Before editing any area, Read the
corresponding file below** — these contain the gotchas, contracts, and conventions that are easy
to break.

| File | Read before touching… |
|---|---|
| `docs/00-overview.md` | the project pitch / tech stack |
| `docs/01-architecture.md` | the engine layer, RPC protocol, build.rs, release profile, `Cargo.toml` deps |
| `docs/02-conventions.md` | code style, commit messages, animation tokens, CHANGELOG DoD, docs-sync rule |
| `docs/03-build-test.md` | build/lint/test commands, `download_e2e`, CI workflows |
| `docs/04-versioning-release.md` | version bumps, semver policy, packaging/CI, GitHub release flow |
| `docs/05-update-flow.md` | aria2/app update pipeline, silent-update scope, `check_updates`, pending-update edge cases |
| `docs/06-ed2k.md` | ED2K bootstrap cache, server/node list sync, search wiring |
| `docs/07-behavior-gotchas.md` | engine spawn pruning, top-border progress, clipboard Smart filter, toolbar filter scoping, CJK font, `XDG_DATA_HOME` quirk |
| `docs/08-testing.md` | writing/editing tests: e2e `ARIA2_BIN` + `#[serial]`, `tests/common/` fixtures, pending-update fixture rule |

Detailed risk bullets from the old AGENTS.md live in their respective `docs/` file — most
belong with the feature they constrain (update → 05, ED2K → 06, top-border / clipboard / font /
error-task pruning / toolbar / XDG → 07).

---

## Global conventions (always apply)

- **Commit message format**: `type(scope): 中文描述 (English summary)`. Types: `feat`, `fix`,
  `perf`, `refactor`, `style`, `docs`, `build`, `ci`, `chore`, `test`, `revert`. Scope = affected
  module. ≤72 chars in the subject. See `docs/02-conventions.md` for full rules and examples.
- **CHANGELOG DoD**: any user-visible change (feat / fix / UI tweak / behavior change) **must**
  append a bullet under `## [Unreleased]` in `CHANGELOG.md` (Added / Changed / Fixed / Removed)
  in user-facing terms. Pure internal refactors / chore / dep bumps may skip.
- **Path resolution**: all on-disk paths must go through the resolvers in `src/config/` (`aria2_bin_dir()`,
  `session_dir()`, `db_path()`, `log_dir()`, `config_file_path()`). Modules must not call
  `directories::*` directly — the resolvers honour user overrides from `Settings::paths`.
- **Pre-commit checks**: `cargo clippy --workspace` (no warnings allowed) + `cargo fmt --check`.
  Run both before declaring a change done.
- **No comments in code** unless explaining a non-obvious design decision.

## Docs sync rule

Any PR that introduces a new external dependency, a new env var, a new manual run step, a new
failure mode, or a new operator-facing gotcha **must update the matching `docs/NN-*.md` file in
the same PR** — never "I'll add docs later". Code-only PRs that need a doc tweak but skip it are
rejected at review. See `docs/02-conventions.md` for the full convention.

## Build / check commands (short list)

```bash
cargo build                    # debug
cargo build --release          # release
cargo clippy --workspace       # lint, no warnings
cargo fmt --check              # formatting
cargo test --test download_e2e -- --nocapture   # needs ARIA2_BIN set; see docs/03-build-test.md
```

Full command list + e2e caveats + CI workflow are in `docs/03-build-test.md`.

## Release notes

Hand-maintained `CHANGELOG.md` at repo root (Keep a Changelog 1.1.0). The
`.github/workflows/release.yml` uploads the current `CHANGELOG.md` as the GitHub Release body —
no auto-generation. See `docs/04-versioning-release.md` for the version policy and release flow.