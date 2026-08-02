# File Tree & Details Dialog Refinement

## Goal

Polish the torrent file tree and its dialog usage:

1. Remove per-file `progress_bar` widgets (too noisy); show progress as
   "downloaded / size" text in the size column.
2. Keep the single overall progress bar at the top of the Files tab.
3. **New composite component `torrent_file_list`** that owns the fixed title row
   ("Torrent Files" + Select All / Select None, optional size subtitle), the
   separator rule, the scrollable tree, AND the single rounded 1px border frame
   around the whole panel (title row included). Both dialogs use it, so the
   border/scrollbar/header are written exactly once.
4. Unify the dir-row toggle and file-row checkbox into one consistent control
   (same size, same native-checkbox look) using a new **tri-state checkbox**
   widget, since iced 0.14's native `checkbox` is strictly binary
   (`is_checked: bool`, no indeterminate state — confirmed in
   `iced_widget-0.14.2/src/checkbox.rs:91`).
5. Long file names must truncate with "…" (already handled by
   `truncated_text`; must be preserved, not regressed).

## Decisions (confirmed with user)

- Per-file progress representation: **"downloaded / size" text** (dir rows
  aggregate downloaded across descendants). No background-fill rows.
- Overall top progress bar in Files tab: **keep** (stays above the panel, not
  inside it).
- Border + title row + scrollbar all live in ONE place: a new
  `torrent_file_list` component (the title row is currently duplicated in
  `details_dialog.rs` and `add_dialog.rs`, NOT in `file_tree.rs`). The border
  wraps the header + rule + scrollable tree together as a single framed panel.
- Scrollbar stays in `file_tree.rs` (`file_tree::view` returns the tree wrapped
  in `slim_scrollable`); `torrent_file_list` adds the header and the frame
  around `file_tree::view`.
- Toggle control: **custom tri-state checkbox** used for both dir and file rows
  (native checkbox look, checkmark + drawn "minus" bar for partial state).

## Constraints

- `file_tree::view` keeps its **original 7-parameter signature** (returns the
  scrollable tree, no frame, no `height` param). Both dialogs stop calling it
  directly and go through `torrent_file_list::view` instead.
- No changes to `app.rs` or i18n strings (Select All / Select None reuse
  `Tr::SelectAll` / `Tr::SelectNone`).
- No new external dependencies.
- Imports grouped `std` → external crates → `crate::`; no code comments unless
  a non-obvious decision requires one.

## Implementation Tasks

### 1. New component: `src/ui/components/tri_checkbox.rs`

A small custom widget modeled on `iced_widget-0.14.2/src/checkbox.rs` and on the
existing custom-widget pattern in `src/ui/components/truncated_text.rs`.

API sketch:

```rust
pub enum CheckState { Checked, Partial, Unchecked }

pub fn tri_checkbox(state: CheckState) -> TriCheckbox<'_, Message>;

pub struct TriCheckbox<'a, Message> {
    state: CheckState,
    size: f32,                      // default 16.0
    on_toggle: Option<Box<dyn Fn() -> Message + 'a>>, // None => disabled
}
// methods: size(Pixels), on_toggle(Fn() -> Message), on_toggle_maybe(Option<F>)
```

`Widget<Message, iced::Theme, Renderer>` impl with
`Renderer: iced::advanced::text::Renderer`:

- **tag/state**: stateless toggle box but store hover status for redraw:
  `struct TriCheckState { last_status: Option<iced::widget::checkbox::Status> }`
  (mirror `Checkbox::update`/`last_status` logic in `checkbox.rs:318-364`).
- **size()**: `Size { width: Length::Fixed(self.size), height: Length::Fixed(self.size) }`.
- **layout()**: `layout::Node::new(Size::new(self.size, self.size))`.
- **update()**: on left mouse press / touch over bounds with `on_toggle` Some →
  `shell.publish((on_toggle)())` + `shell.capture_event()`; track
  Active/Hovered/Disabled status and `shell.request_redraw()` on change.
