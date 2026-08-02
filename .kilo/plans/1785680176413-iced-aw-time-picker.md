# Time Picker: Switch to iced_aw + Fix Trigger Layout

## Goal
Replace the custom antd-style time picker panel (`.kilo/plans/1785676058861-antd-style-time-picker.md`, superseded) with **iced_aw 0.14's `time_picker` component**, and fix the trigger so the text + clock icon are **vertically centered** and the trigger **shrinks to its content** (text + gap + icon only).

## Confirmed decisions
- After clicking the trigger, show iced_aw's analog-clock time picker (clock face + digital display + icon Cancel/OK buttons) centered on the trigger, in 24h mode, no seconds.
- Trigger: rounded-rect button, `Length::Shrink` width, row content vertically centered.
- Commit/close: OK → `on_submit` produces `Message::SettingChanged(ScheduleStart|ScheduleEnd, "HH:MM")`; the app **closes the picker** in the `SettingChanged` match arm. Cancel button → `on_toggle` (existing toggle message) closes.
- iced_aw does **not** dismiss on Escape or outside click — accepted (Cancel/OK only). Clicking the trigger again still toggles closed (underlay `on_press`).
- Revert the `Tr::TimeNow`/`Tr::TimeOk` i18n additions (iced_aw's buttons are icon glyphs, not text). Keep the `clock` icon in `fonts/icons.toml` (used by the trigger).
- Drop the `width: Length` param (no longer used with a shrink trigger).

## Key technical facts
- **Vertical-centering root cause**: `iced_widget::button::Button::layout` → `layout::padded` → `positioned(..., |content, _| content)`. Content is NOT repositioned, so a Shrink-height row inside `height(Length::Fixed(CONTROL_HEIGHT))` button is top-aligned. Fix: give the row `.height(Length::Fill)` + `.align_y(Alignment::Center)` so it fills the button's inner height and centers the text/icon.
- iced_aw `time_picker` feature enables `chrono` + `iced_widget/canvas` (already satisfied via our `iced` canvas feature). iced_aw `TimePicker::new(show_picker, time: impl Into<Time>, underlay, on_cancel: Message, on_submit: F)` with `F: 'static + Fn(Time) -> Message`; `Message: 'static + Clone`. Our `crate::message::Message` is `'static + Clone` — OK.
- `iced_aw::time_picker::Time` is an enum (`Hm { hour: u32, minute: u32, period }` / `Hms`); `Display` prints `"{:02}:{:02}"` for `Period::H24`. `.use_24h()` guarantees `Period::H24` on submit. No `FromStr`; build from `parse_hhmm` (src/scheduler.rs:33).
- iced_aw registers its widget state via `Tag::of::<State>()` / `state()`, seeded **once** from the `Time` passed at construction. The widget tree persists across frames → reopening after a commit would show the stale pre-commit time. Fix with a small wrapper widget that re-seeds `tree.children[0].state = tree::State::new(State::new(time, true, false))` on the open transition.
- `iced::Renderer` and `iced_widget::Renderer` are the same type (`iced_renderer::Renderer`), so iced_aw widgets drop into `Element<'a, M, iced::Theme, iced::Renderer>`.
- `iced::Theme` implements iced_aw's `time_picker::Catalog` (default `primary` style is palette-aware) plus iced_widget `button/text/container::Catalog` — no custom styling needed.
- The iced_aw overlay font (`cancel`/`ok` glyphs) requires `.font(iced_aw::ICED_AW_FONT_BYTES)` in main.rs.

## Files touched
1. `Cargo.toml` — add `iced_aw = { version = "0.14", default-features = false, features = ["time_picker"] }`.
2. `src/main.rs` — register font: `.font(iced_aw::ICED_AW_FONT_BYTES)`.
3. `src/ui/components/time_picker.rs` — full rewrite: fixed trigger + iced_aw wrapper.
4. `src/ui/settings_page.rs` — revert both call sites to `(value, open, on_toggle, on_change)`.
5. `src/app.rs` — close the corresponding picker in the `SettingChanged(ScheduleStart|ScheduleEnd)` arms.
6. `src/i18n.rs` + `i18n/locales/en/main.ftl` + `i18n/locales/zh-CN/main.ftl` — remove `Tr::TimeNow`/`Tr::TimeOk` and `time-now`/`time-ok`.

## Implementation steps

### 1. Cargo.toml
```toml
iced_aw = { version = "0.14", default-features = false, features = ["time_picker"] }
```

### 2. main.rs
Add after the existing `.font(...)` lines:
```rust
.font(iced_aw::ICED_AW_FONT_BYTES)
```

### 3. Rewrite `src/ui/components/time_picker.rs`
Keep `picker_button()` (existing rounded style, `snap: false`). Remove the custom panel/columns/overlay code entirely.

Public entry — drops the label + width params:
```rust
pub fn time_picker<'a, M>(
    value: &'a str,
    open: bool,
    on_toggle: M,
    on_change: impl Fn(String) -> M + 'static,
) -> Element<'a, M, iced::Theme, iced::Renderer>
where
    M: 'static + Clone,
```

Trigger (fixed centering + shrink width — the two changes from the current version are `.height(Length::Fill)` on the row, and removing both `.width(Length::Fill)` from the button and the text):
```rust
let underlay = button(
    row![
        iced::widget::text(value).size(FONT_MEDIUM),
        icon::clock().size(FONT_ICON),
    ]
    .align_y(Alignment::Center)
    .spacing(SPACE_LG)
    .height(Length::Fill),
)
.on_press(on_toggle.clone())
.padding(PADDING_GROUPED)
.height(Length::Fixed(CONTROL_HEIGHT))
.style(picker_button())
.into();
```

Build the iced_aw picker + wrapper:
```rust
use iced_aw::time_picker::{Period, Time, TimePicker as AwTimePicker};

let time = parse_hhmm(value)
    .map(|(h, m)| Time::Hm { hour: h as u32, minute: m as u32, period: Period::H24 })
    .unwrap_or_else(|| Time::now_hm(true));

let inner: Element<'a, M> = AwTimePicker::new(
    open,
    time,
    underlay,
    on_toggle.clone(),
    move |t: Time| (on_change)(t.to_string()),
)
.use_24h()
.into();

Element::new(TimePickerStateful { open, value, inner })
```

Wrapper widget `TimePickerStateful<'a, M>` (`impl Widget<M, iced::Theme, iced::Renderer>`):
- Fields: `open: bool`, `value: &'a str`, `inner: Element<'a, M>`.
- `tag()` → `tree::Tag::of::<WrapperState>()`; `state()` → `WrapperState { prev_open: false }`; `children()` → `vec![Tree::new(&self.inner)]`.
- `diff()`: if `self.open && !state.prev_open`, re-seed `tree.children[0].state = tree::State::new(iced_aw::time_picker::State::new(Time::Hm { ... } from parse_hhmm(self.value), true, false))`; then `state.prev_open = self.open`; `tree.diff_children(&[&self.inner])`. (`State::new(time, use_24h, show_seconds)` and `Tree.state` are public.)
- `size`/`layout`/`draw`/`update`/`operate`/`mouse_interaction`: forward to `self.inner` with `tree.children[0]` (same shape as the old wrapper did for the underlay).
- `overlay()`: `self.inner.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)`.

Imports needed (trim the custom-panel ones): `iced::advanced::{widget::{self, tree, Operation, Tree, Widget}, mouse, overlay, renderer, Clipboard, Layout, Shell}`, `iced::advanced::Renderer as _`, `iced::mouse::Cursor`, `iced::{Alignment, Element, Event, Length, Rectangle, Size, Vector}`, `iced::widget::{button, row}` (keep `icon`, `theme`, `dims::*`, `CONTROL_HEIGHT`, `parse_hhmm`). No `chrono`, `keyboard`, `touch`, `Pixels`, `Font` imports needed anymore.

### 4. settings_page.rs — revert both call sites
Start picker (and end picker identically):
```rust
time_picker(
    &settings.speed_limit_schedule.start,
    settings_ui.schedule_start_picker_open,
    Message::ToggleScheduleStartPicker,
    move |s| Message::SettingChanged(SettingKey::ScheduleStart, s),
),
```
Remove the `fluent.get(Tr::TimeNow)`, `fluent.get(Tr::TimeOk)`, and `Length::Fixed(160.0)` args.

### 5. app.rs — close picker on commit
In the `SettingChanged` match (src/app.rs:1064), add a close for each arm:
```rust
SettingKey::ScheduleStart => {
    if crate::scheduler::parse_hhmm(&value).is_some() {
        state.settings.speed_limit_schedule.start = value;
    }
    state.settings_ui.schedule_start_picker_open = false;
}
SettingKey::ScheduleEnd => {
    if crate::scheduler::parse_hhmm(&value).is_some() {
        state.settings.speed_limit_schedule.end = value;
    }
    state.settings_ui.schedule_end_picker_open = false;
}
```
(These SettingKeys are only ever emitted by the picker's OK, so auto-close is safe.)

### 6. Revert i18n
- `src/i18n.rs`: remove `Tr::TimeNow`, `Tr::TimeOk` and their `key()` arms.
- `en/main.ftl` and `zh-CN/main.ftl`: remove `time-now = ...` / `time-ok = ...` lines.

### 7. Optional: AGENTS.md
Update the dependencies table line for iced_aw (`features = ["time_picker"]`) and note the picker now uses the iced_aw clock component.

## Validation
- `cargo build`, `cargo clippy --workspace` (zero warnings), `cargo fmt --check`, `cargo test`.
- Manual (Settings > Download > Speed Limits > enable scheduled speed limit):
  - Trigger shows the current `HH:MM` + clock icon; width is exactly text + gap + icon; text and icon are vertically centered.
  - Click opens the iced_aw analog-clock panel centered on the trigger (24h clock face, digital display, icon Cancel/OK buttons).
  - Click/drag the clock hands or use the digital display arrows; Cancel (×) closes without saving; OK (✓) commits → trigger text updates and the panel closes; `settings.json` updates on Apply.
  - Reopen after a commit shows the committed value (wrapper state re-seed works).
  - Start and end pickers work independently; values persist across restart.
  - Known/accepted: Escape and outside clicks do not close the panel (iced_aw limitation); the trigger click still toggles it closed.

## Risks / notes
- First build after adding `iced_aw` downloads new crates (iced_fonts 0.3, etc.); iced_aw is already in the local cargo registry cache.
- The analog-clock UI replaces the antd columns and has no "Now" button (user-approved).
- The `time_picker` builder now requires `M: 'static + Clone` and `on_change: 'static`; the two existing call sites already satisfy this.
- `Time::to_string()` yields exactly `HH:MM` only with `.use_24h()` + `Period::H24`; keep both to guarantee `parse_hhmm` validation passes on commit.
- `tree.children[0].state` reassignment must happen before `diff_children`; the iced_aw `diff` only touches its own children, so the re-seeded state survives.
