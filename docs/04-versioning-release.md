# Remotrix — Versioning, Release & Packaging

Part of the AI context doc set; index in `AGENTS.md`. Read this before bumping the version, cutting
a release, or touching the packager / release workflow.

---

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
  4. Bump `version` in `packager.toml` to the same value (must stay in sync; see below).
  5. Commit, then push a tag `v<version>`. `release.yml` triggers on tag push and runs `--locked`, so the
     lock **must** already be in sync — do not create the tag before steps 1–4.
- **A tag must point to a commit whose lock is in sync.** If a release fails, do NOT just re-tag the same
  commit: recreate the tag on the commit that actually fixed the lock, or cut a fresh `v0.1.y`. Retagging a
  stale commit (e.g. `git tag -f v0.1.2 <old-stale-commit>`) will re-fail the same way.

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
- The app also installs a per-user `.desktop` at runtime (`src/config/install.rs::install_desktop_file()`); a
  packaged `.deb` provides its own desktop entry. The runtime entry now writes `Icon=remotrix` and
  additionally installs a fallback `remotrix.png` under `$XDG_DATA_HOME/icons/hicolor/256x256/apps/`
  (sourced from `include_bytes!("../../assets/icon.png")`) so the icon resolves in GNOME whether or
  not the deb's system-scope hicolor icon is present. The user-scope entry still shadows the deb entry
  by XDG precedence; this is intentional (it points `Exec` at the dev/installed binary and refreshes
  `StartupWMClass`). The content-equality short-circuit in `install_desktop_file` rewrites the file
  whenever the template changes, so upgrading an old icon-less entry self-heals on the next launch —
  no manual `rm ~/.local/share/applications/remotrix.desktop` is required.