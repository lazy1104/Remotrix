# Network Proxy Settings Refactor

## Goal
Replace the current split proxy configuration (separate `Enable proxy` toggle + single `All proxy` text field) with one unified proxy block: an **Enable proxy** toggle that, when turned on, reveals three fields — **Address**, **Username**, **Password** (masked). Credentials are embedded into the aria2 `all-proxy` URL at apply time.

## Current behavior
- `Aria2Options` in `src/config.rs` stores `proxy_enabled: bool` + `all_proxy: String`.
- Network settings page (`network_view` in `src/ui/settings_page.rs`) shows: group title "Enable Proxy", the toggle, then a second group "Other Proxy Config" with one text input.
- `to_aria2_task_options` (config.rs:226) sends `all-proxy` = `Some(server)` when enabled, else `Some("")` (empty string clears a previously set proxy on `change_global_option`).

## Design decisions
1. **Field rename + migration**: rename `all_proxy: String` → `proxy_server: String` with `#[serde(default, alias = "all_proxy")]` so existing `settings.json` files keep their configured proxy address. Add `proxy_username: String` and `proxy_password: String`, both `#[serde(default)]`.
2. **Credentials go inside the proxy URL**: aria2-next (`aria2-core 0.2.3`) has no `all-proxy-user/passwd` options; its `ProxyUrl::parse` and `reqwest::Proxy::all` both support `scheme://[user:pass@]host[:port]`. A helper builds the URL:
   - if proxy disabled or `proxy_server` empty → `None`
   - trim the server; if it contains `://`, insert `user:pass@` right after the scheme; otherwise prefix `http://user:pass@`
   - credentials included only when username is non-empty
   - no percent-encoding of user/pass (accepted limitation, matches typical Motrix-style behavior)
