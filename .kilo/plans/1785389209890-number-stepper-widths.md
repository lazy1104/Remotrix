# Widen number_stepper widths in settings speed inputs and add-dialog split

## Changes

### 1. `src/ui/settings_page.rs:829`
`speed_labeled_input` uses number_stepper with `Length::Fixed(120.0)`. Bump to `Length::Fixed(160.0)` to match `labeled_number` (line 688).

```
-            Length::Fixed(120.0),
+            Length::Fixed(160.0),
```

### 2. `src/ui/add_dialog.rs:108`
Split input number_stepper uses `Length::Fixed(80.0)`. Bump to `Length::Fixed(100.0)` to comfortably display 3-digit max (128).

```
-            Length::Fixed(80.0),
+            Length::Fixed(100.0),
```

## Verification
```bash
cargo clippy --workspace
```
