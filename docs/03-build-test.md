# Remotrix — Build, Test & Verification

Part of the AI context doc set; index in `AGENTS.md`. Read this before changing build flags, CI, or
running the integration tests.

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
Run `/check-docs` (Kilo command) to audit README.md, AGENTS.md, and the
`docs/*.md` files against the codebase.