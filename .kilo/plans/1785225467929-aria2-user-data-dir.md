# Relocate aria2 files from app dir to user data dir

## Problem
`aria2_dir()` in `src/config.rs:372` prefers `<exe_dir>/aria2` (the application/install directory) whenever it is writable, falling back to the user data directory only when the exe dir is not writable. As a result the aria2-next binary, `session.txt`, `.installed`, and `.pending-update` all land next to the executable instead of in the user data directory.

Note: `settings.json` (`config_path`), `remotrix.db` (`db_path`), and `logs/` (`log_dir`) are **already** in the user data directory via `directories::ProjectDirs`. They are not affected by this change. The only offender is `aria2_dir()`.

## Root Cause (verified by reading source)
- `aria2_dir()` (config.rs:372-389) tries `exe_dir().join("aria2")` first, runs a writability probe, and returns it on success.
- `session_dir()` (config.rs:404) and `aria2_bin_dir()` (config.rs:408) both delegate to `aria2_dir()`, so the binary, session, `.installed`, and `.pending-update` all follow the exe-dir preference.
- `exe_dir()` (config.rs:365) is used **only** by `aria2_dir()` (confirmed via repo-wide grep). Nothing else references it.
- `ARIA2_BIN` env override in `aria2_fetcher.rs:64-72` is a separate, earlier code path and is unaffected.

## Decision
**Make `aria2_dir()` always use `ProjectDirs.data_dir()/aria2`**, mirroring the existing `log_dir()`/`db_path()` pattern. Remove the now-unused `exe_dir()` and the writability-probe/fallback logic. `session_dir()` and `aria2_bin_dir()` keep delegating to `aria2_dir()` unchanged.

**No migration of existing `<exe_dir>/aria2` files** (user-confirmed). After the switch, any files left in the old app-dir location are ignored; the app re-downloads the aria2-next binary and starts from an empty session on first launch in the new location. The `ARIA2_BIN` env override remains as the manual escape hatch.

## Tasks
1. **`src/config.rs`** - rewrite `aria2_dir()` (lines 372-389) to:
   ```rust
   fn aria2_dir() -> Option<PathBuf> {
       let proj = directories::ProjectDirs::from("dev", "remotrix", "Remotrix")?;
       let dir = proj.data_dir().join("aria2");
       let _ = std::fs::create_dir_all(&dir);
       Some(dir)
   }
   ```
   This matches `log_dir()` (config.rs:391) and `db_path()` (config.rs:398) exactly.
2. **`src/config.rs`** - delete the now-unused `pub fn exe_dir()` (lines 365-370). Confirmed no other call sites.
3. No changes to `session_dir()`, `aria2_bin_dir()`, `engine.rs`, `aria2_fetcher.rs`, `main.rs`, or `ui/settings_page.rs` - they all read through the existing functions and continue to work.
4. **`AGENTS.md`** - update the two stale references that say aria2-next caches in `<exe_dir>/aria2/`:
   - Line 20: change "caches in `<exe_dir>/aria2/` (falls back to `<data_dir>/aria2/`)" to state it caches in the user data directory `<data_dir>/aria2/`.
   - Line 133 (Build Process section): change "downloads from GitHub Releases ... to `<exe_dir>/aria2/`" to `<data_dir>/aria2/`; remove the "Falls back to `<data_dir>/aria2/` when exe dir is not writable" sentence since exe-dir is no longer used.

## Validation
- `cargo build` - compiles with `exe_dir()` removed (proves no dangling references).
- `cargo clippy --workspace` - no warnings.
- `cargo fmt --check` - formatting clean.
- `cargo run --` - on first launch in a clean state the aria2-next binary downloads into the user data dir (e.g. Linux `~/.local/share/remotrix/Remotrix/aria2/`); confirm via the Settings page "Engine data dir" / "Engine session file" read-only fields that the paths now point under the user data dir, not next to the executable.
- `config::announce()` logs (`config path`, `log dir`) remain unchanged; optionally extend tracing to log the aria2 dir for verification (optional, not required).

## Risk
- Low. The relocation is isolated to one helper; all consumers go through `session_dir()`/`aria2_bin_dir()` and are unaffected.
- Existing users with files in `<exe_dir>/aria2` lose their cached binary (re-downloaded, a few MB) and aria2 session state (in-progress downloads not resumed). Accepted per user decision (fresh start). The `ARIA2_BIN` env var remains available to point at a specific binary manually.
- If `ProjectDirs::from(...)` returns `None` on an exotic platform, `aria2_dir()` returns `None` and the engine degrades gracefully (same as today's fallback behavior for `log_dir`/`db_path`).
