# Plan: Integrate opaline theme engine + split theme settings

## Goal
Replace the hand-crafted Motrix palette with the `opaline` crate (token-based theme engine) so users can pick real themes. Split the single "主题" setting into **three dropdowns** in Settings → General:

1. **浅色主题 (Light Theme)** — pick one of opaline's 12 light builtin themes.
2. **深色主题 (Dark Theme)** — pick one of opaline's 27 dark builtin themes.
3. **明暗 (Mode)** — 跟随系统 / 浅色 / 深色 (System / Light / Dark).

Effective theme = the picked light theme when mode resolves to light, the picked dark theme when it resolves to dark. "跟随系统" follows OS dark/light (via existing `dark_light` crate). This is conflict-free because each theme picker is already variant-filtered.

## Key decisions (confirmed with user)
- **3-dropdown model** (VS Code style): separate light-theme and dark-theme picks + a mode dropdown. Each theme dropdown lists ONLY themes of that variant, so there is never a light/dark mismatch.
- **opaline drives the palette.** Build the `iced::Theme` from the selected opaline theme via `opaline::adapters::iced::to_iced_custom`. Remove the hand-crafted `seed_palette`/`dark_background`/`light_background` and the hardcoded Motrix color constants; replace all usages with theme-aware accessors reading `iced::Theme::extended_palette()` (which is opaline-backed).
- **Naming (optimized):** group title `外观`/`Appearance`; rows `浅色主题`/`Light Theme`, `深色主题`/`Dark Theme`, `明暗`/`Mode`. Mode options reuse existing `跟随系统/浅色/深色` strings.

