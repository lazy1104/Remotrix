# Remember Window Geometry (Size + Maximized State)

## Goal
Persist the window's last non-maximized size and its maximized state to `settings.json`, and restore them on launch — so the user doesn't have to resize every time they open the app.

## Scope (confirmed with user)
- **In:** window width/height (logical) + maximized flag.
- **Out:** window position (unreliable on Wayland / multi-monitor; `Event::Opened.position` is `None` on Wayland, off-screen risk on monitor changes).

## Key API facts (iced 0.14, verified)
- `iced::window::resize_events() -> Subscription<(Id, Size)>` — fires `Resized` on edge-drag resize AND on maximize/restore (incl. OS snap).
- `iced::window::is_maximized(id) -> Task<bool>` — query true maximized state.
- `Task::then(|T| Task<O>)`, `Task::done(value)`, `Task::none()` — for chaining a query into a message.
- `iced::window::Settings { size, maximized, min_size, .. }` — set at creation in `main.rs`.
- **Pitfall:** naively saving every `Resized` would persist the fullscreen size when maximized. Mitigation: only commit a resized size to the persisted value when `is_maximized` is false; keep the maximized flag separately.
- **Window creation happens in `main.rs`**, but `config::load()` runs later in `app::init()`. To restore on launch, `main.rs` must also load config (cheap, harmless double-read of a tiny JSON; both reads are consistent at startup).

## Bonus correctness improvement
`state.maximized` is currently toggled ONLY by the title-bar button (stale vs OS snap — noted risk in AGENTS.md). Syncing it from `is_maximized` queries also fixes the borderless resize-frame overlay (skipped when maximized) to correctly reflect OS-snap maximize.

## Design

### Settings (`src/config.rs`)
Add three fields to `Settings` (backward-compatible via serde defaults):
```rust
#[serde(default = "default_window_width")]
pub window_width: f32,     // default 1040.0
#[serde(default = "default_window_height")]
pub window_height: f32,    // default 720.0
#[serde(default)]
pub window_maximized: bool, // default false
```
Add the three `default_window_*` fns; also set them in `impl Default for Settings`.

### State (`src/app.rs` `Remotrix`)
Add fields:
- `window_size: iced::Size` — last committed **non-maximized** size (init from settings).
- `last_resize: Option<iced::Size>` — most recent `Resized` size, unverified (may be a maximize).
- `geometry_dirty: bool` — geometry changed since last persist.
- `pending_close: bool` — close window after the close-time geometry query commits.
- (`maximized: bool` already exists; now kept in sync with real state.)

`init()`: set `maximized: settings.window_maximized` (instead of `false`); `window_size: iced::Size::new(settings.window_width, settings.window_height)`; `last_resize: None`; `geometry_dirty: false`; `pending_close: false`.

### Messages (`src/message.rs`)
Add next to the existing `DragWindow`/`WindowAction` group:
- `WindowResized(iced::Size)`
- `WindowMaximized(bool)` — result of `is_maximized` query; commits geometry + saves; closes if `pending_close`.
- `PersistWindowGeometry` — from periodic timer.

### Subscriptions (`src/app.rs` `subscription`)
Add:
```rust
let resizes = iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size));
let persist = iced::time::every(Duration::from_millis(2000)).map(|_| Message::PersistWindowGeometry);
```
Add both to the `Subscription::batch` vec.

### Update logic (`src/app.rs` `update`)
Helper `fn sync_geometry_to_settings(state: &mut Remotrix)`:
```rust
state.settings.window_width = state.window_size.width;
state.settings.window_height = state.window_size.height;
state.settings.window_maximized = state.maximized;
```

`Message::WindowResized(size)`:
```rust
state.last_resize = Some(size);
state.geometry_dirty = true;
```

`Message::PersistWindowGeometry`:
```rust
if state.geometry_dirty {
    if let Some(id) = state.window_id {
        return iced::window::is_maximized(id)
            .then(|max| Task::done(Message::WindowMaximized(max)));
    }
}
```

