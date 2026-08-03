# Settings Persistence & UI Fixes Plan

## Goal

1. **Settings save discipline**: Only appearance (theme color, color mode) and language save immediately. All other settings (including font, BT tracker, etc.) only save to disk when Apply is clicked. On close, dirty (unapplied) changes are discarded.
2. **Font in General tab**: participates in dirty check → Apply/Reset.
3. **Reset button**: always visible, disabled when not dirty.
4. **"Restart aria2-next engine" button**: icon + text on the right side of actions bar, with confirmation if active downloads exist.

---

## Current Behavior (to change)

| Setting | Saves Immediately? | Should Save Immediately? |
|---|---|---|
| `ThemeModeChanged` | Yes (`config::save` in handler) | Yes (appearance) |
| `ThemeColorChanged` | Yes (`config::save` in handler) | Yes (appearance) |
| `LocaleChanged` | Yes (`config::save` in handler) | Yes (language) |
| `FontFamilyChanged` | Yes (`config::save` in handler) | **No** — should require Apply |
| All other settings | No (only on Apply) | No |
| `finalize_close` | Saves `state.settings` (including dirty) | **Should save `applied_settings`** (discard dirty) |

---

## Changes

### 1. `src/config.rs` — `apply_fields_equal`

Add `font_family` to comparison so font changes trigger dirty state:

```diff
 pub fn apply_fields_equal(&self, other: &Settings) -> bool {
     self.download_dir == other.download_dir
+        && self.font_family == other.font_family
         && self.max_concurrent == other.max_concurrent
         // ... rest unchanged
```

### 2. `src/app.rs` — `revert_apply_settings`

Add font_family restore so Reset reverts font changes:

```diff
 fn revert_apply_settings(state: &mut Remotrix) {
     state.settings.download_dir = state.applied_settings.download_dir.clone();
+    state.settings.font_family = state.applied_settings.font_family.clone();
     // ... rest unchanged
```

### 3. `src/app.rs` — `FontFamilyChanged` handler

Remove immediate `config::save()`. Font now persists only via Apply.

```diff
 Message::FontFamilyChanged(family) => {
     state.settings.font_family = family;
-    config::save(&state.settings);
 }
```

### 4. `src/app.rs` — `ThemeModeChanged`, `ThemeColorChanged`, `LocaleChanged`

These still save immediately (appearance/language). But they must also update `applied_settings` so that `finalize_close` (which saves `applied_settings`) doesn't overwrite them with stale values.

```diff
 Message::ThemeModeChanged(mode) => {
     state.settings.theme_mode = mode;
     rebuild_theme(state);
     config::save(&state.settings);
+    state.applied_settings.theme_mode = mode;
 }
 Message::ThemeColorChanged(color) => {
     state.settings.theme_color = theme::color_to_hex(color);
     rebuild_theme(state);
     config::save(&state.settings);
+    state.applied_settings.theme_color = state.settings.theme_color.clone();
 }
 Message::LocaleChanged(locale) => {
     state.settings.locale = locale;
     state.fluent = Fluent::new(locale);
     config::save(&state.settings);
+    state.applied_settings.locale = locale;
 }
```

### 5. `src/app.rs` — `finalize_close`

Save `state.applied_settings` instead of `state.settings` so dirty changes are discarded on close. Preserve window geometry (which is always updated) from current state.

```diff
 fn finalize_close(state: &mut Remotrix) -> Task<Message> {
     // ...
     sync_geometry_to_settings(state);
-    config::save(&state.settings);
+    let mut save = state.applied_settings.clone();
+    save.window_width = state.settings.window_width;
+    save.window_height = state.settings.window_height;
+    save.window_maximized = state.settings.window_maximized;
+    config::save(&save);
     // ...
 }
```

Note: `applied_settings` already has the correct theme/locale values because the immediate-save handlers now update them. The `ApplySettings` handler already does `state.applied_settings = state.settings.clone()`, so after Apply, everything is in sync.

### 6. `src/ui/settings_page.rs` — Reset button: always visible

Replace conditional rendering with `on_press_maybe`:

```diff
- if dirty {
-     actions = actions.push(
-         button(text(fluent.get(Tr::Reset)).size(FONT_BODY))
-             .on_press(Message::ResetSettings)
-             .padding(PADDING_BUTTON_XL)
-             .style(theme::style::button::secondary()),
-     );
- }
+ actions = actions.push(
+     button(text(fluent.get(Tr::Reset)).size(FONT_BODY))
+         .on_press_maybe(if dirty {
+             Some(Message::ResetSettings)
+         } else {
+             None
+         })
+         .padding(PADDING_BUTTON_XL)
+         .style(theme::style::button::secondary()),
+ );
```

### 7. `src/ui/settings_page.rs` — Restart engine button

Add to the actions row, right-aligned with a spacer. Uses `icon::refresh()` (codepoint `\u{E145}`).