## opaline API facts (v0.4.1, verified)
- Cargo: `opaline = { version = "0.4", default-features = false, features = ["builtin-themes", "iced"] }`.
  - `iced` feature = enables `dep:iced_core` (0.14, same minor as project's `iced 0.14` → types unify) + the `adapters::iced` module.
  - `default-features = false` drops `ratatui`/`gradients` (unneeded). Keep `builtin-themes` (39 embedded themes).
- `opaline::builtins::list_available_themes() -> Vec<ThemeInfo>` (returns builtins when `discovery` off). `ThemeInfo { name: String (kebab id), display_name: String, variant: ThemeVariant, .. }`.
- `opaline::schema::ThemeVariant` = `Dark | Light` (Copy, PartialEq).
- `opaline::builtins::load_by_name(id: &str) -> Option<Theme>`; `opaline::Theme::default()` = `silkcircuit-neon` (Dark).
- `opaline::adapters::iced::to_iced_custom(&Theme) -> iced_core::theme::Custom` — builds a `Custom` from 6 tokens: `bg.base`→background, `text.primary`→text, `accent.primary`→primary, `success`→success, `warning`→warning, `error`→danger. Usage: `iced::Theme::Custom(Arc::new(custom))`.
- `From<OpalineColor> for iced_core::Color` exists (`Color::from_rgb8(r,g,b)`).
- Defaults to use: dark = `silkcircuit-neon`, light = `silkcircuit-dawn`.

## Implementation tasks

### 1. `Cargo.toml`
- Add `opaline = { version = "0.4", default-features = false, features = ["builtin-themes", "iced"] }`.

### 2. `src/ui/theme.rs` (rewrite theme building + accessors)
- Keep `ThemeMode` enum (System/Light/Dark), `detect_dark()`, `resolve_mode()` unchanged.
- Replace `build_iced(dark: bool)` with:
  ```rust
  pub fn build_iced(theme_id: &str) -> iced::Theme {
      let opaline = opaline::builtins::load_by_name(theme_id)
          .unwrap_or_else(opaline::Theme::default);
      let custom = opaline::adapters::iced::to_iced_custom(&opaline);
      iced::Theme::Custom(std::sync::Arc::new(custom))
  }
  ```
- Add theme-list helpers (centralize opaline imports here):
  ```rust
  pub fn themes_for_variant(variant: opaline::schema::ThemeVariant) -> Vec<(String, String)> {
      let mut v: Vec<_> = opaline::builtins::list_available_themes().into_iter()
          .filter(|i| i.variant == variant)
          .map(|i| (i.name, i.display_name))
          .collect();
      v.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
      v
  }
  pub fn light_themes() -> Vec<(String,String)> { themes_for_variant(opaline::schema::ThemeVariant::Light) }
  pub fn dark_themes()  -> Vec<(String,String)> { themes_for_variant(opaline::schema::ThemeVariant::Dark) }
  ```
- Add theme-aware color accessors (read from `extended_palette`, which opaline seeds):
  - `pub fn accent(t:&iced::Theme) -> Color { t.extended_palette().primary.base.color }`
  - `pub fn success(t) -> Color` (`success.base.color`) — replaces PROGRESS/SPEED.
  - `pub fn warning(t) -> Color` (`warning.base.color`) — replaces PAUSED.
  - `pub fn danger(t) -> Color` (`danger.base.color`) — replaces ERROR.
  - Update `text_secondary(t)` and `border_color(t)` to derive from `extended_palette` (e.g. background.base.text / background.strong.color) instead of the old constants.
- Remove now-unused constants: `BG_PRIMARY`, `BG_SIDEBAR`, `BG_CARD`, `ACCENT`, `PROGRESS`, `SPEED`, `ERROR`, `PAUSED`, `TEXT_PRIMARY`, `TEXT_SECONDARY`, `BORDER`, `TITLE_BAR`, `BG_*_LIGHT`, `TEXT_*_LIGHT`, `BORDER_LIGHT`, `TITLE_BAR_LIGHT`, `seed_palette`, `dark_background`, `light_background`. Keep `OVERLAY` (fixed scrim) — used by `style::overlay`.
- Update `style` fns that referenced removed constants to use `extended_palette`:
  - `active_filter`: accent via `t.extended_palette().primary.base.color`.
  - `button::sidebar_icon`: accent via extended_palette primary.
  - `button::new_download`: background = primary, text = `background.base.text`.
  - `button::window_control`: keep close-button red hardcoded (OS convention) — acceptable.
  - `progress::task`, `base_background`, `sidebar_background`, `category_background`, `card`, `text::secondary`: already use `extended_palette` — no change.

### 3. `src/config.rs`
- In `Settings`, replace the single `theme_mode` field block with three fields:
  ```rust
  #[serde(default = "default_light_theme")]
  pub light_theme: String,
  #[serde(default = "default_dark_theme")]
  pub dark_theme: String,
  #[serde(default)]
  pub theme_mode: ThemeMode,
  ```
  Add fns `default_light_theme() -> String { "silkcircuit-dawn".into() }` and `default_dark_theme() -> String { "silkcircuit-neon".into() }`.
- `Default for Settings`: set `light_theme: default_light_theme()`, `dark_theme: default_dark_theme()`, `theme_mode: ThemeMode::System`.
- Backward compat: old configs lacking `light_theme`/`dark_theme` get serde defaults; `theme_mode` preserved. (Old Motrix-light users will see `silkcircuit-dawn` instead — acceptable migration.)

### 4. `src/message.rs`
- Add variants: `LightThemeChanged(String)`, `DarkThemeChanged(String)`. Keep `ThemeModeChanged(ThemeMode)`.
- `SettingKey::ThemeMode` arm in app.rs is currently dead for theming (UI uses `ThemeModeChanged`); leave it.

### 5. `src/app.rs`
- `init`: `let theme = theme::build_iced(theme::effective_theme_id(&settings));`
- Add helper:
  ```rust
  fn effective_theme_id(settings: &Settings) -> &str {
      if theme::resolve_mode(settings.theme_mode, None) { &settings.dark_theme } else { &settings.light_theme }
  }
  ```
- `rebuild_theme`: `state.theme = theme::build_iced(effective_theme_id(&state.settings));`
- Handle new messages:
  - `LightThemeChanged(id)` → `state.settings.light_theme = id;` rebuild only if effective mode is light; `config::save`.
  - `DarkThemeChanged(id)` → `state.settings.dark_theme = id;` rebuild only if effective mode is dark; `config::save`.
  - `ThemeModeChanged(mode)` → existing, but `rebuild_theme` now switches between the two picked themes; `config::save`.
- "跟随系统" is evaluated at startup and on manual mode change (no live OS-theme subscription — consistent with current behavior).

### 6. `src/ui/settings_page.rs` (`general_view`)
- Replace the single Theme pick_list with three `labeled_pick` rows under a new group title `Tr::Appearance` (`外观`/`Appearance`):
  1. `Tr::LightTheme` (`浅色主题`/`Light Theme`) → `pick_list` of `theme::light_themes()` (`Labeled{value: id, label: display_name}`), selected = `settings.light_theme`, on_select `Message::LightThemeChanged(id)`.
  2. `Tr::DarkTheme` (`深色主题`/`Dark Theme`) → `theme::dark_themes()`, selected = `settings.dark_theme`, `Message::DarkThemeChanged(id)`.
  3. `Tr::ColorMode` (`明暗`/`Mode`) → existing `ThemeMode` pick_list (System/Light/Dark) with `Message::ThemeModeChanged`.
- Note: `Labeled<T>` already supports `T=String`; the `labeled_pick` helper matches by `value`. For theme picks, options = `Vec<Labeled<String>>` from the (id, display_name) tuples.
- Replace direct constant usages in this file with theme accessors:
  - `group_title` (line ~638): `.color(crate::ui::theme::ACCENT)` → pass `theme` (or an accent Color) into `group_title`; use `theme::accent(theme)`. Since `general_view` has no `theme` param, either add `theme: &iced::Theme` to `general_view` (propagate from `view`) or compute accent once in `view` and pass down. **Recommended:** add `theme: &iced::Theme` parameter to `general_view` (and pass `t` from `view`'s `general_view(fluent, theme, settings)`).
  - `advanced_view` (lines ~429–439): `crate::ui::theme::ACCENT`/`PROGRESS`/`ERROR` → `theme::accent(theme)`/`theme::success(theme)`/`theme::danger(theme)` (theme already available there).

### 7. `src/ui/task_list.rs`
- Replace constant usages with accessors (theme available as `theme: &iced::Theme` in `task_card`):
  - `theme::PROGRESS` → `theme::success(theme)` (lines 229, 231, 257).
  - `theme::PAUSED` → `theme::warning(theme)` (lines 230, 255).
  - `theme::ERROR` → `theme::danger(theme)` (lines 232, 256).
  - `theme::SPEED` → `theme::success(theme)` (line 246).
  - `theme::ACCENT` (lines 24, 42): these are in the empty-state glyph fns — ensure `theme: &iced::Theme` is available (propagate if needed) and use `theme::accent(theme)`.

### 8. `src/i18n.rs` + locale files
- `Tr` enum: remove `Theme` (now unused), add `Appearance`, `LightTheme`, `DarkTheme`, `ColorMode`. Keep `ThemeSystem`, `ThemeLight`, `ThemeDark` (reused for mode options).
- `Tr::key()`: add `appearance=>"appearance"`, `light-theme=>"light-theme"`, `dark-theme=>"dark-theme"`, `color-mode=>"color-mode"`; remove `theme`.
- `i18n/locales/en/main.ftl`:
  ```
  appearance = Appearance
  light-theme = Light Theme
  dark-theme = Dark Theme
  color-mode = Mode
  ```
  (keep `theme-system = System`, `theme-light = Light`, `theme-dark = Dark`; remove `theme = Theme`.)
- `i18n/locales/zh-CN/main.ftl`:
  ```
  appearance = 外观
  light-theme = 浅色主题
  dark-theme = 深色主题
  color-mode = 明暗
  ```
  (keep `theme-system = 跟随系统`, `theme-light = 浅色`, `theme-dark = 深色`; remove `theme = 主题`.)

## Risks & edge cases
- **Type unification:** opaline's `iced_core` 0.14 must match iced 0.14's `iced_core`. Both are `0.14` → Cargo unifies. `iced::Theme::Custom(Arc<iced_core::theme::Custom>)` accepts `to_iced_custom`'s return. Verify with `cargo build`.
- **Invalid stored theme id** (e.g. after downgrade): `build_iced` falls back to `opaline::Theme::default()` (silkcircuit-neon). Optionally clamp on load.
- **Extended palette visual change:** dropping the custom 7-level background builder means iced auto-generates background shades from opaline's 6-color palette. Sidebar/card/titlebar shades will differ from the old Motrix look but remain coherent per theme. Acceptable; verify visually.
- **`SPEED` mapping:** old SPEED was a distinct light green; mapped to `success` (green). Fine.
- **`list_available_themes()` per render:** 39 `ThemeInfo` clones on each Settings→General render — negligible. Optionally cache in app state if desired (not required).
- **No live OS theme tracking:** "跟随系统" re-evaluates only at startup / manual mode change (unchanged from current behavior).
- **clippy `no warnings`:** ensure no unused constants/imports remain after refactor (`cargo clippy --workspace`).

## Validation
1. `cargo build` — compiles; opaline + iced_core unify.
2. `cargo clippy --workspace` — no warnings.
3. `cargo fmt --check` — pass.
4. `cargo run --` — launch app:
   - Settings → General shows three dropdowns under "外观": 浅色主题, 深色主题, 明暗.
   - Light theme dropdown lists 12 light themes (e.g. Catppuccin Latte, GitHub Light, SilkCircuit Dawn); dark lists 27 (e.g. Dracula, Nord, SilkCircuit Neon).
   - Picking a light theme while 明暗=浅色 immediately applies it; picking a dark theme while 明暗=深色 applies it.
   - Switching 明暗 between 浅色/深色 swaps to the respective picked theme.
   - 明暗=跟随系统 applies dark or light pick based on OS.
   - Restart app: selections persist (`settings.json` has `light_theme`, `dark_theme`, `theme_mode`).
   - Old config (only `theme_mode`) loads without error and gets default light/dark themes.
5. Switch locale en/zh-CN: labels translate correctly.
