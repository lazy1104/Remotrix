# Plan: Scroll the entire New Download dialog body

## Goal
Change `src/ui/add_dialog.rs` so the **entire dialog body** is wrapped in a scrollable, instead of only the advanced-options form being scrollable inside a 230px box.

Currently (add_dialog.rs:195-201) only the advanced form is wrapped:
```rust
if state.advanced_open {
    body_items.push(
        slim_scrollable(advanced_form(fluent, state))
            .height(Length::Fixed(230.0))
            .into(),
    );
}
let body = column(body_items).spacing(14).width(Length::Fill);
```

## Confirmed decision
- **Fixed max height** for the whole-body scrollable (~400px). The dialog always reserves this body height; when the advanced form is collapsed there is some empty scroll area, when expanded the body scrolls. Predictable and keeps the dialog within the default 720px window (title + footer + 2×28px padding leave ~528px usable; 400px is safe).

## Why a fixed height
- iced `Length` has no "shrink to content but cap at max". A `scrollable` must have a bounded height to actually scroll.
- The `Dialog` component sizes to content (`container(inner)` without a height constraint), so the scrollable needs an explicit `height`.

## Changes (single file: `src/ui/add_dialog.rs`)

### In `view()` (the `body` construction)
1. Remove the inner `slim_scrollable` wrapping of the advanced form. When `state.advanced_open`, push `advanced_form(fluent, state).into()` directly into `body_items` (no height wrapper).
2. Wrap the whole body column in `slim_scrollable` with a fixed height:
   ```rust
   let body = slim_scrollable(column(body_items).spacing(14).width(Length::Fill))
       .height(Length::Fixed(400.0))
       .into();
   ```
   - `slim_scrollable` is already imported (add_dialog.rs:12) — no import change.
   - `Dialog::body(body)` accepts `impl Into<Element>`, and `Scrollable` implements `Into<Element>`, so passing the scrollable element directly works unchanged.

3. Keep everything else (rename row, advanced checkbox, `Dialog` layout, `advanced_form`/`advanced_field` helpers) unchanged.

## Validation
1. `cargo fmt --check`
2. `cargo clippy --workspace` (no warnings)
3. `cargo build` (offline, must succeed)
4. Manual (runtime): open New Download — body is ~400px with a scrollable region; toggle Advanced → form expands within the same body area and scrolls; collapsed → some empty space below content. Dialog still fits the window.

## Risks / notes
- Fixed body height means the dialog does not shrink when advanced is collapsed — expected, per confirmed decision.
- If 400px feels too tall/short, adjust the single constant; no other layout logic depends on it.
