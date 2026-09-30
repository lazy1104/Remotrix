# Remotrix — Testing: Hard Landmines Only

Part of the AI context doc set; index in `AGENTS.md`. Read this **before writing or modifying any
test** (unit or integration) so you don't produce a green-but-wrong suite.

This page deliberately holds ONLY the non-obvious rules that are easy to break and costly to debug.
Common-sense Rust test practice (where `#[cfg(test)] mod tests` lives, temp-dir uniqueness, no
magic literals) is covered by tooling + review, not repeated here. `docs/03-build-test.md` has the
run commands.

## Unit tests (in `src/`)
- No hard rules beyond general Rust practice. Cross-cutting invariants for the engine / updater /
  ED2K / UI state live in their own `docs/` chapter — read it before testing that area.

## Integration tests (in `tests/`)
- **Live aria2 is required.** `download_e2e` skips (logs `skip: ARIA2_BIN not set`, returns `Ok`)
  when `$ARIA2_BIN` is unset, so the suite stays green on CI/fresh boxes **without actually
  testing**. A green CI run does NOT prove the pipeline works — someone must run it locally with
  `ARIA2_BIN` set. CI is intentionally NOT wired to the live suite.
- **Serialise with `#[serial(aria2)]`** — the tests hit one shared aria2 sidecar + shared
  `~/.local/share/remotrix` state; running in parallel flakes.
- **Linux/macOS only.** Isolated via redirecting `$HOME` to a temp dir. Windows unsupported because
  `directories` reads `SHGetKnownFolderPath` there (ignores `$HOME`).
- **`#[ignore]`d tests are tracked follow-ups**, not dead weight: keep the attribute AND an inline
  comment naming the reason, so reviewers know it's intentional (see
  `tests/download_e2e.rs` for the per-test reasoning).
- **Reuse `tests/common/` fixtures first.** `common/http.rs` (axum HTTP/HTTPS + flaky/hang/500
  endpoints) and `common/bt.rs` (`BtSeeder` + `magnet_for_torrent`) exist exactly to avoid
  hand-rolled server boilerplate.

## Update / aria2 fixture rule
- Pending-update fixtures **must be real, executable binaries** (or sandbox paths that don't shadow
  an installed build). A shell placeholder that passes a `sha256` check can still fail the
  pre-promote smoke semantics and, worse, get promoted and delete the working aria2 install. Full
  detail: `docs/05-update-flow.md` ("pending-update promote can delete the working binary").