- **mouse_interaction()**: `Pointer` when over bounds and `on_toggle` is Some.
- **draw()**:
  1. `let status = state.last_status.unwrap_or(Disabled { is_checked: self.state != Unchecked });`
  2. `let style = iced::widget::checkbox::primary(theme, status);` — reuses the
     native checkbox style (accent fill when checked/partial, bordered box when
     unchecked, weaker on Disabled).
  3. `renderer.fill_quad(Quad { bounds, border: style.border, ..Quad::default() }, style.background);`
  4. `Checked` → `renderer.fill_text(Text { content: Renderer::CHECKMARK_ICON.to_string(), font: Renderer::ICON_FONT, size: Pixels(bounds.height * 0.7), ... }, bounds.center(), style.icon_color, viewport)` — identical to native (`checkbox.rs:413-439`).
  5. `Partial` → draw a small rounded "minus" quad centered in the box
     (`width = bounds.width * 0.55`, `height = bounds.height * 0.15`,
     rounded radius `height/2`, color `style.icon_color`) via `fill_quad`.
  6. `Unchecked` → nothing.
- `From<TriCheckbox<'a, Message>> for Element<'a, Message, iced::Theme, Renderer>`.

### 2. `src/ui/components/file_tree.rs`

Imports: drop `checkbox`, `container`, `progress_bar` from the iced widget
import; add `use crate::ui::components::slim_scrollable::slim_scrollable;` and
`use crate::ui::components::tri_checkbox::{tri_checkbox, CheckState};`.

**Dir row (`render_dir_row`)** — replace the `tri_btn` icon-button match with:

```rust
let check_state = match dir_state(node, is_selected) {
    Some(true) => CheckState::Checked,
    Some(false) => CheckState::Partial,
    None => CheckState::Unchecked,
};
let on_toggle_maybe = if enabled {
    Some(on_toggle(node.rel_path.clone()))
} else {
    None
};
tri_checkbox(check_state).size(16.0).on_toggle_maybe(on_toggle_maybe)
```

**File row (`render_file_row`)** — replace the native `checkbox` with
`tri_checkbox(if is_selected(idx) { Checked } else { Unchecked })`, `.size(16.0)`,
`on_toggle_maybe(Some(move || on_toggle(rel.clone())))` when enabled, else
`on_toggle_maybe(None::<fn() -> M>)`.

**Size column**:
- File row: if `progress` is Some →
  `format!("{} / {}", format_size(done), format_size(node.length))` where
  `done = p(idx).map(|(d, _)| d).unwrap_or(0)`; else `format_size(node.length)`.
- Dir row: if `progress` is Some → aggregate via
  `descendant_indices(node)` summing `progress(i).map(|(d, _)| d).unwrap_or(0)`,
  render `"done / total"`; else `format_size(node.length)`.
- Keep existing `text(...).size(FONT_SMALL).style(theme::style::text::secondary)`.

**Remove** the `progress_bar` block under each file row (its import is dropped).
Keep the chevron `button`s, `icon::folder`/`icon::file`, and the
`truncated_text(node.name.clone()).max_lines(1).width(Length::Fill)` name
columns (truncation preserved).

**`view`** keeps its original 7 parameters and now returns the scrollable tree
(no frame — the frame lives in `torrent_file_list`):

```rust
pub fn view<'a, M>(
    nodes, expanded, is_selected, progress, enabled, on_toggle, on_expand,
) -> Element<'a, M> {
    let mut col = column![].spacing(SPACE_SM).width(Length::Fill);
    for node in nodes {
        col = col.push(render_node(node, 0, /* ... */));
    }
    slim_scrollable(col).height(Length::Fill).into()
}
```

`build_tree`, `flip_with_guard`, `sum_lengths`, `descendant_indices`,
`find_node`, `collect_dir_paths`, `dir_state`: unchanged.

