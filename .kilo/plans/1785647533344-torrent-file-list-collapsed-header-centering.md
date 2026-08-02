# Center collapsed torrent-file-list header vertically

## Goal

When the Add-dialog torrent file panel is **collapsed** (only the header row is shown), the header appears pushed up because the frame container's bottom padding (2px) is much smaller than its top padding (8px). Make the collapsed header vertically centered by giving the bottom padding the same value as the top.

Expanded behavior is unchanged.

## Root cause

`torrent_file_list::view` wraps its content in a `container` with a fixed asymmetric padding:

```
top:    SPACE_LG        // 8.0
right:  SPACE_MD        // 6.0
bottom: PADDING_XS as f32 // 2.0
left:   SPACE_MD        // 6.0
```

Expanded, this asymmetry reads as intentional (rule + tree fill the panel, extra top breathing room). Collapsed, only the header remains, so the 8px top gap vs 2px bottom gap makes the header sit off-center.

## Change (single file: `src/ui/components/torrent_file_list.rs`)

Make the container's `bottom` padding conditional on `collapsed` so collapsed state is vertically symmetric (top == bottom == SPACE_LG):

```rust
.padding(iced::Padding {
    top: SPACE_LG,
    right: SPACE_MD,
    bottom: if collapsed { SPACE_LG } else { PADDING_XS as f32 },
    left: SPACE_MD,
})
```

- Collapsed: top and bottom both `SPACE_LG` → header centered.
- Expanded: current padding preserved (`PADDING_XS` bottom).
- `SPACE_LG` is already an `f32` (8.0), so no cast is needed.

## Validation

- `cargo build`, `cargo clippy --workspace`, `cargo fmt --check`.
- Manual (`cargo run --`): Add dialog + a torrent → collapse the file panel → header (master checkbox + title + subtitle + collapse button) is vertically centered within the frame; expand again → panel height and spacing look exactly as before.

## Out of scope

- No layout/geometry changes to the expanded tree, rule, or scrollable.
- No i18n, message, or state changes.