`Message::WindowMaximized(max)`:
```rust
state.maximized = max;
if let Some(s) = state.last_resize {
    if !max { state.window_size = s; }   // commit only non-maximized sizes
    state.last_resize = None;
}
sync_geometry_to_settings(state);
config::save(&state.settings);
state.geometry_dirty = false;
if state.pending_close {
    state.pending_close = false;
    if let Some(id) = state.window_id {
        return iced::window::close::<Message>(id);
    }
}
```

`Message::CloseDialog(CloseDialogChoice::Close)` — replace the existing close path:
```rust
// keep existing EngineCmd::Shutdown send
if state.geometry_dirty {
    state.pending_close = true;
    if let Some(id) = state.window_id {
        return iced::window::is_maximized(id)
            .then(|max| Task::done(Message::WindowMaximized(max)));
    }
}
// not dirty (or no window id): capture any toggled maximized state, save, close
sync_geometry_to_settings(state);
config::save(&state.settings);
if let Some(id) = state.window_id {
    return iced::window::close::<Message>(id);
}
Task::none()
```
(`Cancel` / `MinimizeToTray` arms unchanged.)

`Message::WindowAction(WindowCmd::ToggleMaximize)` — keep as-is (toggles `state.maximized` + `toggle_maximize`); the resulting `Resized` + next query will re-confirm. No direct persist needed here (close path syncs it).

### Restore at launch (`src/main.rs`)
Load config in `main.rs` to feed window settings (init still loads its own copy for live state):
```rust
let cfg = crate::config::load();
let w = cfg.window_width.max(800.0);
let h = cfg.window_height.max(560.0);
// ...
.window(iced::window::Settings {
    size: iced::Size::new(w, h),
    maximized: cfg.window_maximized,
    icon: load_icon(),
    decorations: false,
    exit_on_close_request: false,
    min_size: Some(iced::Size::new(800.0, 560.0)),
    ..Default::default()
})
```
Rationale for setting BOTH `size` (saved non-max size) and `maximized`: when launched maximized and the user later restores, the OS restores to `Settings::size` = the saved non-maximized size.

## Edge cases handled
- First launch / missing fields → serde defaults 1040x720, not maximized (matches current behavior).
- Saved size below 800x560 → clamped via `.max()` on restore.
- Maximize (title bar or OS snap) → fullscreen size never persisted; only the flag + last non-max size.
- OS-snap to half-screen → saved as a normal size (desirable).
- Resize then close within 2s → close-time `is_maximized` query commits the unverified `last_resize`.
- Crash/kill between persists → at most 2s of geometry lost (periodic 2s save).

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` (no warnings)
- `cargo build`
- `cargo run --` and manually:
  - Resize to a non-default size, close, reopen → opens at saved size.
  - Maximize (title bar), close, reopen → opens maximized; restore → returns to saved non-max size (not fullscreen-size windowed).
  - OS-snap maximize, close, reopen → opens maximized (not a fullscreen-size windowed window).
  - Edge-resize down to 800x560 floor, close, reopen → clamped at floor.
  - Resize frame overlay correctly disappears when OS-snap-maximized (bonus fix).
  - Confirm `settings.json` now contains `window_width`, `window_height`, `window_maximized` and they update after a resize+pause (≤2s).

## Risks / Notes
- `config::load()` is called twice at startup (main.rs + init). Intentional; tiny JSON, negligible cost, reads are consistent.
- Periodic 2s `config::save` writes the whole `settings.json` when geometry is dirty; consistent with the existing pattern (theme/locale changes already call `config::save` synchronously). `state.settings` is the single source of truth, so no clobbering of in-flight edits.
- Position restore deliberately excluded (scope decision).
- `BORDER`/resize-frame behavior unaffected; only the `maximized` gating becomes more accurate.
