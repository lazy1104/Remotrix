# Plan: Task-list toolbar — right-align, New-Download icon, sort dropdown

## Goal
Rework the toolbar in `src/ui/task_list.rs` (plus supporting files):
1. Right-align the action-button group.
2. Add a **New Download** icon button as the first (leftmost) toolbar item — same behavior as the sidebar "New" button (`Message::OpenAddDialog`).
3. Replace the sort `combo_box` with a **sort icon button** that opens a dropdown popup listing the 5 sort fields **plus an Asc/Desc toggle**.
4. Highlight the sort icon when the dropdown is open **OR** the active sort field differs from the default (`SortField::AddedTime`).

## Confirmed decisions
- Popup mechanism: `iced_aw::widget::drop_down::DropDown` (enable `drop_down` feature on `iced_aw`). `iced_aw` is already a dependency.
- Sort icon highlight trigger: `sort_menu_open || sort_field != SortField::AddedTime`.
- Popup includes Asc/Desc toggle (reuses existing `SortOrder` enum).
- Menu stays open after picking a field or toggling order; closes only via click-outside / Escape (`on_dismiss`) or toggling the sort icon again.
- Icon glyphs (font name `"lucide"`, already registered via `crate::ui::icon::FONT`):
  - New Download: `'\u{E13D}'` (`icon::plus`, same as sidebar New)
  - Sort: `'\u{E37D}'` (`icon::sort` / "arrow-up-down")
- Toolbar layout: `[NewDownload] <Fill> [Refresh][Sort][StartAll][PauseAll][DeleteAll][ClearCompleted]`.

## File changes

### 1. `Cargo.toml`
Add `drop_down` feature to `iced_aw`:
```toml
iced_aw = { version = "0.14", default-features = false, features = ["number_input", "drop_down"] }
```

### 2. `src/message.rs`
Add variants to `enum Message`:
- `ToggleSortMenu`
- `CloseSortMenu`
- `ToggleSortOrder`

Keep existing `SortSelected(SortField)`. `SortField`/`SortOrder` enums unchanged.

### 3. `src/i18n.rs`
- Add `SortAsc`, `SortDesc` to `enum Tr`.
- In `Tr::key()`: `SortAsc => "sort-asc"`, `SortDesc => "sort-desc"`.

### 4. Locale files
- `i18n/locales/en/main.ftl`:
  ```
  sort-asc = Ascending
  sort-desc = Descending
  ```
- `i18n/locales/zh-CN/main.ftl`:
  ```
  sort-asc = 升序
  sort-desc = 降序
  ```
  (Existing `sort`, `sort-by-added`…`sort-by-status`, `new-download` already present — reuse them.)

### 5. `src/app.rs`
- **Remove** `sort_combo_state: combo_box::State<SortField>` field; remove the `sort_options` vec and `sort_combo_state: combo_box::State::new(sort_options)` init in `init()`.
- **Add** field `sort_menu_open: bool` (default `false`) to `Remotrix` and init.
- Remove `combo_box` from `use iced::widget::{column, combo_box, container, row, stack, text_editor};` (grep confirmed `combo_box` is used only here and in `task_list.rs`).
- In `update()`, handle new messages:
  - `Message::ToggleSortMenu => state.sort_menu_open = !state.sort_menu_open`
  - `Message::CloseSortMenu => state.sort_menu_open = false`
  - `Message::ToggleSortOrder => state.sort_order = match state.sort_order { SortOrder::Asc => SortOrder::Desc, SortOrder::Desc => SortOrder::Asc }`
  - `Message::SortSelected(field) => state.sort_field = field` (do **not** close menu)
  (`SortOrder` is already imported.)
- In `view()`, change the `task_list::view` call (line ~677) to pass sort state instead of combo state:
  ```rust
  crate::ui::task_list::view(
      &state.fluent, t, &sorted,
      state.sort_field, state.sort_order, state.sort_menu_open,
  )
  ```

### 6. `src/ui/task_list.rs`
- **Imports**: remove `combo_box`; add `use iced_aw::widget::drop_down;`; change `use crate::message::{Message, SortField};` → `use crate::message::{Message, SortField, SortOrder};`. Keep `use crate::ui::theme;`.
- **`view` signature**: replace
  `sort_combo_state: &'a combo_box::State<SortField>,`
  with
  `sort_field: SortField, sort_order: SortOrder, sort_menu_open: bool,`
