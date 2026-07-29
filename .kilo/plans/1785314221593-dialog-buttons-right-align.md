# Plan: Right-align dialog button rows

## Context
The app has 5 dialogs rendered as overlays (`src/app.rs:1117-1158`): `add_dialog`, `about_dialog`, `close_dialog`, `confirm_dialog`, `details_dialog`. The user wants all dialog buttons right-aligned.

The codebase already has an established right-alignment convention used by `add_dialog.rs` and `about_dialog.rs`: a leading `iced::widget::Space::new().width(Length::Fill)` as the first child of the button `row!`, plus `.width(Length::Fill)` on the row. This plan applies that same convention to the two dialogs whose button rows are currently left-aligned.

## Current state audit

| Dialog | File:Lines | Button row alignment | Action |
|---|---|---|---|
| add_dialog | `src/ui/add_dialog.rs:122-143` | Right (leading `Space::Fill` + row `width(Fill)`) | No change |
| about_dialog | `src/ui/about_dialog.rs:39-48` | Right (leading `Space::Fill` + row `width(Fill)`) | No change |
| details_dialog | `src/ui/details_dialog.rs:67-72` | Header close button already right-aligned; no footer action row; empty-state button is a centered single button (out of scope) | No change |
| close_dialog | `src/ui/close_dialog.rs:42-47` | **Left** (no leading spacer, no `width(Fill)`) | **Fix** |
| confirm_dialog | `src/ui/confirm_dialog.rs:43-49, 61-67, 75-80, 92-98` | **Left** (4 branches, none right-aligned) | **Fix** |

## Changes

### 1. `src/ui/close_dialog.rs` (lines 42-47)
Make the `buttons` row right-aligned by inserting a leading fill spacer and giving the row `width(Length::Fill)`.

Before:
```rust
let buttons = row![]
    .push(cancel_btn)
    .push(tray_btn)
    .push(close_btn)
    .spacing(10)
    .align_y(Alignment::Center);
```

After:
```rust
let buttons = row![]
    .push(iced::widget::Space::new().width(Length::Fill))
    .push(cancel_btn)
    .push(tray_btn)
    .push(close_btn)
    .spacing(10)
    .align_y(Alignment::Center)
    .width(Length::Fill);
```

Button order is unchanged: Cancel, Tray, Close (danger on the far right).

### 2. `src/ui/confirm_dialog.rs` (4 branches)
Apply the identical change (leading `Space::width(Length::Fill)` + `.width(Length::Fill)`) to each of the 4 `row!` branches:

- `DeleteTask` branch (lines 43-49): Cancel → RemoveRecord → DeleteFiles (danger, far right)
- `DeleteAll` branch (lines 61-67): Cancel → RemoveAllRecords → DeleteAllFiles (danger, far right)
- `ClearCompleted` branch (lines 75-80): Cancel → Confirm (danger, far right)
- `LeaveSettings` branch (lines 92-98): Cancel → Discard (danger) → Apply (primary, far right)

Each branch becomes:
```rust
row![]
    .push(iced::widget::Space::new().width(Length::Fill))
    .push(cancel_btn)
    .push(...)
    .push(...)
    .spacing(10)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
```

Button order within each row is unchanged; only alignment changes.

## Out of scope
- `details_dialog.rs` empty-state centered close button (`src/ui/details_dialog.rs:92-111`): this is a single centered button in a centered empty state, not a footer action row. Left intentionally centered.
- Native `rfd` file dialogs (`src/app.rs:1247, 1260`): OS-native, not styled by the app.

## Validation
1. `cargo fmt --check` — formatting must pass.
2. `cargo clippy --workspace` — no warnings.
3. `cargo build` — compiles.
4. Manual: open each affected dialog and confirm the button group hugs the right edge of the panel:
   - Close dialog (quit the app via window close button).
   - Confirm dialogs: delete a task (right-click), delete all, clear completed, leave settings page with unapplied changes.
   - Sanity-check already-right-aligned dialogs (Add download, About) remain right-aligned.