### 3. New component: `src/ui/components/torrent_file_list.rs`

The framed panel = title row + separator + `file_tree::view`. This is the single
place that owns the header and the border.

```rust
#[allow(clippy::too_many_arguments)]
pub fn view<'a, M>(
    fluent: &'a Fluent,
    theme: &'a iced::Theme,
    title: String,                  // e.g. "Torrent Files" or "Torrent Files (42)"
    subtitle: Option<String>,       // optional right-side text (e.g. total size)
    height: Length,                 // frame height: details Fill, add Fixed(~230)
    nodes: &'a [FileTreeNode],
    expanded: &'a HashSet<String>,
    is_selected: &impl Fn(u64) -> bool,
    progress: Option<&impl Fn(u64) -> Option<(u64, u64)>>,
    enabled: bool,
    on_toggle: &'a impl Fn(String) -> M,
    on_expand: &impl Fn(String) -> M,
    on_select_all: M,
    on_select_none: M,
) -> Element<'a, M>
where
    M: Clone + 'a,
{
    let mut header = row![
        text(title).size(FONT_MEDIUM),
        iced::widget::Space::new().width(Length::Fill),
    ];
    if let Some(sub) = subtitle {
        header = header
            .push(text(sub).size(FONT_SMALL).style(theme::style::text::secondary));
    }
    header = header
        .push(
            button(text(fluent.get(Tr::SelectAll)).size(FONT_SMALL))
                .on_press(on_select_all)
                .padding(PADDING_XS)
                .style(theme::style::button::text()),
        )
        .push(
            button(text(fluent.get(Tr::SelectNone)).size(FONT_SMALL))
                .on_press(on_select_none)
                .padding(PADDING_XS)
                .style(theme::style::button::text()),
        )
        .spacing(SPACE_SM)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    container(
        column![]
            .push(header)
            .push(rule::horizontal(1))
            .push(file_tree::view(
                nodes, expanded, is_selected, progress, enabled, on_toggle, on_expand,
            ))
            .spacing(SPACE_MD)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(height)
    .padding(iced::Padding::from([PADDING_XS as f32, PADDING_SM as f32]))
    .style(theme::style::tree_frame)
    .into()
}
```

Notes:
- The tree scrollable is `Length::Fill`, so it takes whatever space remains
  inside the framed panel after the header + rule.
- `height` is the total frame height: `Length::Fill` (details tab) or
  `Length::Fixed(230.0)` (add dialog, keeps the tree area ≈ the previous 200px).
- `enabled` is forwarded to `file_tree::view`, which disables the row toggles
  internally when `false` (completed/removed task); the Select All / Select None
  buttons stay enabled in both dialogs, matching current behavior.

Register both new modules in `src/ui/components/mod.rs`:
`pub mod torrent_file_list;` (plus `pub mod tri_checkbox;` from task 1).

### 4. `src/ui/theme.rs`

Add to `pub mod style` a `tree_frame` container style (near `card`):

```rust
pub fn tree_frame(t: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(t.extended_palette().background.base.color.into()),
        border: iced::Border {
            color: super::border_color(t),
            width: 1.0,
            radius: iced::border::rounded(super::RADIUS_CARD).radius,
        },
        ..Default::default()
    }
}
```

(Subtle frame: light base background + 1px `border_color` + `RADIUS_CARD`, no
shadow.)

### 5. `src/ui/details_dialog.rs`

In `files_tab`, delete the local `header` row and the
`rule::horizontal(1)`/`slim_scrollable` pushes; replace them with the composite
component. Keep the overall bar + overall info above the panel:

