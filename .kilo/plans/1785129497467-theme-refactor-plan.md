# Theme refactor: eliminate per-UI `dark: bool` judgment

## Goal
Stop threading `dark: bool` into every `view` fn and repeating `if dark { X } else { Y }` in each UI file. Make `&iced::Theme` the single source of colors, the way the official `iced/examples/styling` and the [iced guide](https://jl710.github.io/iced-guide/themes_and_styling/) prescribe: style functions read `theme.extended_palette()`; the theme is global.

## Root cause
`iced::Theme` (0.14) `Palette` has only 6 fields. The codebase defines ~12 semantic colors (`BG_SIDEBAR`, `BG_CARD`, `TEXT_SECONDARY`, `BORDER`, `TITLE_BAR`, `OVERLAY`, `SPEED`, …), so each UI file re-resolves them from a `dark: bool` param. The resolution is scattered across 7 files instead of centralized.

## Decision (resolved)
Adopt the **idiomatic iced 0.14 approach** (not a custom `Theme` type — that would require implementing `Base` + per-widget `Catalog` traits and reparameterizing every `Element`/`Text`; not worth it here):

- Keep `iced::Theme` as the application/widget theme type. `Element<'_, Message>` stays unchanged.
- Build it with `iced::Theme::custom_with_fn(name, palette, generate)` so our **exact** brand shades are injected into `extended_palette().background` (`base/weak/strong/stronger` = `BG_PRIMARY/BG_CARD/BG_SIDEBAR/TITLE_BAR`), overriding the auto-`deviate` shades. `is_dark` is set from the resolved mode.
- Cache the resolved `iced::Theme` once in `Remotrix` state; rebuild only on theme-mode change.
- Pass `&iced::Theme` into `view` fns (replaces `dark: bool`). Views read `theme.extended_palette()` for concrete colors and call centralized style fns in `ui/theme.rs`.
- Centralize the 2 colors with no natural palette slot (`TEXT_SECONDARY`, `BORDER`) as helper fns in `theme.rs` that branch on `pal.is_dark`. Brand constants that don't vary by mode (`ACCENT`, `PROGRESS`, `SPEED`, `ERROR`, `PAUSED`, `OVERLAY`) stay `const`.
- Result: `grep dark` in `src/ui/*.rs` returns zero hits (except the `theme.rs` helpers themselves).

## Out of scope
- **OS theme reactivity for `System` mode.** Current code calls `dark_light::detect()` only at init / on manual change; it does not react to OS theme flips at runtime. This refactor preserves that behavior. Adding a polling `Subscription` for OS theme changes is a separate follow-up.
- No persisted settings schema change (`ThemeMode` enum unchanged).

## State model changes (`src/app.rs`)
- Replace field `dark: bool` with `theme: iced::Theme`.
- `init()`: `let theme = theme::build_iced(theme::resolve_mode(settings.theme_mode, None));` store it.
- `app::theme(state) -> iced::Theme` returns `state.theme.clone()` (was `theme::build(state.dark)`).
- `app::view`: pass `&state.theme` to every sub-view instead of `state.dark`; drop the local `bg_primary = if state.dark {…}` block — base container uses `theme::style::base_background`.
- Update sites that must rebuild `state.theme` (same places `state.dark` was set today):
  - `Message::ThemeModeChanged(mode)` (app.rs ~439)
  - `Message::SettingChanged(SettingKey::ThemeMode, …)` (app.rs ~259-266)
  - `init()` (app.rs ~60)
  - Helper: `fn rebuild_theme(state)` -> `state.theme = theme::build_iced(theme::resolve_mode(state.settings.theme_mode, None));`

## `src/ui/theme.rs` — new shape
Keep `ThemeMode`, `detect_dark`, `resolve_dark` (rename internal use to `resolve_mode` returning a new `Mode` enum if helpful, or keep `bool`). Add:

```rust
pub fn build_iced(mode: Mode) -> iced::Theme {
    iced::Theme::custom_with_fn(
        if mode.is_dark() { "Remotrix Dark" } else { "Remotrix Light" },
        seed_palette(mode),            // 6-field Palette for primary/success/...
        move |_| extended_palette(mode), // hand-built Extended w/ exact bg shades
    )
}
```
- `extended_palette(mode)`: start from `palette::Extended::generate(seed_palette(mode))`, then overwrite `.background` with exact `Background { base: Pair{BG_PRIMARY, TEXT_PRIMARY/​_LIGHT}, weak: Pair{BG_CARD,…}, strong: Pair{BG_SIDEBAR,…}, stronger: Pair{TITLE_BAR,…}, … }` (fill remaining shades by interpolating/repeating). Set `.is_dark = mode.is_dark()`. Construct `Pair` with raw fields to bypass `readable()`.
- Keep `seed_palette(mode)` = current `palette()` -> `Palette { background, text, primary: ACCENT, success: PROGRESS, warning: PAUSED, danger: ERROR }`.

Color accessors (centralized; branch lives HERE only):
```rust
pub fn text_secondary(t: &iced::Theme) -> Color { /* if pal.is_dark TEXT_SECONDARY else _LIGHT */ }
pub fn border_color(t: &iced::Theme) -> Color   { /* if pal.is_dark BORDER else _LIGHT */ }
```
Brand constants `ACCENT/PROGRESS/SPEED/ERROR/PAUSED/OVERLAY` stay `pub const`. Remove the now-unused `BG_PRIMARY_LIGHT`/`TEXT_PRIMARY_LIGHT`/etc. free-floating dual constants only if no longer referenced (they move into `extended_palette`/accessors); keep them `pub(crate)` as the single definition source.

### Centralized style catalog (new submodules in `theme.rs`, mirroring `button::primary` organization)
Each is a `fn(&iced::Theme[, Status]) -> <Widget>::Style` (or a unit-struct class) reading `t.extended_palette()`:

| Style fn | Used by | Source of color |
|---|---|---|
| `style::base_background` | app base container | `pal.background.base.color` |
| `style::sidebar_background` | sidebar container, title-bar left seg | `pal.background.strong.color` |
| `style::category_background` | category_bar container, title-bar mid seg | `pal.background.weak.color` |
| `style::card` | task card, dialog panels | `pal.background.weak.color` + rounded(8/12) |
| `style::overlay` | dialog backdrops | `OVERLAY` (const) |
| `style::active_filter` | active category/filter highlight | ACCENT @0.18 |
| `button::sidebar_icon(active)` | sidebar nav buttons | ACCENT @0.25 when active, hover rgba(1,1,1,0.08) else |
| `button::window_control(close: bool)` | title-bar min/max/close | hover rgba(1,1,1,0.12) / close_hover |
| `progress::task(bar: Color)` | task progress bar | track=`pal.background.base.color`, bar=captured `bar` |
| `text::secondary` | muted text | `text_secondary(t)` |

Use built-in `button::primary/secondary/danger/text` and `container::rounded_box` where they already match (they will, once `custom_with_fn` injects our palette).

## Per-file UI changes (drop `dark: bool` param, drop all `if dark` blocks)
- `ui/sidebar.rs`: `view(fluent, theme: &iced::Theme, page, logo)`; resolve `bg_sidebar`/`text_primary` from `theme.extended_palette()`; sidebar buttons use `button::sidebar_icon(active)`.
- `ui/title_bar.rs`: `view(theme: &iced::Theme, maximized)`; three segments use `sidebar_background`/`category_background`/`base_background`; window buttons use `button::window_control`.
- `ui/category_bar.rs`: `view(fluent, theme, page, …)`; `bg_card`/`text_primary` from palette; active filter uses `style::active_filter`.
- `ui/task_list.rs`: `view(fluent, theme, tasks, sort_state)`; `text_primary`/`text_secondary`/`bg_card`/progress track from palette; secondary text via `.style(theme::text::secondary)` or `.color(theme::text_secondary(theme))`; `task_card` drops its 4 color params, takes `theme: &iced::Theme` instead.
- `ui/settings_page.rs`: `view(fluent, theme, …)`; same accessor pattern.
- `ui/add_dialog.rs`, `ui/about_dialog.rs`, `ui/close_dialog.rs`: `view(fluent, theme, …)`; panel uses `style::card`, backdrop uses `style::overlay`.
- `ui/icons.rs`: `icon_text`/`icon_small` keep `iced::Theme` type params unchanged (no reparameterization).

## Migration / safety
- No persisted-config change. Existing `settings.json` loads unchanged.
- Visual output should be byte-equivalent to today (same hex colors), since `custom_with_fn` injects the exact same RGB values currently hard-coded.
- `app::theme()` still returns `iced::Theme`; `main.rs` `.theme(...)` wiring unchanged.

## Validation
1. `cargo fmt --check`
2. `cargo clippy --workspace` (must be warning-free per AGENTS.md)
3. `cargo build` (offline OK; build.rs only generates icons)
4. `cargo run --` and manually:
   - Toggle Settings → Theme: Dark / Light / System; verify sidebar, title bar, cards, dialogs, progress bars, inputs, radios, combo_box, scrollable all switch correctly.
   - Open Add/About/Close dialogs in both modes; verify overlay + card backgrounds.
   - Confirm no `dark` references remain in `src/ui/*.rs` except `theme.rs`: `rg '\bdark\b' src/ui` should only match `theme.rs`.
5. Spot-check a task card in light vs dark: progress track, bar color (active/paused/error), status text color.

## Risks
- `custom_with_fn` requires building a complete `Extended` (all `Background` shades + swatches). Missing/odd shades could make a built-in style (e.g. `button::secondary`) look off. Mitigation: start from `Extended::generate(seed)` and only overwrite `.background` + `.is_dark`; keep swatches standard-derived.
- Default `text()` color (class `Default` -> `color: None`, inherited) must render as palette text. If it doesn't, add explicit `theme::text::primary` style fn and apply where primary text is shown. Verify visually.
- `Pair` constructed with raw fields bypasses `readable()`; our chosen text colors are already readable on the backgrounds, so this is intentional and safe.
