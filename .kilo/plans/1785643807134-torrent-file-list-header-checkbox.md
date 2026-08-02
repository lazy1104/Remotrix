# Torrent File List Header: master tri-checkbox + top spacing

## Goal

Tighten the header of the framed torrent-file panel (`src/ui/components/torrent_file_list.rs`):

1. Add top spacing above the title row (currently the frame's top padding is only 2px, so the title sits too tight against the frame edge).
2. Replace the two right-aligned text buttons **Select All / Select None** with a single **master tri-state checkbox** placed at the start of the first column (leftmost, before the title).

## Decisions (confirmed with user)

- **Single master tri-checkbox** (not two checkboxes), reusing the existing `tri_checkbox` component (consistent visuals with the tree's row checkboxes).
  - `Checked` when all files selected; `Partial` when some selected; `Unchecked` when none.
  - Clicking toggles **all ↔ none** using the existing `SelectAll` / `SelectNone` messages — no new messages, no new i18n strings.
  - Disabled (greyed) when `enabled == false` (completed/removed task in the Details dialog), matching the tree rows.
- Master checkbox sits at the far left of the header row: `[master][title][Space:Fill][subtitle?]`.
- Top spacing via increasing the framed container's **top** padding (asymmetric).

## Scope / affected files

- `src/ui/components/torrent_file_list.rs` — the only file changed. Both dialogs call it unchanged (signature is unchanged).

## Implementation (single file: `torrent_file_list.rs`)

### 1. Import the tri-checkbox

Add to existing imports:

```rust
use crate::ui::components::tri_checkbox::{tri_checkbox, CheckState};
```

(`file_tree::descendant_indices` is already reachable via the existing `use crate::ui::components::file_tree::{self, FileTreeNode};`.)

### 2. Build the master checkbox and header

Compute overall selection state from the tree nodes + `is_selected`, then decide the toggle message at render time. Insert before building `header`:

```rust
let all_indices: Vec<u64> = nodes
    .iter()
    .flat_map(|n| file_tree::descendant_indices(n))
    .collect();
let selected_count = all_indices.iter().filter(|&&i| is_selected(i)).count();
let total_count = all_indices.len();

let master_state = if total_count > 0 && selected_count == total_count {
    CheckState::Checked
} else if selected_count > 0 {
    CheckState::Partial
} else {
    CheckState::Unchecked
};

let master_msg = if selected_count == total_count {
    on_select_none
} else {
    on_select_all
};

let mut master = tri_checkbox(master_state).size(16.0);
if enabled {
    master = master.on_toggle(move || master_msg.clone());
} else {
    master = master.on_toggle_maybe(None::<fn() -> M>);
}
```

Replace the current `header` construction (the `row![text(title)...]` block and the `.push(button(...SelectAll))/.push(button(...SelectNone))` chain) with:

```rust
let mut header = row![
    master,
    text(title).size(FONT_MEDIUM),
    iced::widget::Space::new().width(Length::Fill),
];
if let Some(sub) = subtitle {
    header = header.push(
        text(sub)
            .size(FONT_SMALL)
            .style(theme::style::text::secondary),
    );
}
header = header.spacing(SPACE_SM).align_y(Alignment::Center).width(Length::Fill);
```

Notes:
- `on_select_all` / `on_select_none` are `M` values (`M: Clone + 'a`); moving one into `master_msg` is fine (only one is used).
- `Tr::SelectAll` / `Tr::SelectNone` are no longer referenced here — remove those usages from the header. The `Tr` import stays (still used for `Tr::TorrentFiles`). The i18n catalog variants remain defined (no dead-code warning on a public enum).

### 3. Increase top spacing of the framed container

Change the container `.padding(...)` from the current `iced::Padding::from([PADDING_XS as f32, SPACE_MD])` to an asymmetric padding with a larger top:

```rust
.padding(iced::Padding {
    top: SPACE_LG,            // 8.0 — breathing room above the title
    right: SPACE_MD,          // 6.0
    bottom: PADDING_XS as f32, // 2.0 — unchanged
    left: SPACE_MD,           // 6.0
})
```

(Tunable: `SPACE_MD` = 6.0 if 8.0 feels too large.)

## Risks / notes

- **Column alignment**: the master checkbox is placed leftmost ("first column start") as requested. The tree's first-level checkboxes sit slightly right of the left edge (dir rows have a chevron button, file rows a 20px `CHEVRON_SLOT`), so the master won't be pixel-aligned with them — acceptable per the request. If exact alignment is later wanted, add a leading `Space` of ~`CHEVRON_SLOT` before the master.
- **Perf**: computing `all_indices` / `selected_count` is O(n) per frame, consistent with the existing `descendant_indices` calls in tree rendering. Acceptable.
- **Empty tree**: if `all_indices` is empty the master is `Unchecked` and toggling sends `SelectAll` (harmless no-op); the panel is only rendered when files exist in both callers.

## Validation

```bash
cargo build
cargo clippy --workspace   # no warnings
cargo fmt --check
```

Manual (`cargo run --`): open a torrent's Details → Files tab and the Add dialog → Torrent tab. Verify:
- Header has top spacing (not touching the frame edge).
- A master checkbox sits left of the title; shows ✓ when all files selected, − for partial, □ for none.
- Clicking toggles all ↔ none; disabled (completed/removed task) shows a greyed master.
- Subtitle (total size in the Add dialog) still renders on the right.
- Tree row checkboxes and long-name truncation unchanged.
