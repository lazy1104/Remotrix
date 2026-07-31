# Toast / Message component (8-direction popup)

## Goal
Add a reusable toast message component that pops up in one of **8 directions** (placement positions). Each toast has a type (Normal / Warning / Error / Success) with a relevant icon and color, a fixed-width, auto-wrapping message body, and an optional close (X) icon. Auto-close timeout configurable per toast, default **3s**. Wire it into the app state so any code can show a toast.

## Design decisions (from user spec)
- **8 directions** = 8 placement positions on a full-size overlay: `TopLeft`, `Top`, `TopRight`, `Right`, `BottomRight`, `Bottom`, `BottomLeft`, `Left` (the 8 compass points; no center). Position is per-toast and configurable.
- **Types & icons/colors** (color = `theme::*` helper):
  - `Normal` → `icon::info()` colored `background.strong` (via existing `theme::border_color`)
  - `Warning` → new `icon::triangle_alert()` colored `theme::warning(t)`
  - `Error`   → new `icon::circle_x()` colored `theme::danger(t)`
  - `Success` → existing `icon::circle_check()` colored `theme::success(t)`
- **Card style**: background = `background.base`, border = `background.strong` (1px). New `theme::style::toast`.
- **Content row order**: icon, message, close icon. Message wraps automatically; card has a fixed max width. Icon is vertically top-aligned.
- **Close icon** hidden by default; shown per-toast via `show_close`.
- **Close time** configurable per toast; default 3s. `None`/0 → no auto-close.

## Icons to add (`fonts/icons.toml`)
Add two entries (they regenerate `src/ui/icon.rs` at build time via `build.rs` — do NOT edit `icon.rs` by hand):
```toml
triangle_alert = "triangle-alert"
circle_x       = "circle-x"
```
(`info` and `circle_check` already exist; `x` already exists for the close glyph.) Requires a normal `cargo build` to regenerate the font subset.

## Implementation

### 1. New file `src/ui/components/toast.rs` (+ register in `components/mod.rs`)
`pub mod toast;` in `src/ui/components/mod.rs`.

```rust
pub enum ToastKind { Normal, Warning, Error, Success }   // Default: Normal

pub enum ToastPosition { TopLeft, Top, TopRight, Right,
                         BottomRight, Bottom, BottomLeft, Left }  // Default: BottomRight

pub struct Toast {
    pub id: u64,                 // assigned by app when pushed
    pub kind: ToastKind,
    pub message: String,
    pub position: ToastPosition,
    pub show_close: bool,        // default false
    pub close_after: Option<Duration>,  // default Some(Duration::from_secs(3)); None => sticky
}
```
`impl Toast { pub fn new(kind, message) -> Self }` with defaults, plus builder methods `.position(..)`, `.show_close()`, `.close_after(Option<Duration>)`.

**`view(theme, toasts: &[Toast]) -> Element<'a, Message>`**
Builds one full-size `stack`/container overlay. For each of the 8 positions, if any toast has that position, build a child full-size `container` aligned to that position (mirrors existing `hud_overlay` pattern in `app.rs:1196-1206`) with padding (e.g. 16px), containing a `column` of toast cards spaced 8px. Position→alignment:
- TopLeft: Left/Top; Top: Center/Top; TopRight: Right/Top
- Right: Right/Center; Left: Left/Center
- BottomLeft: Left/Bottom; Bottom: Center/Bottom; BottomRight: Right/Bottom

Each toast card:
```rust
row![icon_col, message_col]
    .push_maybe_close()           // close button only if toast.show_close
    .align_y(Vertical::Top)
```
wrapped in `container(...).width(Length::Fixed(320.0)).padding(...).style(theme::style::toast)`.
- `icon_col`: `container(icon).align_y(Vertical::Top)` (top-aligned). Icon `.size(16)`.
- `message_col`: `text(message).width(Length::Fill)` so long text wraps within the 320px card. Wrapping is **already `Word` by default** in iced 0.14 (`iced_core::text::Wrapping::Word` is `#[default]`), so no explicit `.wrapping()` call is needed — a bounded width (`Width::Fill` inside the fixed 320px card) is what triggers wrapping. If explicit is desired, the correct path is `iced::widget::text::Wrapping::Word` (there is no top-level `iced::text::Wrapping` re-export). NOTE: iced 0.14 `Column`/`row` have no `push_maybe` — conditionally build (`if toast.show_close { row = row.push(btn) }`).
- Close button: `button(icon::x().size(14)).on_press(Message::DismissToast(toast.id)).padding(2).style(theme::style::button::text())`.

Return the overlay element directly (no dismiss behavior inside the component beyond emitting the `Message`).

### 2. `src/ui/theme.rs`
Add:
```rust
pub fn toast(t: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(t.extended_palette().background.base.color.into()),
        border: iced::Border {
            color: t.extended_palette().background.strong.color,
            width: 1.0,
            radius: super::RADIUS_BUTTON.into(),
        },
        ..Default::default()
    }
}
```
(helpers `warning`/`success`/`danger` already exist at `theme.rs:68-78`; `border_color` returns `background.strong`.)

### 3. `src/message.rs`
Add variants (import the toast types):
```rust
ShowToast(crate::ui::components::toast::Toast),
DismissToast(u64),
```

### 4. `src/app.rs`
- Add fields to `Remotrix`: `toasts: Vec<crate::ui::components::toast::Toast>`, `next_toast_id: u64`. Init in `init()`: `toasts: Vec::new()`, `next_toast_id: 0`.
- `update()`:
  - `Message::ShowToast(mut t)` → assign `t.id = state.next_toast_id; state.next_toast_id += 1;` push into `state.toasts`. If `close_after` is `Some(d)`, `return Task::perform(async move { tokio::time::sleep(d).await }, move |_| Message::DismissToast(id))` (same `Task::perform` pattern as `pick_path`, `app.rs:1381`; tokio runtime is available). Cap per-position length at e.g. 6 (drop oldest) to avoid overflow.
  - `Message::DismissToast(id)` → `state.toasts.retain(|t| t.id != id)`.
- `view()`: after the confirm-dialog block (`app.rs:1263`), if `!state.toasts.is_empty()`, push the toast overlay last so it sits on top:
  ```rust
  if !state.toasts.is_empty() {
      stacked = stack![stacked, crate::ui::components::toast::view(t, &state.toasts)]
          .width(Length::Fill).height(Length::Fill).into();
  }
  ```

## Usage (API for callers)
```rust
Message::ShowToast(
    Toast::new(ToastKind::Success, "Done")
        .position(ToastPosition::TopRight)
        .show_close(),
)  // default 3s auto-close
```
Callers can override with `.close_after(Some(Duration::from_secs(5)))` or `.close_after(None)` for sticky. No existing toast triggers required by this task; the API is the deliverable.

## Validation
- `cargo build` (regenerates icon.rs + font subset).
- `cargo clippy --workspace` — no warnings.
- `cargo fmt --check`.
- Manual: trigger a toast for each kind/position; verify icons+colors, top-aligned icon, 320px wrap, close X shows only when `.show_close()`, and 3s auto-dismiss (and `None` stays).

## Risks / notes
- `icon.rs` is generated; add icons only via `fonts/icons.toml` and rebuild. Verify `triangle-alert` / `circle-x` are valid Lucide names (confirmed present in `iced_lucide` 0.1.0 `unicode.html`).
- `row`/`column` in iced 0.14 have no `push_maybe`; use `if` guards.
- Toasts are intentionally layered above all dialogs for visibility.
- Default position `BottomRight` and card width `320.0` are configurable constants; adjust if desired.
