# Plan: Remove opaline — single accent-color theme with swatch picker

## Goal
Remove the `opaline` dependency and its two theme dropdowns (浅色主题 / 深色主题). Replace them with **one accent-color picker** (a row of circular color swatches) in Settings → General → Appearance. The selected accent is the `primary` color for **both** light and dark modes; iced auto-generates the full palette (`background`, `text`, `secondary`, `success`, `warning`, `danger`, weak/strong variants) from it. The existing 明暗模式 dropdown (Dark / Light / System) stays.

## Decisions (confirmed with user)
- Keep `ThemeMode` (Dark/Light/System) and `dark-light` system detection.
- Single `theme_color` applies to both modes; `build_iced` builds a light or dark base `Palette` depending on resolved mode, overriding only `primary` with the chosen color.
- Store color as hex `String` (`#RRGGBB`) in `settings.json` (iced `Color` has no serde impl enabled; hex string avoids adding the `serde` feature).
- Old configs with `light_theme`/`dark_theme` are ignored by serde (missing-field defaults) → default accent blue is used. No migration mapping old opaline ids.

## iced 0.14 API (verified in vendored source)
- `iced::Theme::custom(name: impl Into<Cow<'static,str>>, palette: Palette) -> Self` auto-generates `palette::Extended` (primary/secondary/success/warning/danger + weak/strong) — this is the "自动生成主题配置" the user wants.
- `iced::theme::Palette { background, text, primary, success, warning, danger }` (all pub).
- `Palette::LIGHT` / `Palette::DARK` consts — build custom palette via struct update: `Palette { primary: color, ..Palette::DARK }`.
- `Color` is `Copy + Clone + PartialEq`; `Color::from_rgb8(u8,u8,u8)` for hex parsing.
- `crate::ui::icon::circle_check()` (lucide `\u{E226}`) exists for the selected-swatch marker; button `text_color` from style colors it.

## Tasks (ordered)

### 1. `Cargo.toml`
- Remove the `opaline = { ... }` dependency line.
- `cargo build` to refresh `Cargo.lock` (offline-safe; opaline is the only removed crate).

### 2. `src/ui/theme.rs`
- Remove `use std::sync::Arc;` (no longer needed) and all `opaline::*` imports/usages.
- Replace `build_iced(theme_id: &str)` with:
  ```rust
  pub fn build_iced(color: Color, dark: bool) -> iced::Theme {
      let palette = if dark {
          Palette { primary: color, ..Palette::DARK }
      } else {
          Palette { primary: color, ..Palette::LIGHT }
      };
      iced::Theme::custom("remotrix", palette)
  }
  ```
  (`use iced::{palette::Palette, Color, Theme};` — `Palette` re-exported as `iced::theme::Palette`.)
- Delete `themes_for_variant`, `light_themes()`, `dark_themes()`.
- Add constants/helpers:
  - `pub const DEFAULT_THEME_COLOR: Color = Color::from_rgb8(0x58, 0x65, 0xF2);`
  - `pub fn color_to_hex(c: Color) -> String` — `#RRGGBB`, `(v.clamp(0.0,1.0)*255.0).round()` per channel.
  - `pub fn color_from_hex(s: &str) -> Option<Color>` — parse `#rrggbb` → `Color::from_rgb8`.
  - `pub fn candidate_colors() -> &'static [(Color, &'static str)]` — ~10 accents with display names (e.g. Blue `#5865F2`, Indigo `#6366F1`, Purple `#A855F7`, Pink `#EC4899`, Red `#EF4444`, Orange `#F97316`, Amber `#F59E0B`, Lime `#84CC16`, Green `#22C55E`, Teal `#14B8A6`, Cyan `#0EA5E9`).
- In `pub mod style` → `pub mod button`, add:
  ```rust
  pub fn swatch<'a>(color: Color, selected: bool) -> impl Fn(&iced::Theme, Status) -> Style + 'a
  ```
  - background = the swatch color (hover: `lighten`, pressed: `darken` — reuse existing helpers).
  - circular border radius `SWATCH_SIZE/2`.
  - unselected border: neutral gray (`Color::from_rgba(0.5,0.5,0.5,0.6)`); selected border: `t.extended_palette().background.base.text`, width 2.0.
  - `text_color` (check marker): contrast by luminance — `0.299r+0.587g+0.114b > 0.55` → dark `#111`, else white; `Color::TRANSPARENT` when not selected.
- Keep `ThemeMode`, `detect_dark`, `resolve_mode`, all `extended_palette` accessors, and all existing widget styles unchanged.

### 3. `src/dims.rs`
- Add `pub const SWATCH_SIZE: f32 = 28.0;`.

