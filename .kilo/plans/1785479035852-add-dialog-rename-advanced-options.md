# Plan: Add rename + advanced options to the New Download dialog

## Goal
Extend the "New Download" dialog (`src/ui/add_dialog.rs`) with:
1. A **rename** input (aria2 `out`).
2. An **Advanced options** checkbox at the bottom that expands a form containing: User-Agent, HTTP auth (account, password), Referer (来源页面), Cookie.

These are **per-task, non-persisted** options passed through to aria2 via `EngineCmd`.

## Confirmed decisions
- **Rename (`out`)**: shown only in URL mode; **hidden** when a torrent file is selected; **disabled with a hint** when >1 URL is entered (aria2 `out` only applies to single-URI downloads).
- **Auth fields**: HTTP auth only → `http-user` + `http-passwd` (one group "HTTP 认证").
- **Persistence**: per-task only; reset when the dialog reopens. No Settings/DB changes.

## aria2 / aria2-ws mapping
`aria2_ws::TaskOptions` (src/options.rs) has direct fields `out` and `header`. The rest go into `extra_options: Map<String, Value>` with kebab-case keys:
- `user-agent` → String
- `http-user` → String
- `http-passwd` → String
- `referer` → String
- `cookie` → String

Build `extra_options` via `options.extra_options.insert("user-agent".into(), Value::String(...))` etc. Only insert non-empty values so defaults are untouched.

## Files to change

