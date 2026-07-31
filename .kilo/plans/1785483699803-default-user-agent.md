# Settings page: UA default, remove Headers, fix Apply button position

## Goal
Three settings-page adjustments (all in Network/Download settings):
1. **User-Agent default** — give `aria2.user_agent` a sensible non-empty default.
2. **Remove Headers feature** — delete the Headers editor from the Network page and all its
   plumbing (config field, message variants, app state, i18n).
3. **Apply button position** — move it out of the scrollable content, pinned below the scroll
   area, left-aligned (currently it's inside the scrolled content, pushed right, so it's hard
   to reach after scrolling down).

Note: the global speed HUD (bottom-right capsule) is **explicitly out of scope** — user
confirmed "参考这个位置就行，不要对它改动" (do not modify it).

The working tree already has uncommitted in-flight changes (`min_split_size` → `min_split_size_mb`
in `src/config.rs` / `src/app.rs` / `src/ui/settings_page.rs`); implement on top of them.

## Task 1: User-Agent default value — `src/config.rs`
- Add a helper near the other `default_*` fns (`src/config.rs:57`):
  ```rust
  fn default_user_agent() -> String {
      format!("Remotrix/{}", env!("CARGO_PKG_VERSION"))
  }
  ```
  → `Remotrix/0.1.0` (same version string style as `src/ui/about_dialog.rs:22`).
- On the field (`src/config.rs:31-32`): `#[serde(default)]` → `#[serde(default = "default_user_agent")]`.
- In `impl Default for Aria2Options` (`src/config.rs:100`): `user_agent: String::new()` →
  `user_agent: default_user_agent()`.
- Edge: serde default only applies when the `user_agent` key is absent; existing saved
  `"user_agent": ""` keeps its value (user can edit in Network page). No migration needed.
- Per-task UA (`engine.rs` `TaskAdvancedOptions.user_agent`, add dialog) is separate — unchanged.

## Task 2: Remove Headers feature (full removal)
### `src/config.rs`
- Remove the `headers: Vec<String>` field + its `#[serde(default)]` (`src/config.rs:33-34`).
- In `Settings::to_aria2_task_options`: delete the `let header = ...` block (`src/config.rs:217-221`)
  and change `header,` (`src/config.rs:231`) → `header: None,`.
- Legacy `settings.json` with a `headers` key under `aria2` is ignored on load (no
  `deny_unknown_fields` on `Aria2Options`) — safe, no migration.

### `src/message.rs`
- Remove `HeadersEditor(iced::widget::text_editor::Action)` (`src/message.rs:115`).
- Remove `SettingKey::Headers` (`src/message.rs:224`).

### `src/app.rs`
- Remove struct field `headers_editor: text_editor::Content` (`src/app.rs:57`).
- Remove init `let headers_editor = ...` (`src/app.rs:89`) and struct init `headers_editor,`
  (`src/app.rs:148`).
- Remove reload lines (`src/app.rs:218-219`).
- Remove `SettingKey::Headers` match arm (`src/app.rs:591-597`).
- Remove `Message::HeadersEditor` arm (`src/app.rs:1072-1082`).
- Remove `&state.headers_editor` from the `settings_page::view` call (`src/app.rs:1351`).

### `src/ui/settings_page.rs`
- Remove `headers_editor` param from `view` (`src/ui/settings_page.rs:80`) and from the
  `network_view` call (`src/ui/settings_page.rs:92`).
- Remove `headers_editor` param from `network_view` (`src/ui/settings_page.rs:493`).
- Remove the Headers group block in `network_view` (`src/ui/settings_page.rs:519-524`):
  the `.push(Space 16)` + `.push(group_title(Tr::Headers, ...))` + `.push(labeled_editor(...))`.

### i18n
- `src/i18n.rs`: remove `Tr::Headers` variant (`src/i18n.rs:115`) and its key mapping
  (`Tr::Headers => "headers"`, near `src/i18n.rs:285`).
- `i18n/locales/en/main.ftl:74` — remove `headers = Headers (one per line)`.
- `i18n/locales/zh-CN/main.ftl:74` — remove `headers = 请求头（每行一个）`.

## Task 3: Apply button below scrollable, left-aligned — `src/ui/settings_page.rs`
Current `view()` (`src/ui/settings_page.rs:106-137`): the Apply button is appended to `col`
which is wrapped by `slim_scrollable`, and pushed right via `Space::new().width(Length::Fill)`.

Restructure:
```rust
let needs_apply = matches!(...); // unchanged: Download | BitTorrent | Network

let mut body = column![]
    .push(text(settings_title(fluent, category)).size(22))
    .push(iced::widget::Space::new().height(Length::Fixed(20.0)))
    .push(slim_scrollable(content).height(Length::Fill));

if needs_apply {
    body = body.push(iced::widget::Space::new().height(Length::Fixed(16.0)));
    body = body.push(
        row![]
            .push(
                button(text(fluent.get(Tr::Apply)).size(14))
                    .on_press(Message::ApplySettings)
                    .padding([10, 24])
                    .style(theme::style::button::primary()),
            )
            .width(Length::Fill), // children stay left-aligned; no Space::Fill
    );
}

container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding([24, 28])
    .into()
```
- Left-aligned: omit the `Space::new().width(Length::Fill)` filler that currently pushes the
  button to the right edge.
- Always visible: button is outside `slim_scrollable`, so it no longer requires scrolling.
- `needs_apply == false` categories (General/Ed2k/Advanced) show no button, as before.

## Validation
1. `cargo build` (offline)
2. `cargo clippy --workspace` (no new warnings)
3. `cargo fmt --check`
4. Manual:
   - Network settings shows `Remotrix/0.1.0` in the User-Agent editor; Headers group is gone.
   - Download/BitTorrent/Network pages: Apply button sits below the scroll area, left-aligned,
     visible at all scroll positions; General/Ed2k/Advanced have no button.
   - Global speed HUD bottom-right unchanged.