```rust
// After Reset button, add spacer + restart button
actions = actions.push(iced::widget::horizontal_space(Length::Fill));
actions = actions.push(
    button(
        row![icon::refresh().size(FONT_ICON), text("Restart aria2-next engine").size(FONT_BODY)]
            .spacing(SPACE_SM)
            .align_y(Alignment::Center)
    )
    .on_press(Message::RestartEngine)
    .padding(PADDING_BUTTON_XL)
    .style(theme::style::button::secondary()),
);
```

### 8. `src/message.rs` — Add `ConfirmAction::RestartEngine`

```diff
 pub enum ConfirmAction {
     LeaveSettings { target: Page },
+    RestartEngine,
 }
```

### 9. `src/app.rs` — `RestartEngine` handler → confirm if active tasks

```rust
Message::RestartEngine => {
    let has_active = state.tasks.iter().any(|(_, t)| {
        matches!(t.status, TaskStatus::Active)
    });
    if has_active {
        state.confirm = Some(ConfirmAction::RestartEngine);
    } else {
        let _ = state.handle.cmd_tx.send(EngineCmd::RestartEngine);
    }
}
```

### 10. `src/app.rs` — Confirm dialog response handler

Find the match arm for `ConfirmAction` response (around line 1690). Add:

```rust
ConfirmAction::RestartEngine => {
    state.confirm = None;
    let _ = state.handle.cmd_tx.send(EngineCmd::RestartEngine);
}
```

### 11. `src/ui/confirm_dialog.rs` — Add `RestartEngine` dialog

```rust
ConfirmAction::RestartEngine => {
    let body = text(fluent.get(Tr::ConfirmRestartEngineBody));
    // buttons: Cancel (secondary) + Confirm (primary)
    let cancel_btn = button(text(fluent.get(Tr::Cancel)))
        .on_press(Message::ConfirmDialog(ConfirmDialogChoice::Cancel))
        .style(theme::style::button::secondary());
    let confirm_btn = button(text(fluent.get(Tr::Confirm)))
        .on_press(Message::ConfirmDialog(ConfirmDialogChoice::Confirm))
        .style(theme::style::button::primary());
    // ...
}
```

### 12. `src/i18n.rs` — Add translation strings

Add `Tr::ConfirmRestartEngineBody` with Chinese and English translations:
- zh: "有下载任务正在运行，重启后任务将暂停并恢复。确定重启？"
- en: "Active downloads will be paused and resumed after restart. Continue?"

---

## Edge Cases

| Scenario | Handling |
|---|---|
| Change font → close without Apply | `finalize_close` saves `applied_settings` → font change discarded |
| Change theme → close without Apply | Theme saved immediately + `applied_settings` updated → preserved |
| Change BT tracker → close without Apply | `finalize_close` saves `applied_settings` → BT change discarded |
| Apply → close | `applied_settings = settings.clone()` on Apply → everything saved |
| Reset → close | Reset calls `revert_apply_settings` → `applied_settings` unchanged → fine |
| Restart engine while tasks active | Confirm dialog shown; on confirm, engine restart preserves tasks via session save |
| Restart engine while app closing | `closing = true` prevents new restart; shutdown sequence takes priority |
| Restart engine during abnormal exit | Engine supervisor handles crash; on next boot, aria2 reads session file |
| Font changed + Apply → restart not needed | Font needs app restart; `RestartEngine` only restarts engine; separate `RestartApp` for font |

---

## Files to Modify

| File | Changes |
|---|---|
| `src/config.rs` | Add `font_family` to `apply_fields_equal` |
| `src/app.rs` | `revert_apply_settings` (add font restore), `FontFamilyChanged` (remove save), `ThemeModeChanged/ThemeColorChanged/LocaleChanged` (update `applied_settings`), `finalize_close` (save `applied_settings`), `RestartEngine` (add confirm check), confirm dialog handler |
| `src/message.rs` | Add `ConfirmAction::RestartEngine` |
| `src/ui/settings_page.rs` | Reset always visible via `on_press_maybe`, add restart engine button with icon |
| `src/ui/confirm_dialog.rs` | Add `RestartEngine` dialog variant |
| `src/i18n.rs` | Add `Tr::ConfirmRestartEngineBody` translations |

---

## Validation

1. `cargo clippy --workspace` — no warnings
2. `cargo build` — compiles
3. Manual: change font → Apply/Reset appear → Apply saves → Reset reverts → close without Apply → font NOT persisted
4. Manual: change theme/locale → saves immediately → close → theme/locale preserved
5. Manual: Reset button always visible, disabled when clean
6. Manual: Restart engine button works, shows confirm when tasks active
7. Manual: change BT tracker → close without Apply → BT tracker NOT persisted
8. Manual: change BT tracker → Apply → close → BT tracker persisted