- **Toolbar button helper**: extend `toolbar_btn` to take `active: bool`; when `active`, color the glyph with `theme::ACCENT` (else leave default). Pass `false` for New-Download/Refresh/StartAll/PauseAll/DeleteAll/ClearCompleted.
- **Toolbar layout** (`toolbar` row):
  1. `toolbar_btn('\u{E13D}', fluent.get(Tr::NewDownload), Message::OpenAddDialog, false)` — leftmost New Download.
  2. `iced::widget::Space::new().width(Length::Fill)` — pushes the rest right.
  3. Right group row (`.spacing(4).align_y(Center)`): Refresh (`Message::Refresh`), then the **sort DropDown** (see below), then StartAll, PauseAll, DeleteAll, ClearCompleted (unchanged glyphs/messages).
- **Sort DropDown** (replaces `combo_w`):
  - `sort_active = sort_menu_open || sort_field != SortField::AddedTime;`
  - Underlay button: `toolbar_btn('\u{E37D}', fluent.get(Tr::Sort), Message::ToggleSortMenu, sort_active)` but as a standalone button built the same way (not via the shared closure if the closure returns a tooltip-wrapped element — DropDown needs the raw button element). Build the sort underlay as a plain `button(glyph).on_press(Message::ToggleSortMenu).padding([6,8]).style(button::text)` with glyph colored `theme::ACCENT` when `sort_active`. (Tooltip not required for the underlay since DropDown wraps it; if desired, DropDown can wrap a tooltip-wrapped button — but keep it simple: no tooltip on sort underlay, the popup itself is the affordance.)
  - Overlay: a `container` styled `theme::style::card` (or a column with card style) of width `Length::Fixed(170.0)`, spacing 2, containing:
    - Asc/Desc toggle button: label = `fluent.get(Tr::SortDesc)` if `sort_order == SortOrder::Desc` else `fluent.get(Tr::SortAsc)`; `on_press(Message::ToggleSortOrder)`; full width; selected look optional.
    - A thin divider (e.g. `iced::widget::rule::horizontal` or a 1px space).
    - Five field buttons built from a static array:
      `[(SortField::AddedTime, Tr::SortByAdded), (SortField::Name, Tr::SortByName), (SortField::Size, Tr::SortBySize), (SortField::Progress, Tr::SortByProgress), (SortField::Status, Tr::SortByStatus)]`
      Each button: left-aligned label `fluent.get(tr)`, `on_press(Message::SortSelected(field))`. For the field equal to `sort_field`, style with accent background (reuse `theme::style::button::sidebar_icon(true)` or a custom accent style) and optionally prefix a check glyph; others use `button::text`.
  - Construct:
    ```rust
    drop_down::DropDown::new(sort_underlay, sort_overlay, sort_menu_open)
        .on_dismiss(Message::CloseSortMenu)
        .width(Length::Fixed(170.0))
    ```
    (Default alignment `Bottom` + offset 5 places the menu below the icon — acceptable.)

## Validation
- `cargo fmt --check`
- `cargo clippy --workspace` — must be warning-free (remove now-dead `combo_box` import/field to avoid warnings; `Tr::Sort`/`SortOrder` still used).
- `cargo build` — offline build must succeed (no new network dep; `drop_down` feature is local to iced_aw).
- Manual run (`cargo run --`):
  - New Download icon (left) opens the Add dialog (same as sidebar New).
  - Action buttons sit on the right; New Download sits on the left.
  - Click sort icon → popup with 5 fields + Asc/Desc toggle; selecting a field re-sorts immediately; toggling Asc/Desc flips direction; popup stays open.
  - Sort icon is accent-colored while popup open, or when current sort ≠ AddedTime.
  - Click outside / Escape closes the popup.

## Risks / notes
- `DropDown` overlay must set an explicit width (`Length::Fixed(170.0)`) or the column collapses to Shrink.
- `DropDown` underlay must be a plain button element (not a tooltip-wrapped `Element`) so its bounds drive overlay positioning; if a tooltip is wanted, wrap after construction — verify it still positions correctly, otherwise drop the tooltip on the sort icon.
- Removing `combo_box`: double-check no remaining references via `rg combo_box src/` after edits (expected: none).
- `SortOrder` was previously fixed to `Desc` and unused by UI; it is now user-controllable and still applied in `crate::ui::sort::sort_tasks` (no change needed there).