3. **Clearing behavior preserved**: `to_aria2_task_options` keeps sending `Some("")` when the proxy is disabled/empty (`all_proxy_value().unwrap_or_default()`), so a previously applied proxy is cleared on Apply rather than left stale. (aria2-next's `get_str` filters empty strings to `None`, so `""` cleanly means "no proxy".)
4. **UI**: on the Network category, `group_title(Tr::Proxy)` + the `Enable proxy` toggle; the three text inputs render only when `settings.aria2.proxy_enabled` is true. Password input uses `.secure(true)` (same pattern as `add_dialog.rs:246`).
5. **Plaintext password in settings.json** is accepted — consistent with the existing design (no settings encryption anywhere; AGENTS.md already tolerates a plaintext CLI secret).

## Changes by file

### 1. `src/config.rs`
- `Aria2Options`: replace `pub all_proxy: String` with
  ```rust
  #[serde(default, alias = "all_proxy")]
  pub proxy_server: String,
  #[serde(default)]
  pub proxy_username: String,
  #[serde(default)]
  pub proxy_password: String,
  ```
- `Default` impl: replace `all_proxy: String::new()` with `proxy_server`, `proxy_username`, `proxy_password` all `String::new()`.
- Add helper on `impl Aria2Options`:
  ```rust
  pub fn all_proxy_value(&self) -> Option<String> {
      if !self.proxy_enabled || self.proxy_server.trim().is_empty() {
          return None;
      }
      let server = self.proxy_server.trim();
      let auth = if self.proxy_username.is_empty() {
          String::new()
      } else {
          format!("{}:{}@", self.proxy_username, self.proxy_password)
      };
      if let Some((scheme, rest)) = server.split_once("://") {
          Some(format!("{scheme}://{auth}{rest}"))
      } else {
          Some(format!("http://{auth}{server}"))
      }
  }
  ```
- `to_aria2_task_options`: replace the `all_proxy` arm (lines 226-230) with:
  ```rust
  all_proxy: Some(self.aria2.all_proxy_value().unwrap_or_default()),
  ```

### 2. `src/message.rs`
- `SettingKey`: remove `AllProxy`, add `ProxyServer`, `ProxyUsername`, `ProxyPassword`. Keep `EnableProxy`.

### 3. `src/app.rs`
- In the `SettingChanged` match (around line 665): replace the `AllProxy` arm with
  ```rust
  SettingKey::ProxyServer => state.settings.aria2.proxy_server = value,
  SettingKey::ProxyUsername => state.settings.aria2.proxy_username = value,
  SettingKey::ProxyPassword => state.settings.aria2.proxy_password = value,
  ```
- `revert_apply_settings` needs no change (clones the whole `Aria2Options`).

### 4. `src/i18n.rs`
- `Tr` enum: remove `OtherProxyConfig`; add `ProxyAddress`, `ProxyUsername`, `ProxyPassword`.
- `Tr::key()`: remove `"other-proxy-config"`; add `"proxy-address"`, `"proxy-username"`, `"proxy-password"`. Update `Tr::Proxy` key stays `"proxy"`.

### 5. `i18n/locales/en/main.ftl`
- `proxy = All proxy` → `proxy = Proxy`
- Remove `other-proxy-config = Other Proxy Config`
- Add:
  ```
  proxy-address = Address
  proxy-username = Username
  proxy-password = Password
  ```

### 6. `i18n/locales/zh-CN/main.ftl`
- `proxy = 代理服务器` → `proxy = 代理`
- Remove `other-proxy-config = 其他代理配置`
- Add:
  ```
  proxy-address = 地址
  proxy-username = 账号
  proxy-password = 密码
  ```

### 7. `src/ui/settings_page.rs` — `network_view`
- `labeled_text_input` (line 719): add a `secure: bool` parameter; apply `.secure(secure)` on the `text_input` when true. Update existing call sites (`BtTracker` at line 453, proxy address at line 503) passing `false`.
- Replace the proxy section (lines 495-507) with:
  ```rust
  .push(group_title(fluent, Tr::Proxy, accent))
  .push(labeled_toggle(fluent.get(Tr::EnableProxy), settings.aria2.proxy_enabled, SettingKey::EnableProxy))
  .push(proxy_fields(fluent, settings))
  ```
  where
  ```rust
  fn proxy_fields<'a>(fluent: &'a Fluent, settings: &'a Settings) -> Element<'a, Message> {
      if settings.aria2.proxy_enabled {
          column![
              labeled_text_input(fluent.get(Tr::ProxyAddress), &settings.aria2.proxy_server, SettingKey::ProxyServer, false),
              labeled_text_input(fluent.get(Tr::ProxyUsername), &settings.aria2.proxy_username, SettingKey::ProxyUsername, false),
              labeled_text_input(fluent.get(Tr::ProxyPassword), &settings.aria2.proxy_password, SettingKey::ProxyPassword, true),
          ]
          .into()
      } else {
          iced::widget::Space::new().height(Length::Fixed(0.0)).into()
      }
  }
  ```
- Remove the now-unused `Tr::OtherProxyConfig` usage; `Tr::Proxy` is now the group title.

## Behavior / edge cases
- Toggling proxy off hides Address/Username/Password but keeps their values in settings (so re-enabling restores them). Apply is still required for the engine to change; the toggle itself enables the dirty state.
- Disabled proxy on Apply → `all-proxy=""` sent, clearing any previously applied proxy (unchanged from current behavior).
- Server entered without scheme (`127.0.0.1:7890`) → treated as `http://...`.
- Existing `settings.json` with `all_proxy` loads into `proxy_server` via serde alias; no data loss.
- Username/password containing `@`, `:`, or non-ASCII are not percent-encoded (known limitation, out of scope).

## Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual: Settings → Network → Enable proxy → three fields appear (password masked). Fill address/user/pass, Apply → `change_global_option` receives `all-proxy` with embedded credentials; engine downloads through the proxy. Disable toggle → fields hide → Apply → downloads bypass proxy.
- Upgrade check: run once with a pre-existing `settings.json` containing `all_proxy`; the value appears in the Address field after relaunch.
