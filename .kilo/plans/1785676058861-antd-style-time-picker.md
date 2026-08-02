# Time Picker Redesign: antd-style Panel

## Goal
Replace the `number_stepper`-based time picker (`src/ui/components/time_picker.rs`) with an antd-TimePicker-style component:

- Trigger: rounded-rectangle field showing `HH:MM` + a **clock icon**.
- Clicking the trigger opens a panel **below** it when there is room, **above** when not (recomputed as the window scrolls).
- Panel: two scrollable columns `00–23` (hours) and `00–59` (minutes). Selected row is auto-centered on open, highlighted with the accent color; hover highlight; mouse-wheel scrolling.
- Footer with **现在 / 确定** buttons placed at the **far end from the trigger** (bottom when the panel opens downward, top when it opens upward) — confirmed with user.
- Draft semantics (confirmed): clicking rows edits a draft; **现在** sets the draft to the current local time and keeps the panel open; **确定** commits via `on_change("HH:MM")` and closes; Escape / outside click / clicking the trigger closes without committing.

## Confirmed decisions
- Footer at far end from trigger (not antd's always-bottom).
- `现在` fills draft + keeps panel open; `确定` commits and closes.

## Key technical facts (from exploration)
- iced 0.14 has **no self-contained programmatic scroll-to** (`operation::scrollable::scroll_to` needs an app-level `Task`). => Columns must be fully custom-drawn (own scroll offset in tree state) to center the selection on open.
- Panel internals (row clicks, Now, OK) must **not** emit the generic `M` messages (they would route to the app's `update`); only commit/toggle reach the app. => Panel handles its own mouse input via `Event::Mouse` + `shell.capture_event()`, publishing `M` only via stored `Rc<dyn Fn(String) -> M>` (`on_change`) and `M on_toggle`.
- The shared `DropDown` primitive has fixed alignment (no flip) and cannot place the footer at the far end, so the time picker gets its **own `overlay()` implementation** (mirroring `DropDownOverlay`: Escape + outside-click dismiss, viewport clamping). `drop_down.rs` is unchanged.
- Custom drawing is supported: `renderer.with_layer(bounds, f)` (clip), `renderer.fill_quad` (rects), `renderer.fill_text(iced::text::Text { .. })` (rows/buttons).
- Icons are generated from `fonts/icons.toml` by `build.rs` (`iced_lucide`). Adding `clock = "clock"` regenerates `src/ui/icon.rs` with a `clock()` fn automatically.
- Model widgets: `NumberStepper` (tree-state + custom `update`) and `DropDown` (custom `overlay`).

## Files touched
1. `fonts/icons.toml` — add `clock = "clock"`.
2. `src/i18n.rs` + `i18n/locales/en/main.ftl` + `i18n/locales/zh-CN/main.ftl` — add `TimeNow`/`TimeOk`.
3. `src/ui/components/time_picker.rs` — full rewrite (new custom widget).
4. `src/ui/settings_page.rs` — pass the two new label params at both call sites.

## Implementation steps

### 1. Icon + i18n
- `fonts/icons.toml`: add `clock = "clock"` (a rebuild regenerates `icon.rs`; do not hand-edit `icon.rs`).
- `src/i18n.rs`: add `Tr::TimeNow`, `Tr::TimeOk` + `key()` mappings `"time-now"`, `"time-ok"`.
- `en/main.ftl`: `time-now = Now`, `time-ok = OK`.
- `zh-CN/main.ftl`: `time-now = 现在`, `time-ok = 确定`.

### 2. Rewrite `src/ui/components/time_picker.rs`

Module constants:
- `ROW_H = 30.0`, `ROWS_VISIBLE = 8` => `VIEWPORT_H = 240.0`
- `COL_W = 56.0`, `PANEL_W = COL_W * 2 + SPACE_LG + 2 * PADDING_DROPDOWN` (~132)
- `FOOTER_H = 38.0`, `PANEL_H = VIEWPORT_H + FOOTER_H + 2 * PADDING_DROPDOWN + 1` (separator)

Public entry — keep the existing call-site shape, add two label params:
```rust
pub fn time_picker<'a, M>(
    value: &'a str,          // committed "HH:MM"
    open: bool,
    on_toggle: M,
    on_change: impl Fn(String) -> M + 'a,
    now_label: &'a str,      // from fluent: Tr::TimeNow
    ok_label: &'a str,       // from fluent: Tr::TimeOk
    width: Length,
) -> Element<'a, M, iced::Theme, iced::Renderer>
where M: 'a + Clone;
```
Builds a `TimePicker` widget containing the underlay element (rounded-rect `button` with `row![ text("HH:MM").width(Fill), icon::clock().size(FONT_ICON) ]`, `picker_button()` style kept, height `CONTROL_HEIGHT`, width `Length::Fill`, `on_press = on_toggle`) and a diff-template panel element.

`TimePicker` widget (`impl Widget<M, iced::Theme, iced::Renderer>`):
- `TimePickerState { draft_h: u8, draft_m: u8, prev_open: bool }` at `tree.state`.
- `children()`: `[Tree::new(&underlay), Tree::new(&panel_template)]`.
- `diff()`: if `open && !prev_open`, seed `draft_h/draft_m` from `parse_hhmm(value)` and reset the panel tree's `PanelState.centered = false` (via `tree.children[1].state.downcast_mut::<PanelState>()`); set `prev_open = open`; `tree.diff_children(&[&underlay, &panel_template])`.
- `size/layout/draw/update/operate/mouse_interaction`: forward to the underlay (as `DropDown` does).
- `overlay()`: if `!open` -> forward the underlay's overlay; else compute direction — `space_below = viewport.height - (pos.y + pos.height)`, `space_above = pos.y` (using `layout.bounds()` + `viewport`), `direction = if space_below >= space_above { Bottom } else { Top }` — build a fresh `TimePickerPanel` element (with `draft_h/draft_m` from state, `direction`, `on_change` Rc, `on_toggle`, labels), and return `TimePickerOverlay { state: &mut tree.children[1], panel, on_toggle, position, viewport, underlay_bounds }`.

`TimePickerPanel` widget (fully custom-drawn, no child elements):
- fields: `draft_h`, `draft_m`, `direction`, `on_change: Rc<dyn Fn(String) -> M + 'a>`, `on_toggle: M`, `now_label`, `ok_label`.
- `PanelState { scroll_h: f32, scroll_m: f32, hovered: Option<Item>, centered: bool }` where `Item = Hour(usize) | Minute(usize) | Now | Ok`.
- `layout`: fixed `Size::new(PANEL_W, PANEL_H)`; compute `footer_rect` (top edge when `Top`, bottom edge when `Bottom`) and the columns viewport rect (the remainder); store nothing in state, recompute from the passed `Layout` in `update`/`draw`.
- `draw`: card background (`theme::style::card`-style quad); separator between columns and above footer; within `renderer.with_layer(columns_rect, ..)`: for each visible row (from `scroll_y`), draw the selected row's accent-tinted band + accent-colored `fill_text`, hovered row's weak band, then `fill_text("{:02}", row)`. Footer: Now/OK as rounded buttons with hover fill, text labels via `fill_text`.
- `update`: handle `Event::Mouse`:
  - `WheelScrolled` over a column: `scroll_y += delta` (handle both `ScrollDelta::Lines`/`Pixels`), clamp to `[0, (n_rows*ROW_H) - VIEWPORT_H]`, `shell.capture_event()`.
  - `ButtonPressed(Left)` over a visible row: set `draft_h`/`draft_m`, capture.
  - over Now: set both drafts from `chrono::Local::now()`, capture (panel stays open).
  - over OK: `shell.publish((on_change)(format!("{:02}:{:02}", draft_h, draft_m)))` then `shell.publish(on_toggle.clone())`, capture.
  - `CursorMoved`: update `hovered` for row/button highlight.
  - Center-on-open: in `update` (or `layout`), if `!centered { scroll to center draft; centered = true }` — done once per open (flag reset by `TimePicker::diff`).
- `mouse_interaction`: `Pointing` over panel.

`TimePickerOverlay` (impl `overlay::Overlay<M, iced::Theme, iced::Renderer>`), mirroring `DropDownOverlay`:
- `layout`: place panel at `(underlay.x, y)` — below (`underlay.y + underlay.h`) or above (`underlay.y - panel_h`) per `direction` — then clamp x/y into the viewport.
- `draw`: forward to panel.
- `update`: Escape -> publish `on_toggle`; mouse/touch press not over panel and not over underlay -> publish `on_toggle`; otherwise forward to panel's `update` (panel captures consumed events).
- `mouse_interaction`: forward to panel.

Notes: keep `M: 'a + Clone`; `Rc<dyn Fn(String) -> M + 'a>` for `on_change`. Remove the `number_stepper`/`drop_down` imports no longer used here. `icon::clock()` replaces `icon::chevron_down()` in the trigger.

### 3. `src/ui/settings_page.rs`
- Both `time_picker(...)` calls (start/end) gain two args: `&fluent.get(Tr::TimeNow)` and `&fluent.get(Tr::TimeOk)`. No other changes (the `SettingsUiState` open booleans, `ToggleScheduleStart/EndPicker` messages, and `SettingChanged(ScheduleStart/End, ...)` flow stay as-is).

## Validation
- `cargo build`, `cargo clippy --workspace` (zero warnings), `cargo fmt --check`, `cargo test`.
- Manual (Settings > Download > Speed Limits > enable scheduled speed limit):
  - Trigger shows `HH:MM` + clock icon in a rounded rect; clicking toggles the panel.
  - Panel opens below by default; with the window scrolled so the trigger is near the bottom it opens above; footer is at the far end in both cases; selected row is centered on open.
  - Wheel-scroll both columns; click rows to build a draft (trigger unchanged until OK).
  - 现在 sets the draft to current time, panel stays open; 确定 commits (trigger + settings.json update), panel closes.
  - Escape / outside click / clicking the trigger closes without committing.
  - Editing start/end independently works; values persist across restart.

## Risks / notes
- The fully custom panel is the largest new widget in the project; keep it consistent with `NumberStepper` (state in `tree`, direct `Event::Mouse` handling) and `DropDown` (overlay mechanics) so review is straightforward.
- Direction is recomputed each `overlay()` call, so it stays correct while the settings page scrolls.
- `PANEL_H` must be an upper bound for direction estimation; viewport clamping handles remaining overflow (panel may slightly overlap the trigger in extreme cases, same as `DropDown`).
- Only commit/toggle messages reach the app — no new `Message`/`SettingKey` variants are needed.
- `clock` icon requires a rebuild to regenerate `icon.rs`; it is generated, never hand-edited.
