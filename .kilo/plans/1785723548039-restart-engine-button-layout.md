# Restart Engine Button Layout Fix

## Goal

Move the Restart Engine button so it sits immediately to the right of the Reset button, instead of being pushed to the far right of the settings actions row.

## Current Behavior

In `src/ui/settings_page.rs` the actions row is currently built as:

```rust
[Apply] [Reset] [Space::Fill] [Restart Engine]
```

This pushes the Restart Engine button all the way to the right edge.

## Desired Behavior

```rust
[Apply] [Reset] [Restart Engine]
```

The button should follow the Reset button with the standard `SPACE_2XL` spacing of the row.

## Change

### `src/ui/settings_page.rs`

Remove the spacer between the Reset button and the Restart Engine button:

```diff
     actions = actions.push(
         button(text(fluent.get(Tr::Reset)).size(FONT_BODY))
             .on_press_maybe(if dirty {
                 Some(Message::ResetSettings)
             } else {
                 None
             })
             .padding(PADDING_BUTTON_XL)
             .style(theme::style::button::secondary()),
     );
-    actions = actions.push(iced::widget::Space::new().width(Length::Fill));
     actions = actions.push(
         button(
             row![
                 icon::refresh().size(FONT_ICON),
                 text(fluent.get(Tr::RestartEngine)).size(FONT_BODY),
             ]
             .spacing(SPACE_SM)
             .align_y(Alignment::Center),
         )
         .on_press(Message::RestartEngine)
         .padding(PADDING_BUTTON_XL)
         .style(theme::style::button::secondary()),
     );
```

## Validation

1. `cargo fmt --check` passes.
2. `cargo clippy --workspace` passes with no warnings.
3. `cargo build` compiles.
