# Path Picker: History Tooltip & DropDown Clamping Fix (v4)

## Root Cause

iced_aw 0.14.1's `DropDownOverlay::layout()` clamping (drop_down.rs:368-381) is buggy:

```rust
// BUGGY: compares absolute coords against viewport SIZE / origin 0
if new_position.x + node.bounds().width > self.viewport.width {
    new_position.x -= node.bounds().width;
}
if new_position.x < 0.0 {
    new_position.x = 0.0;
}
```

`new_position` is in absolute screen coords (`layout.position() + translation`). When the DropDown is inside a scrollable (settings page), the scrollable passes `visible_bounds` as `viewport` — which has `viewport.x = 28` (from container padding `[24, 28]`). The overlay at `x = 228` with width `200` triggers `228 + 200 = 428 > viewport.width (400)` → clamped to `28` (viewport left edge) → **"too far to the left"**.

In the add_dialog (no scrollable, viewport starts at x=0): `428 > 1920` is false → no clamping → correct.

## Fix: Vendor DropDown with corrected clamping

### 1. Create `src/ui/components/drop_down.rs`
Vendor iced_aw 0.14.1's `drop_down.rs` (lines 1-476, excluding tests) with:

**Local `Alignment` and `Offset` types** (self-contained, no iced_aw::core dependency):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    TopStart, Top, TopEnd, End, BottomEnd, Bottom, BottomStart, Start,
}

#[derive(Debug, Clone, Copy)]
pub struct Offset { pub x: f32, pub y: f32 }
impl From<f32> for Offset {
    fn from(val: f32) -> Self { Offset { x: val, y: val } }
}
```

**Fixed clamping** (replaces lines 368-381):
```rust
// FIXED: clamp within viewport bounds using origin + size
if new_position.x + node.bounds().width > self.viewport.x + self.viewport.width {
    new_position.x = self.viewport.x + self.viewport.width - node.bounds().width;
}
if new_position.x < self.viewport.x {
    new_position.x = self.viewport.x;
}
if new_position.y + node.bounds().height > self.viewport.y + self.viewport.height {
    new_position.y = self.viewport.y + self.viewport.height - node.bounds().height;
}
if new_position.y < self.viewport.y {
    new_position.y = self.viewport.y;
}
```

**Imports** adapted to use `iced::advanced` and `iced::` top-level:
- `iced::advanced::{Clipboard, Shell, Layout, Widget, Overlay, Renderer}`
- `iced::advanced::layout::{Limits, Node}`
- `iced::advanced::widget::{Operation, Tree}`
- `iced::advanced::overlay`
- `iced::advanced::renderer`
- `iced::advanced::mouse::{self, Cursor}`
- `iced::{Element, Event, Length, Point, Rectangle, Size, Vector}`
- `iced::keyboard::{self, key::Named}`
- `iced::touch`

### 2. Update `src/ui/components/mod.rs`
Add `pub mod drop_down;`

### 3. Update `src/ui/components/path_picker.rs`
- Change `use iced_aw::widget::drop_down;` → `use super::drop_down;`
- Remove `.alignment(drop_down::Alignment::Bottom).offset(drop_down::Offset::from(0.0))` — use defaults (the fixed clamping makes `Alignment::Bottom` + `Offset::from(5.0)` work correctly in both scrollable and non-scrollable contexts)

### 4. Update `src/ui/task_list.rs`
- Change `use iced_aw::widget::drop_down;` → `use crate::ui::components::drop_down;`
- (The sort dropdown isn't affected by the bug since it's not in a scrollable, but using the fixed version is consistent)

## Validation
```bash
cargo clippy --workspace
cargo fmt --check
```
