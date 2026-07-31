# Per-Task Proxy in Add Dialog

## Goal
Add a per-task proxy override to the **Advanced Options** section of the New Download dialog. When the user fills a proxy there, the task is added with its own `all-proxy` (overriding the global setting for that task); when left empty, the task inherits the global proxy (aria2 semantics — empty is filtered to `None` by the engine).

The new proxy fields form a distinct sub-section inside the advanced form, separated from the original advanced fields by a divider + section title.

## Design decisions (confirmed with user)
1. **Three fields** — Address / Username / Password, mirroring the Settings → Network page. Reuses the existing URL-building logic and i18n keys; no hand-written `user:pass@`.
2. **Empty Address → inherit global proxy**: do NOT set `TaskOptions.all_proxy` in that case (aria2 falls back to the global value). There is no "explicitly disable proxy for this task" path — out of scope.
3. **UI separation**: in the advanced form, after the existing fields (User-Agent, HTTP auth, Referer, Cookie), push a `rule::horizontal(1)` divider, a section title `Tr::Proxy` (accent-colored, same style as settings `group_title`), then the three proxy fields.
4. **Reuse i18n**: `Tr::Proxy`, `Tr::ProxyAddress`, `Tr::ProxyUsername`, `Tr::ProxyPassword` already exist — no new FTL keys.
5. Password field uses `.secure(true)` (same `advanced_field` pattern as `AddField::HttpPasswd`).

## Changes by file

### 1. `src/config.rs` — extract shared URL builder
- Add a free function (module-level, `pub`):
  ```rust
  pub fn all_proxy_url(server: &str, username: &str, password: &str) -> Option<String> {
      if server.trim().is_empty() {
          return None;
      }
      let server = server.trim();
      let auth = if username.is_empty() {
          String::new()
      } else {
          format!("{}:{}@", username, password)
      };
      if let Some((scheme, rest)) = server.split_once("://") {
          Some(format!("{scheme}://{auth}{rest}"))
      } else {
          Some(format!("http://{auth}{server}"))
      }
  }
  ```
- Rewrite `Aria2Options::all_proxy_value` to delegate (behavior unchanged):
  ```rust
  pub fn all_proxy_value(&self) -> Option<String> {
      if !self.proxy_enabled {
          return None;
      }
      all_proxy_url(&self.proxy_server, &self.proxy_username, &self.proxy_password)
  }
  ```
- No import changes needed (helper lives in the same module).

### 2. `src/message.rs` — `AddField`
Add three variants after `Cookie`:
```rust
ProxyServer,
ProxyUsername,
ProxyPassword,
```

### 3. `src/engine.rs` — `TaskAdvancedOptions`
- Add fields to the struct:
  ```rust
  pub proxy_server: String,
  pub proxy_username: String,
  pub proxy_password: String,
  ```
- Extend `is_empty()` with the three fields.
- In `apply()`, after the `extra_options` loop, set the per-task proxy:
  ```rust
  if let Some(proxy) = crate::config::all_proxy_url(
      &self.proxy_server,
      &self.proxy_username,
      &self.proxy_password,
  ) {
      opts.all_proxy = Some(proxy);
  }
  ```
- Both `AddDownload` (engine.rs:450) and `AddTorrent` (engine.rs:591) call `advanced.apply(&mut options)`, so both paths pick it up automatically. No other engine changes.

### 4. `src/app.rs` — state handling
- `Message::AddFieldChanged` match (app.rs:378-385): add
  ```rust
  AddField::ProxyServer => add.proxy_server = value,
  AddField::ProxyUsername => add.proxy_username = value,
  AddField::ProxyPassword => add.proxy_password = value,
  ```
- `Message::AddDownload` construction (app.rs:391-402): clone the three new fields into `TaskAdvancedOptions { .. }`. Keep the existing torrent branch behavior — only `out` is cleared for torrents; proxy fields are passed through unchanged.

### 5. `src/ui/add_dialog.rs` — state + UI
- `AddDialogState`: add `proxy_server: String`, `proxy_username: String`, `proxy_password: String`.
- `new()` and `open()`: initialize/clear the three fields.
- `advanced_form` (line 261): change signature to `fn advanced_form<'a>(fluent: &'a Fluent, theme: &'a iced::Theme, state: &'a AddDialogState)` and append to the `column![]`:
  ```rust
  rule::horizontal(1),
  text(fluent.get(Tr::Proxy))
      .size(14)
      .color(theme::accent(theme))
      .style(theme::style::text::secondary),
  advanced_field(fluent, Tr::ProxyAddress, &state.proxy_server, AddField::ProxyServer, false),
  advanced_field(fluent, Tr::ProxyUsername, &state.proxy_username, AddField::ProxyUsername, false),
  advanced_field(fluent, Tr::ProxyPassword, &state.proxy_password, AddField::ProxyPassword, true),
  ```
  Note: `theme::accent` on `text` may be redundant with the text style — match the settings `group_title` look (`text(...).size(16).color(accent)`); drop `.style(...)` if it clashes, but keep the divider + title + three `advanced_field` rows.
- `view()` (line 196): call `advanced_form(fluent, theme, state)`.
- Import `rule` in the iced widget imports (`iced::widget::{..., rule, ...}`).

## Behavior / edge cases
- Empty Address → task inherits the global proxy (no `all-proxy` sent per task).
- Multi-URL add → the proxy applies to every URL in the batch (same options are cloned per `add_uri`).
- Torrent add → proxy applies to the torrent too (including its HTTP/BT downloads).
- Credentials with `@`/`:` are not percent-encoded (same known limitation as the settings page; out of scope).
- Global proxy setting keeps working for tasks that don't set a per-task proxy.

## Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual: New Download → Advanced options → divider + "Proxy" title appears; fill Address/Username/Password (password masked); Add → confirm the task downloads via the per-task proxy. Leave Address empty → task uses the global proxy. Both URL and .torrent paths should behave identically.
