# Fix: NumberStepper subtract/overflow panic

## Root Cause
Two guard conditions in `build_row` (`src/ui/components/number_stepper.rs`) allow out-of-range arithmetic, causing panics for unsigned types like `u16`/`u32`/`u64`:

- **Line 152**: `if *value > min { *value - step }` — if `*value = 1`, `min = 0`, `step = 2`, the check passes but `1 - 2` underflows.
- **Line 165**: `if *value < max { *value + step }` — if `*value` is close to `T::MAX`, `*value + step` overflows.

## Fix
1. Change minus guard to `*value >= min + step` (guarantees `*value - step >= min`, no underflow).
2. Change plus guard to `*value <= max - step` (guarantees `*value + step <= max`, no overflow).

## Files to edit
- `src/ui/components/number_stepper.rs`

## Verification
```bash
cargo build
cargo clippy --workspace
```
