# Settings: Disable Apply when clean + Reset button (rev. 2)

## Goal
- Apply button is **disabled** while the editable settings equal the last-applied settings; the comparison happens **inside `src/ui/settings_page.rs`** (the view), not via a state flag.
- When there are un-applied changes, a **Reset** button (secondary) appears **to the right of Apply** — so Apply never shifts position when Reset appears/disappears.
- The Apply/Reset button row is **always shown** on every settings category (no `needs_apply` gate).

Rev. 1 had already added (kept as-is): `Message::ResetSettings`, `Tr::Reset`, FTL keys `reset = Reset` / `reset = 重置`, `Aria2Options: PartialEq`. Rev. 2 only refactors the dirty-detection location, button order, and visibility.

## Implemented changes

### 1. `src/config.rs`
Added `Settings::apply_fields_equal` to the `impl Settings` block containing `record_path`:
```rust
pub fn apply_fields_equal(&self, other: &Settings) -> bool {
    self.download_dir == other.download_dir
        && self.max_concurrent == other.max_concurrent
        && self.download_limit_kb == other.download_limit_kb
        && self.upload_limit_kb == other.upload_limit_kb
        && self.split == other.split
        && self.nav_to_tasks_after_add == other.nav_to_tasks_after_add
        && self.delete_torrent_after_complete == other.delete_torrent_after_complete
        && self.aria2 == other.aria2
}
```
Same field set as `revert_apply_settings`; theme/locale/update/window fields excluded (applied immediately, not part of Apply flow). This is the single source of truth used by both the view and app.rs.

### 2. `src/app.rs` — removed the `settings_dirty` flag (on-demand comparison)
- Deleted the `settings_dirty: bool` field and its `settings_dirty: false` init.
- Deleted the `editable_equal` free fn.
- `revert_apply_settings`: removed `state.settings_dirty = false;` (still restores field values, `download_picker` value, and `ua_editor` content).
- `Message::NavigatePage` gate became:
  ```rust
  if page == Page::Tasks
      && state.page == Page::Settings
      && !state.settings.apply_fields_equal(&state.applied_settings)
  ```
- Removed the recompute line in `Message::SettingChanged` (the `state.settings.aria2.user_agent = value` write for `SettingKey::UserAgent` stays — dirty is now derived).
- `Message::ApplySettings`: removed `state.settings_dirty = false;` (the `applied_settings` snapshot makes the comparison equal).
- `Message::UaEditor`: removed the recompute line (write to `settings.aria2.user_agent` stays).
- `Message::ApplyAndLeaveSettings`: removed `state.settings_dirty = false;`.
- `apply_path` DownloadDir branch: removed the recompute line.
- `Message::ResetSettings` arm: unchanged (`revert_apply_settings` + `config::save`).
- View call: replaced `state.settings_dirty,` with `&state.applied_settings,`.

### 3. `src/ui/settings_page.rs`
- `view()` signature: replaced `settings_dirty: bool` with `applied_settings: &'a Settings`.
- Computed at the top: `let dirty = !settings.apply_fields_equal(applied_settings);`
- Deleted the `needs_apply` local.
- Replaced the `if needs_apply { ... }` block with an unconditional button row; Apply first, then Reset to its right:
  ```rust
  let mut actions = row![].spacing(12).width(Length::Fill);
  actions = actions.push(
      button(text(fluent.get(Tr::Apply)).size(14))
          .on_press_maybe(if dirty { Some(Message::ApplySettings) } else { None })
          .padding([10, 24])
          .style(theme::style::button::primary()),
  );
  if dirty {
      actions = actions.push(
          button(text(fluent.get(Tr::Reset)).size(14))
              .on_press(Message::ResetSettings)
              .padding([10, 24])
              .style(theme::style::button::secondary()),
      );
  }
  body = body.push(actions);
  ```
- No orphaned imports: `SettingsCategory` is still used by `view()`'s signature and `settings_title`.

### 4. `src/message.rs`, `src/i18n.rs`, `i18n/locales/{en,zh-CN}/main.ftl`
No changes (already done in rev. 1).

## Behavior / edge cases
- Dirty state is derived on every render: the view compares `settings` vs `applied_settings` directly; app.rs compares on demand only for the leave-settings confirm dialog gate. Single source of truth (`Settings::apply_fields_equal`), no stale-flag risk.
- General/Advanced/Ed2k tabs: the button row is always visible; Apply stays disabled there unless editable fields were changed on another tab (dirty persists across tabs). Theme/locale/auto-check changes apply immediately and never enable Apply.
- Speed-unit dropdown (`SpeedUnitChanged`) never enables Apply. Reset does not reset the unit dropdown (display preference, consistent with Discard).
- Reset never sends engine commands — it restores exactly what the engine already has applied.
- Minor accepted edge case: picking the *same* download dir as applied leaves dirty=false, so that history entry is persisted only on a later real Apply.

## Validation
- `cargo build` ✅
- `cargo clippy --workspace` ✅ (no warnings)
- `cargo fmt --check` ✅
- Manual: Settings → Download: Apply disabled. Change a value → Apply enabled, Reset appears to its right (Apply does not move). Click Reset → values revert, Apply disabled, Reset hidden. Apply → dirty cleared. Navigate to Tasks with changes → confirm dialog still appears; Discard behaves as before. Buttons row visible on all 6 category tabs.