### 4. `src/config.rs`
- Remove `default_light_theme()`, `default_dark_theme()`.
- Add `fn default_theme_color() -> String { crate::ui::theme::color_to_hex(crate::ui::theme::DEFAULT_THEME_COLOR) }`.
- In `Settings`:
  - Remove `light_theme: String`, `dark_theme: String` (fields + serde defaults + `Default` impl).
  - Add `#[serde(default = "default_theme_color")] pub theme_color: String,`.
- (config.rs already imports `crate::ui::theme::ThemeMode`; add the two helpers to that import.)

### 5. `src/message.rs`
- Replace `LightThemeChanged(String)` and `DarkThemeChanged(String)` with `ThemeColorChanged(iced::Color)`.

### 6. `src/app.rs`
- Replace `effective_theme_id(settings)` with a helper that returns the parsed accent:
  ```rust
  fn settings_accent(settings: &Settings) -> Color {
      theme::color_from_hex(&settings.theme_color).unwrap_or(theme::DEFAULT_THEME_COLOR)
  }
  ```
- Initial build (line ~102): `theme::build_iced(settings_accent(&settings), theme::resolve_mode(settings.theme_mode, None))`.
- `rebuild_theme`: `let dark = theme::resolve_mode(state.settings.theme_mode, None); state.theme = theme::build_iced(settings_accent(&state.settings), dark);`
- Replace the two handlers with:
  ```rust
  Message::ThemeColorChanged(color) => {
      state.settings.theme_color = theme::color_to_hex(color);
      rebuild_theme(state);
      config::save(&state.settings);
  }
  ```
- `SettingKey::ThemeMode` handler and `Message::ThemeModeChanged` stay as-is (both already call `rebuild_theme`).

### 7. `src/ui/settings_page.rs`
- In `general_view`: delete the `light_opts`/`dark_opts` block and the two `labeled_pick(...)` for Light/Dark theme.
- Insert a `theme_color_swatches(fluent, theme, settings)` row between the Appearance group title and the ColorMode pick:
  - `setting_row(fluent.get(Tr::ThemeColor), row-of-swatches)`.
  - For each `(color, name)` in `theme::candidate_colors()`: a `button` with child `icon::circle_check().size(FONT_ICON)` when `*color == current` else empty `text("")`, fixed `SWATCH_SIZE` width/height, padding 0, style `theme::style::button::swatch(*color, selected)`, `on_press(Message::ThemeColorChanged(*color))`.
  - `current = theme::color_from_hex(&settings.theme_color).unwrap_or(theme::DEFAULT_THEME_COLOR)`.
  - Row spacing `SPACE_XL`; wrap each swatch in `components::tooltip::standard(swatch, text(name), Position::Bottom)` (optional but cheap).
- Keep the ColorMode `labeled_pick`.

### 8. `src/i18n.rs` + locale files
- `i18n.rs`: replace `LightTheme`, `DarkTheme` enum variants with `ThemeColor`; mapping `Tr::ThemeColor => "theme-color"`.
- `i18n/locales/en/main.ftl`: delete `light-theme`, `dark-theme`; add `theme-color = Theme Color`.
- `i18n/locales/zh-CN/main.ftl`: delete `light-theme`, `dark-theme`; add `theme-color = 主题颜色`.

### 9. Docs (keep `/check-docs` green)
- `README.md`: Theming bullet + dependency table row + module-tree comment for `theme.rs` + feature checklist line — replace "opaline builtin themes" with "single accent color → iced palette generation".
- `AGENTS.md`: update the Themes row in the component table, `Cargo.toml` dependency snippet (remove opaline), and the Code Conventions Theme bullet.
- `.kilo/command/check-docs.md`: update rule 4 wording (opaline → iced `Theme::custom` palette generation from accent color).

## Risks / notes
- Old `settings.json` `light_theme`/`dark_theme` values silently dropped → default blue; acceptable, no migration.
- `Color` equality for the "selected" highlight uses exact float match — safe because both settings round-trip and candidates are built with the same `from_rgb8` conversion.
- iced `extended_palette()` adapts weak/strong/secondary automatically per light/dark base, so the same accent reads correctly in both modes.
- `dark-light` stays for System mode; no OS-change subscription exists today, so behavior (resolve at startup/mode-change) is unchanged.

## Validation
1. `cargo build` — removes opaline, updates lock; no network needed.
2. `cargo clippy --workspace` and `cargo fmt --check` — clean.
3. Run the app:
   - Settings → General shows a row of color circles; clicking one immediately changes the accent; selected circle shows a check + ring.
   - Switch 明暗模式 Dark / Light / System — same accent renders correctly (readable text/background in both).
   - Restart app — accent persisted in `settings.json` (`theme_color: "#...hex"`).
   - Verify no leftover opaline strings in code/docs (grep `opaline`).