```rust
let file_list: Element<'a, Message> = if let Some(ref details) = state.details {
    let files_map: HashMap<u64, (bool, u64, u64)> = /* unchanged */;
    let is_selected = /* unchanged */;
    let progress = /* unchanged */;
    let enabled = /* unchanged */;
    crate::ui::components::torrent_file_list::view(
        fluent,
        theme,
        fluent.get(Tr::TorrentFiles),          // title
        None,                                  // subtitle
        Length::Fill,                          // height
        &state.files_tree,
        &state.files_expanded,
        &is_selected,
        Some(&progress),
        enabled,
        &details_tree_toggle,
        &details_tree_expand,
        Message::DetailsFilesSelectAll,
        Message::DetailsFilesSelectNone,
    )
} else {
    // framed loading placeholder so the tab doesn't jump:
    container(
        container(text(fluent.get(Tr::Loading)).size(FONT_BODY).style(text_secondary_fn))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(PADDING_XS)
    .style(theme::style::tree_frame)
    .into()
};
```

Then: `.push(overall_bar).push(overall_info).push(file_list)`. Remove the
`use crate::ui::components::slim_scrollable::slim_scrollable;` import (now
unused — confirmed it only appears at old line 414).

### 6. `src/ui/add_dialog.rs`

In the Torrent tab, replace the local header + `slim_scrollable` wrapping with
the composite component:

```rust
let tree_panel = crate::ui::components::torrent_file_list::view(
    fluent,
    theme,
    format!("{} ({})", fluent.get(Tr::TorrentFiles), total_count),
    Some(format_size(total_size)),
    Length::Fixed(230.0),
    &state.torrent_tree,
    &state.torrent_expanded,
    &is_selected,
    None::<&fn(u64) -> Option<(u64, u64)>>,
    true,
    &torrent_tree_toggle,
    &torrent_tree_expand,
    Message::TorrentFilesSelectAll,
    Message::TorrentFilesSelectNone,
);
body_items.push(tree_panel.into());
```

Keep the `selected_line` ("selected / total · size") below the panel. The
`progress = None` path is unchanged (total-only sizes, no bars). The local
`header` row and its `slim_scrollable(tree_el)` are removed.

## Files touched

- `src/ui/components/tri_checkbox.rs` (new)
- `src/ui/components/torrent_file_list.rs` (new)
- `src/ui/components/mod.rs` (register both new modules)
- `src/ui/components/file_tree.rs`
- `src/ui/theme.rs`
- `src/ui/details_dialog.rs`
- `src/ui/add_dialog.rs`

## Validation

```bash
cargo build                  # offline OK; no new deps
cargo clippy --workspace     # no warnings allowed
cargo fmt --check            # format new/edited files
cargo run --                 # manual: open a torrent's Details → Files tab
```

Manual checks:
- No per-file progress bars; size column shows "downloaded / size" (dirs
  aggregate); total-only still shown in add-dialog (progress=None path).
- Overall bar at top of Files tab still present, above the panel.
- In BOTH dialogs the framed panel wraps the title row + rule + scrollable tree
  together; the border follows the panel (not the expanded tree height).
- Dir toggle shows ✓ / − / □ correctly for all / partial / none; file rows show
  ✓ / □; disabled (completed/removed task) rows render greyed.
- Long names truncate with "…" and the size column stays visible.

## Risks / Notes

- Dir size aggregation calls `descendant_indices` per dir row (also used by
  `dir_state`); fine for typical torrents, worst case O(n²) — acceptable.
- The custom widget reuses `checkbox::primary` so hover/disabled visuals match
  the native checkbox exactly; `Renderer::CHECKMARK_ICON`/`ICON_FONT` are
  associated consts of the text renderer (confirmed in
  `iced_core-0.14.0/src/text.rs:303-308`).
- `file_tree::view` is now consumed only by `torrent_file_list`; its scrollable
  is always `Length::Fill` inside the framed panel, so the panel's `height`
  param controls the total size.
- add_dialog panel uses `Length::Fixed(230.0)` so the tree area stays ~200px
  after the header/rule; tune if the dialog layout feels different.
- Title styling is normalized inside the component (default text color at
  FONT_MEDIUM); details_dialog previously used a secondary-styled title — a
  negligible visual change, acceptable for one shared component.
