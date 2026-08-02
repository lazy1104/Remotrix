# Plan: accent-derived surface background + wrapping swatch picker

## Goal
1. **Background follows the accent (M3-inspired)** — currently only `primary` is overridden in `build_iced`; `background` stays the fixed iced `Palette::DARK`/`LIGHT` value. Generate the background from the chosen accent using a Material-Design-style **surface** derivation: take the seed hue at low saturation, keeping the app's current brightness (dark ≈ current `#2B2D31` lightness, light ≈ near-white). iced auto-derives `background.weak/strong` + contrast text from it, so the whole UI subtly shifts hue with the theme while readability is preserved.
2. **Swatch picker wraps** — the 11 color swatches in Settings → General → Appearance currently sit in a plain `row!` inside `setting_row` (fixed 36px height), so on narrow windows they get clipped/compressed. Use iced 0.14's native `Row::wrap()` (`Wrapping` widget) with a grow-height row so swatches wrap to a new line instead.

## Decisions (confirmed with user)
- Background: **M3-style light tint** — derive from accent hue, low saturation, preserve current brightness (dark ≈ `#2B2D31` L, light ≈ near-white). Not full M3 tone 6/98, not "keep neutral".
- Text color stays iced's fixed `Palette::LIGHT.text` (black) / `Palette::DARK.text` (0.90 gray) — matches M3 `onSurface` (tone 10/90) and needs no change.
- `success`/`warning`/`danger`/`primary` behavior unchanged (`primary` = accent, already done).
- Swatch layout: **keep left-label (200px) row style**, control side wraps vertically; label vertically centered.
- iced 0.14 `Wrapping` widget verified in vendored source (`iced_widget-0.14.2/src/row.rs`): `Row::wrap()` → `Wrapping` with `vertical_spacing()`; `From<Wrapping> for Element` exists; layout uses `limits.max().width` so a bounded width (via `Length::Fill`) is required for wrapping.

## Tasks

### 1. `src/ui/theme.rs` — surface background generation
- Add HSL helpers (private):
  - `fn rgb_to_hsl(c: Color) -> (f32, f32, f32)` (hue 0..1, saturation, lightness; standard max/min formula, hue from RGB sector).
  - `fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color` (standard chroma-sector formula, modulo/rem_euclid-safe).
- Add tunable constants near `DEFAULT_THEME_COLOR`:
  - `pub const SURFACE_TONE_DARK_S: f32 = 0.12;` and `pub const SURFACE_TONE_DARK_L: f32 = 0.18;` (L≈`#2B2D31`, subtle hue tint)
  - `pub const SURFACE_TONE_LIGHT_S: f32 = 0.10;` and `pub const SURFACE_TONE_LIGHT_L: f32 = 0.96;` (near-white, perceptible-but-subtle tint)
  - Values are starting points; tune saturation up if the tint reads too faint.
- Add `fn surface_from_seed(seed: Color, dark: bool) -> Color`:
  - `let (h, _, _) = rgb_to_hsl(seed);`
  - dark → `hsl_to_rgb(h, SURFACE_TONE_DARK_S, SURFACE_TONE_DARK_L)`
  - light → `hsl_to_rgb(h, SURFACE_TONE_LIGHT_S, SURFACE_TONE_LIGHT_L)`
- Update `build_iced(color: Color, dark: bool)` (currently at `src/ui/theme.rs:72`):
  ```rust
  let palette = if dark {
      Palette { background: surface_from_seed(color, true), primary: color, ..Palette::DARK }
  } else {
      Palette { background: surface_from_seed(color, false), primary: color, ..Palette::LIGHT }
  };
  ```
- Signature `pub fn build_iced(color: Color, dark: bool) -> iced::Theme` unchanged → no call-site changes (`src/app.rs` untouched).

### 2. `src/ui/settings_page.rs` — wrapping swatch row
- Add a grow-height variant of `setting_row` (do NOT change `setting_row` used by other controls):
  ```rust
  fn setting_row_auto<'a>(label: String, control: Element<'a, Message>) -> Element<'a, Message> {
      row![]
          .push(text(label).size(FONT_MEDIUM).width(Length::Fixed(200.0)))
          .push(control)
          .align_y(Alignment::Center)
          .into()
  }
  ```
  (No `.height(Length::Fixed(36.0))` — row sizes to content so a wrapped control can grow.)
- In `theme_color_swatches` (currently `src/ui/settings_page.rs:217`):
  - Build the container as a wrapping row: `row![].spacing(SPACE_XL).width(Length::Fill).wrap().vertical_spacing(SPACE_LG)`.
  - Keep pushing each `tooltip::standard(swatch, text(*name), Position::Bottom)` child unchanged.
  - Return `setting_row_auto(fluent.get(Tr::ThemeColor), wrapping_row.into())`.
- No changes needed to swatch buttons (fixed `SWATCH_SIZE`), message enum, or app.rs.

### 3. Docs (keep `/check-docs` green)
- `README.md`: Theming bullet — mention the accent also derives the background/surface (M3-inspired) and swatches wrap.
- `AGENTS.md`: Themes row / theme convention bullet — "accent → iced palette generation (primary + M3-style surface background)".
- `.kilo/command/check-docs.md`: rule 4 wording stays accurate (accent → `Theme::custom` palette generation); optionally note background is surface-derived from accent.

## Risks / notes
- HSL surface at low saturation: for near-white light surfaces the tint is subtle — if too faint, raise `SURFACE_TONE_LIGHT_S` (constants centralized).
- `background.weak/strong` (cards, sidebar, borders) are auto-derived by iced from the new `background`, so they shift hue consistently; text contrast is preserved because `text` is unchanged and tints are light.
- `Wrapping` requires a bounded width: `.width(Length::Fill)` guarantees it. Wrapping only triggers when the control width (< window minus sidebar/category/padding) can't fit 11 swatches (~408px).
- Light-mode hover tints that assume a dark background (`button::text` uses white-alpha hover) are pre-existing and out of scope.

## Validation
1. `cargo build` + `cargo clippy --workspace` (0 warnings) + `cargo fmt --check`.
2. Run app:
   - Settings → General → Appearance: swatches wrap to a second line when the window is narrow (no clipping/compression); single line at default width.
   - Click swatches: `primary` AND `background`/sidebar/cards shift hue; check every candidate color in Dark / Light / System for readable background + text (esp. bright Lime/Cyan).
   - Restart app: `theme_color` persisted; background/primary derived correctly on startup.
3. Sanity: no leftover references to the old surface behavior; README/AGENTS/check-docs updated.