### 1. `src/message.rs`
- Add a new enum for add-dialog advanced fields:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum AddField {
      Out,            // rename
      UserAgent,
      HttpUser,
      HttpPasswd,
      Referer,
      Cookie,
  }
  ```
- Add `Message::AddFieldChanged(AddField, String)` and `Message::ToggleAdvanced(bool)`.
- (No new SettingKey variants needed.)

### 2. `src/engine.rs`
- Define a reusable per-task advanced payload (in engine.rs, exported):
  ```rust
  #[derive(Debug, Clone, Default)]
  pub struct TaskAdvancedOptions {
      pub out: String,
      pub user_agent: String,
      pub http_user: String,
      pub http_passwd: String,
      pub referer: String,
      pub cookie: String,
  }
  impl TaskAdvancedOptions {
      pub fn is_empty(&self) -> bool { ... }
      pub fn apply(&self, opts: &mut TaskOptions) { /* set out, insert extra_options */ }
  }
  ```
  - `apply`: if `!out.is_empty()` set `opts.out = Some(out)`. For each non-empty user_agent/http_user/http_passwd/referer/cookie, `opts.extra_options.insert(key, Value::String(val))`.
- Extend `EngineCmd::AddDownload { urls, save_dir, split, advanced: TaskAdvancedOptions }`.
- Extend `EngineCmd::AddTorrent { path, save_dir, split, advanced: TaskAdvancedOptions }` (rename `out` is ignored for torrents by aria2, but user-agent/cookie/referer still apply; pass through uniformly).
- In `handle_client_cmd` for `AddDownload`/`AddTorrent`: call `advanced.apply(&mut options)` before `add_uri`/`add_torrent`.
- Update all other `EngineCmd::AddDownload`/`AddTorrent` construction sites (only `src/app.rs`).

### 3. `src/ui/add_dialog.rs`
- Extend `AddDialogState`:
  ```rust
  pub out: String,
  pub advanced_open: bool,
  pub user_agent: String,
  pub http_user: String,
  pub http_passwd: String,
  pub referer: String,
  pub cookie: String,
  ```
- `new()`: initialize all to `String::new()`, `advanced_open: false`.
- `open()`: reset all of the above to empty / `false` (per-task, non-persisted).
- Add helper `fn url_count(state) -> usize` counting non-empty trimmed lines of `url_editor.text()`.
- Add helper `fn has_torrent(state) -> bool` = `!torrent_picker.value().is_empty()`.
- `view()`:
  - After `save_row`, add a **rename row**: label (Tr::RenameFile) + `text_input`. Hidden when `has_torrent`. When `url_count > 1`, render disabled (no `on_input`, dimmed via `style`) and append a small hint text (Tr::RenameMultiUrlHint). Bind `on_input` to `Message::AddFieldChanged(AddField::Out, s)`.
  - Keep existing `split_input`.
  - Add at the bottom (above buttons) an **Advanced options** checkbox: `iced::widget::checkbox(fluent.get(Tr::AdvancedOptions), state.advanced_open).on_toggle(Message::ToggleAdvanced)`.
  - When `state.advanced_open` is true, push an advanced form `column` (spacing 8) containing, each as a labeled `text_input` row (label width ~140, input `Length::Fill`, `style(theme::style::input::standard)`, size 13):
    - User-Agent (Tr::UserAgent) → `AddField::UserAgent`
    - HTTP 账号 (Tr::HttpAuthAccount) → `AddField::HttpUser`
    - HTTP 密码 (Tr::HttpAuthPassword) → `AddField::HttpPasswd` (use a password-style input: `text_input("", &state.http_passwd).secure(true)`)
    - 来源页面 (Tr::Referer) → `AddField::Referer`
    - Cookie (Tr::Cookie) → `AddField::Cookie`
  - `can_submit()` unchanged (rename/advanced are optional).

### 4. `src/app.rs`
- Handle new messages in `update`:
  - `Message::ToggleAdvanced(v)` → `state.add_dialog.advanced_open = v;`
  - `Message::AddFieldChanged(field, s)` → match field to set the corresponding `state.add_dialog.*` string.
- In `Message::AddDownload`:
  - Build `TaskAdvancedOptions` from `state.add_dialog` (only include `out` if URL mode, single URL, and non-empty; torrent branch still builds user-agent/cookie/referer).
  - Pass `advanced` into `EngineCmd::AddDownload` and `EngineCmd::AddTorrent`.

### 5. `src/i18n.rs` + locale `.ftl` files
Add `Tr` variants and key strings (both `i18n/locales/zh-CN/main.ftl` and `i18n/locales/en/main.ftl`), near the existing add-dialog keys:
- `RenameFile` → zh "重命名", en "Rename"
- `RenameMultiUrlHint` → zh "仅单链接时可用", en "Only available for a single link"
- `AdvancedOptions` → zh "高级选项", en "Advanced options"
- `HttpAuth` (group title, optional) → zh "HTTP 认证", en "HTTP Authentication"
- `HttpAuthAccount` → zh "账号", en "Account"
- `HttpAuthPassword` → zh "密码", en "Password"
- `Referer` → zh "来源页面", en "Referer"
- `Cookie` → zh "Cookie", en "Cookie"
- `UserAgent` already exists (Tr::UserAgent) → reuse it.

Add the `Tr::` arms in the `as_key()` match and ensure both ftl files get the new entries.

## Edge cases / risks
- **Multi-URL rename**: UI disables the field; engine guard: when `urls.len() > 1`, do NOT set `out` even if the string is non-empty (defense in depth in `TaskAdvancedOptions::apply` — actually `apply` is generic; put the multi-URL guard in `app.rs` before constructing `advanced`, leaving `out` empty).
- **Torrent rename**: hidden in UI; engine ignores `out` for torrents (leave `out` empty in the torrent branch).
- **Empty strings**: never inserted into `extra_options` so aria2 defaults apply.
- **Password field**: use `secure(true)` so it is masked; value still travels in `EngineCmd` (in-process channel, not CLI/ps) — acceptable.
- **checkbox import**: `iced::widget::checkbox` exists in iced 0.14.
- **Dialog height**: advanced form adds height; the dialog body is in an overlay — verify it still fits at `width(520.0)`. If too tall, the `Dialog` should scroll; check `src/ui/components/dialog.rs` behavior. If no scroll, consider capping the advanced section with `slim_scrollable` and a max height.

## Validation
1. `cargo fmt --check`
2. `cargo clippy --workspace` (no warnings)
3. `cargo build` (offline, must succeed)
4. Manual (runtime): open New Download, enter one URL → rename field editable; enter two URLs → rename disabled with hint; select torrent → rename hidden. Toggle Advanced → form appears; fill User-Agent/Cookie/etc; submit; verify aria2 receives options (check aria2 log / `tell_status`). Confirm reopen resets all advanced fields.

## Out of scope
- Persisting advanced options across sessions.
- FTP auth fields.
- Per-task proxy (already a global setting).
- Headers field in the add dialog (exists only in Settings → Network).
