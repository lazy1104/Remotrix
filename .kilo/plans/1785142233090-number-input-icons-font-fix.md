# Fix: NumberInput increase/decrease icons not displaying

## Symptom
In Settings, the `iced_aw::NumberInput` widgets show no +/- (increase/decrease) icon
buttons on the right side. Used in Download, BitTorrent, and Network settings pages
(`src/ui/settings_page.rs`: `labeled_number` -> `iced_aw::NumberInput::new`, 13 call sites).

## Root cause
`iced_aw 0.14.1` renders the NumberInput +/- buttons as glyph text using its embedded
icon font, identified by name `"iced_aw"`:

- `iced_aw::ICED_AW_FONT: Font = Font::with_name("iced_aw")` (lib.rs:176)
- `iced_aw::ICED_AW_FONT_BYTES: &[u8] = include_bytes!("../font.ttf")` (lib.rs:174)
- `number_input.rs:1144/1181` call `down_open()` / `up_open()` from the generated
  `iced_aw_font::advanced_text` module, which return `ICED_AW_FONT` as the glyph font.

The glyph only renders if that font is **registered** with the iced application via
`.font(...)`. In `src/main.rs:16-32` the app registers only:
- `crate::ui::icon::FONT` (lucide.ttf, name `"lucide"`)
- `assets/fonts/HarmonyOS_Sans_SC_Regular.ttf`

The `iced_aw` font is **not** registered, so `renderer.fill_text` with
`Font::with_name("iced_aw")` finds no glyphs -> icons render as nothing. This matches
the reported "右边增减图标不显示".

Icon color is NOT the cause: the default NumberInput class is `primary`
(`style/number_input.rs:42-44`), which sets `icon_color = palette.primary.strong.text`
and `button_background = palette.primary.strong.color` (auto-contrasted in the Remotrix
custom theme where `primary = ACCENT`). Once the font loads, icons will be visible in
both dark and light themes.

## Fix (single change)
In `src/main.rs`, register the iced_aw font on the application builder by adding one
`.font(...)` call alongside the existing font registrations (after line 23):

```rust
.font(iced_aw::ICED_AW_FONT_BYTES)
```

`iced_aw::ICED_AW_FONT_BYTES` is a public `&'static [u8]` already embedded in the
`iced_aw` crate (no new dependency, no asset file needed). The `iced_aw` dependency is
already declared in `Cargo.toml:30` with the `number_input` feature, so the symbol is
available.

No other files need changes.

## Verification
1. `cargo build` (offline OK; no build-time network needed)
2. `cargo clippy --workspace` (must be warning-free per AGENTS.md)
3. `cargo fmt --check`
4. Run `cargo run --` and open Settings:
   - **Download** page (e.g. Max connections, Split, Speed limits) — +/- icons visible
     and clickable on each number input.
   - **BitTorrent** page — +/- icons visible.
   - **Network** page — +/- icons visible.
5. Toggle Dark/Light theme; confirm icons remain visible (contrast should be adequate
   via `palette.primary.strong.text`).

## Follow-up (out of scope unless verification fails)
If icon contrast is poor in either theme after the font fix, add a custom
`number_input::StyleSheet` style using `theme::TEXT_PRIMARY`/`TEXT_PRIMARY_LIGHT` for
`icon_color`. Not needed unless step 5 shows a problem.

## Risk
- Negligible. Adding a registered font cannot regress other widgets; the lucide and
  HarmonyOS fonts are unaffected. The font is tiny ("a handful of glyphs" per iced_aw).
