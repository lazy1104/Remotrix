# Plan: settings label rows — vertically center within the control's first line

## Goal (revised after user feedback)
The prior change set every label row to `Alignment::Start`, which pins labels to the very
top of each row's band instead of centering them against the control's first line.
Desired behavior: **each label is vertically centered within the height of the control's
first line**, so it never jumps when a control's height changes (e.g. swatch wrapping).

## Root cause
- Fixed-height label rows are 36px tall. `Start` puts the label text at y≈0..16 with empty
  space below; `Center` centers the label within the 36px band, which is exactly "centered
  in the first line's height". So those rows were already correct before this plan and only
  need `Center` restored.
- The auto-height swatch row (`setting_row_auto`) is the real problem case: with `Center`
  the label centers against the WHOLE wrapped height, so it jumps vertically when the
  swatches wrap to a second line. It needs the label centered against the FIRST swatch line
  only, and the row anchored so the label tracks the top line.

## Decisions
- Fixed-height label rows: restore `align_y(Alignment::Center)`. Keep `.height(Length::Fixed(36.0))`.
- Swatch auto row (`setting_row_auto`): keep row `align_y(Alignment::Start)` but render the
  label inside a vertically-centered fixed-height box of `SWATCH_SIZE` (28px), so it centers
  on the first swatch line and never jumps when wrapping.
- `labeled_editor` keeps `Alignment::Start` (multiline editor) — out of scope.

## Tasks

### 1. Restore `Center` on the 6 fixed-height label rows (`src/ui/settings_page.rs`)
Change `.align_y(Alignment::Start)` → `.align_y(Alignment::Center)` at:
- `setting_row` (line 857)
- download_view path-picker row (line 278)
- ed2k_view server-list picker row (line 543)
- ed2k_view node-list picker row (line 560)
- advanced_view aria2 version row (line 709)
- `labeled_readonly` (line 985)

Keep `.height(Length::Fixed(36.0))` unchanged.

### 2. `setting_row_auto` (line 861, swatch row) — centered first-line label
Keep `.align_y(Alignment::Start)` on the row. Replace the plain label with a
vertically-centered box of `SWATCH_SIZE` height:

```rust
fn setting_row_auto<'a>(label: String, control: Element<'a, Message>) -> Element<'a, Message> {
    row![]
        .push(
            container(text(label).size(FONT_MEDIUM))
                .width(Length::Fixed(200.0))
                .height(Length::Fixed(SWATCH_SIZE))
                .center_y(),
        )
        .push(control)
        .align_y(Alignment::Start)
        .into()
}
```

`container` is already imported (line 4); `SWATCH_SIZE` comes from `use crate::ui::dims::*`.

### 3. Leave unchanged
- `labeled_editor` (line 944): already `Alignment::Start` — keep.
- Control-internal rows keep `Center`: swatch base row (line 219), MinSplitSize stepper row
  (line 314), speed stepper+unit row (line 1055).

## Notes / risks
- Only label alignment changes; no control heights/widths are touched.
- `SWATCH_SIZE` (28px) equals the swatch button height (padding 0), so a 28px centered label
  box aligns precisely with the first swatch line. If swatch buttons ever gain padding, the
  label box height must be revisited.
- Fixed rows returning to `Center` restores the pre-plan look, which was already correct;
  the wrap-jump bug only ever affected the auto swatch row.

## Validation
1. `cargo build`, `cargo clippy --workspace` (0 warnings), `cargo fmt --check`.
2. Run app → Settings → General → Appearance:
   - Narrow the window until swatches wrap: the "主题颜色" label stays vertically centered on
     the first swatch row; no vertical jump while resizing.
   - Spot-check other pages: toggles, inputs, pickers, version/read-only rows — labels
     centered within the 36px band; nothing clips or overflows.
