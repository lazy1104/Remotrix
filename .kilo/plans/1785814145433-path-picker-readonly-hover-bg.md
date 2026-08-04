# PathPicker: read-only hover/focus fix + muted disabled background

## Goal
Make the read-only `PathPicker` rows (Engine Data Dir, Engine Session file, Log Location in Settings) actually show a hover/focus border highlight, and give them a muted "disabled" background + muted text so it is clear the field is not editable.

## Root cause (why hover currently doesn't work)
- Read-only pickers are **ephemeral**: `labeled_readonly` builds `let picker = PathPicker::read_only(...)` fresh every frame, so its internal `hovered`/`focused` state never survives across renders.
- Their `Entered`/`Exited` events are swallowed by the closure's `_ => Message::Noop` arm, so `update()` is never invoked and `self.hovered` is never set.
- The `mouse_area` wrapper (already applied to all modes) emits events, but without a persistent home for the state they go nowhere.

Editable pickers work because they live in `SettingsUiState` and every event routes through `AddMsg::PathPicker` → `update()` → persistent `hovered`.

## Confirmed decisions
- **Hover fix**: persist a per-path hovered flag in `SettingsUiState` (`HashSet<String>`), route `Entered`/`Exited` through a new `SettingsMsg`, and re-apply it to the recreated picker via a new `set_hovered` setter.
- **Disabled look**: muted surface background (`background.weak.color`) + muted path text (`background.weak.text`) for read-only mode, while keeping the hover/focus primary border highlight.

## Implementation tasks

### 1. `src/message.rs`
Add to `SettingsMsg`:
```rust
ReadOnlyHover { path: String, hovered: bool },
```

### 2. `src/app.rs`
Handle it (near the other `SettingsMsg` arms, e.g. after `ToggleScheduleDaysMenu` at ~line 2408):
```rust
Message::Settings(SettingsMsg::ReadOnlyHover { path, hovered }) => {
    if hovered {
        state.settings_ui.readonly_hovered.insert(path);
    } else {
        state.settings_ui.readonly_hovered.remove(&path);
    }
}
```

### 3. `src/ui/components/path_picker.rs`
- Add a setter (near `set_value`):
  ```rust
  pub fn set_hovered(&mut self, hovered: bool) {
      self.hovered = hovered;
  }
  ```
- Make the input style mode-aware (line ~173): `input` uses `theme::style::input::grouped` for editable modes and a new `theme::style::input::grouped_readonly` for `PickerMode::ReadOnly`.
- Pass `self.mode == PickerMode::ReadOnly` as the `read_only` flag to `grouped_frame_state` (line ~263).
- Keep the `mouse_area(inner)` wrapper applied to all modes (already present).

### 4. `src/ui/theme.rs`
- Extend `grouped_frame_state` with a `read_only: bool` parameter (line ~328):
  ```rust
  pub fn grouped_frame_state(
      focused: bool,
      hovered: bool,
      read_only: bool,
  ) -> impl Fn(&iced::Theme) -> iced::widget::container::Style {
      // background: if read_only { background.weak.color } else { background.base.color }
      // border logic unchanged (primary when focused || hovered)
  }
  ```
  Update the 3 callers:
  - `tag_picker.rs:74` → `grouped_frame_state(false, false, false)`
  - `number_stepper.rs:324` → `grouped_frame_state(state.focused, hovered, false)`
  - `path_picker.rs` → `grouped_frame_state(self.focused, self.hovered, self.mode == PickerMode::ReadOnly)`
- Add `input::grouped_readonly` (transparent bg, `value: p.background.weak.text`, same placeholder/selection as `grouped`):
  ```rust
  pub fn grouped_readonly(t: &iced::Theme, _status: text_input::Status) -> text_input::Style {
      let p = t.extended_palette();
      text_input::Style {
          background: iced::Background::Color(iced::Color::TRANSPARENT),
          border: iced::Border::default(),
          icon: p.background.weak.text,
          placeholder: p.secondary.base.color,
          value: p.background.weak.text,
          selection: p.primary.weak.color,
      }
  }
  ```

### 5. `src/ui/settings_page.rs`
- Import `std::collections::HashSet`.
- Add field to `SettingsUiState` and init in `new()`:
  ```rust
  pub readonly_hovered: HashSet<String>,
  // in new(): readonly_hovered: HashSet::new(),
  ```
- Thread `settings_ui: &SettingsUiState` into `advanced_view` (line ~1126) and `logging_view` (line ~1360); pass it from `view()` (line ~147) and from `advanced_view` when calling `logging_view` (line ~1348).
- `labeled_readonly` (line ~1640): add a `hovered: bool` param, set it on the picker, and map hover events:
  ```rust
  let mut picker = PathPicker::read_only(value.to_string());
  picker.set_hovered(hovered);
  let open_value = value.to_string();
  ... .view(fluent, theme, &[], move |e| match e {
      PathPickerEvent::Copy(s) => Message::Task(TaskMsg::CopyPath(s)),
      PathPickerEvent::Open => Message::Task(TaskMsg::OpenFolder(PathBuf::from(open_value.clone()))),
      PathPickerEvent::Entered => Message::Settings(SettingsMsg::ReadOnlyHover {
          path: open_value.clone(), hovered: true,
      }),
      PathPickerEvent::Exited => Message::Settings(SettingsMsg::ReadOnlyHover {
          path: open_value.clone(), hovered: false,
      }),
      _ => Message::Noop,
  })
  ```
- Update the 3 call sites to bind a path `String` and pass the hovered flag:
  - EngineDataDir (~1174): `let dir_str = dir.to_string_lossy().into_owned();` → `settings_ui.readonly_hovered.contains(&dir_str)`
  - EngineSessionFile (~1183): `let sf_str = sf.to_string_lossy().into_owned();`
  - LogLocation (~1396): `let dir_str = dir.to_string_lossy().into_owned();`

## Validation
- `cargo build` (no warnings).
- `cargo clippy --workspace` (no warnings).
- `cargo fmt --check`.
- Manual: in Settings → Advanced (Engine rows) and Logging (Log Location), hover a read-only row → border turns accent color; background stays muted; path text is muted. Clicking the row does not allow editing (no Browse button). Verify copy / reveal buttons still work.

## Risks / notes
- `grouped_frame_state` signature change touches 3 callers; update all or the build fails.
- The read-only muted text uses `background.weak.text`, which is only meaningful on the `background.weak` frame; keep both changes together.
- The left label text is left unchanged (only the path text inside the picker is muted). Optionally mute the label too if desired.
- Hover flag is stored per path string; entries are removed on `Exited`, so no stale-state accumulation in normal use.