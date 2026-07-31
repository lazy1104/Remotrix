# Reusable Dialog component + optional header close (X) button

## Goal
Five dialogs (`about_dialog`, `close_dialog`, `confirm_dialog`, `add_dialog`, `details_dialog`) duplicate the same full-screen overlay shell, the card panel, and the right-aligned footer button row. Extract these into a reusable component in `src/ui/components/dialog.rs` so all dialogs share one consistent style. Additionally, the reusable Dialog gains an **optional close (X) icon button in the header (title row), per-dialog configurable** (`with_close`), shown on the right side of the title.

**Scope: style/layout only; no dismissal behavior change.** No click-outside (`mouse_area`) and no Escape-key handling (the iced `modal` example is only referenced for the overlay/stacking pattern, per decision we only unify styling, not dismissal behavior).

## Current duplication (source of truth)
- Overlay shell (all 5 dialogs):
  ```rust
  container(panel)
      .center_x(Length::Fill).center_y(Length::Fill)
      .width(Length::Fill).height(Length::Fill)
      .style(theme::style::overlay).into()
  ```
- Card panel (all 5): `container(column[...]).width(Length::Fixed(W)).padding(28).style(theme::style::card)`
- Footer row (about/close/confirm/add): `row![Space(Fill), ...buttons].spacing(10).align_y(Center).width(Fill)`
- Existing header X in `details_dialog.rs:57-64` (`close_btn`, `text('\u{E10B}')`, size 18, padding 6, `theme::style::button::sidebar_icon(false)`) — the new component's X matches its **style/size** but uses `icon::x()` (E1B2). To unify the glyph across all dialogs, also swap `details_dialog`'s X to `icon::x()` (one-line change, see step 2).

## Design

### 1. New file `src/ui/components/dialog.rs`
Add `pub mod dialog;` to `src/ui/components/mod.rs`.

**`overlay(content) -> Element<'a, Message>`**
Generic full-screen overlay shell. Takes any `Into<Element<'a, Message>>`, returns the centered full-screen `container(...).style(theme::style::overlay)`. Reused by every dialog, including `details_dialog`.

**`Dialog` builder** (common title / body / footer structure)
```rust
pub struct Dialog<'a, Message> {
    width: f32,                     // default 420.0
    spacing: f32,                   // default 16.0
    title: Option<String>,          // rendered by the component as text() size 20
    close: Option<Message>,         // None => no X button (hidden)
    body: Option<Element<'a, Message>>,
    footer: Option<Element<'a, Message>>,
}
```
Builder methods: `.width(w)`, `.spacing(s)`, `.title(String)` (owned, renders `text(t).size(20)` itself to unify title style), `.with_close(Message)` (shows the header X), `.body(el)`, `.footer(buttons)`, `.build() -> Element<'a, Message>`.

`build()` renders:
- **Header row** (only when `title` or `close` is `Some`): `row![title_text, Space::new().width(Length::Fill), close_btn].align_y(Alignment::Center)`, where `title_text = text(title).size(20)` (omitted if no title) and `close_btn` is rendered only if `close` is `Some`:
  ```rust
  button(icon::x().size(18).line_height(1.0))
      .on_press(close_msg)
      .padding(6)
      .style(theme::style::button::sidebar_icon(false))
  ```
  Matches the existing `details_dialog.rs:57-64` close button (lucide X, size 18, padding 6). Use `crate::ui::icon::x()` (`src/ui/icon.rs:151`), not a hardcoded codepoint.
- **Body**: passed raw (caller styles fully, e.g. secondary text / inputs / its own `column` with its own spacing).
- **Footer**: wrap as `row![Space::new().width(Length::Fill), buttons].align_y(Alignment::Center).width(Length::Fill)` (NOTE: wrapper adds **no** `.spacing`; the caller-supplied buttons row carries its own `.spacing(10).align_y(Alignment::Center)`). Caller passes the buttons row WITHOUT a leading `Space(Fill)` — e.g. close/confirm/add pass `row![cancel, download].spacing(10).align_y(Alignment::Center)`, about passes a bare `button(close)` element. Avoids nested `row.spacing(10)` double-spacing.
- Conditional children: the plain iced 0.14 `Column` has **no** `push_maybe` — push with `if let Some(x) = opt { col = col.push(x) }` instead.
- Inner `column![header, body, footer]` with `.spacing(spacing)` inside `container(...).width(Fixed(width)).padding(28).style(theme::style::card)`.

`overlay(dialog.build())` produces the final element. Generic over `Message: Clone + 'a`.

### 2. Refactor each dialog view (behavior unchanged except added X)

Each simple dialog calls `.with_close(<cancel/close message>)` so the X shows and acts as the cancel/close equivalent, **coexisting** with existing bottom buttons:

- **`about_dialog.rs`**: `Dialog::new().width(380.0).title(AboutTitle).with_close(Message::CloseAbout).body(<text-lines column>).footer(<CloseAbout button>).build()` wrapped in `overlay(...)`.
- **`close_dialog.rs`**: `Dialog::new().title(ConfirmCloseTitle).with_close(Message::CloseDialog(CloseDialogChoice::Cancel)).body(<body + tray-note>).footer(<cancel / tray / close row>).build()` in `overlay(...)`.
- **`confirm_dialog.rs`**: `Dialog::new().title(dynamic from action).with_close(Message::ConfirmCancel).body(dynamic).footer(<buttons per action>).build()` in `overlay(...)`. Keep existing per-`ConfirmAction` button logic.
- **`add_dialog.rs`**: `Dialog::new().width(520.0).spacing(14.0).title(NewDownload).with_close(Message::CancelAdd).body(<url/torrent/save/split column>).footer(<cancel / download row>).build()` in `overlay(...)`. Preserve width 520 / spacing 14.
- **`details_dialog.rs`**: uses `overlay(...)` ONLY (its header with close icon + tab bar + tab content doesn't fit title/body/footer). Wrap its existing custom panel container in `overlay(...)`. Additionally swap its header `close_btn` icon from `text('\u{E10B}')...` to `icon::x().size(18).line_height(1.0)` (`on_press(Message::CloseTaskDetails)`, same padding 6 / `sidebar_icon(false)`) so the X glyph matches the new component across all dialogs.

### 3. `app.rs`
No changes — the stacking logic (`stack![...]`) in `view()` already composes the returned overlay elements.

## Validation
- `cargo build`
- `cargo clippy --workspace` — no warnings allowed
- `cargo fmt --check`
- Manual: open each dialog (Add, About, Close, Confirm, Details) and verify visual parity (width, spacing, padding, title size, footer alignment) and that each simple dialog now shows a working X in its header (about→CloseAbout, close→cancel, confirm→ConfirmCancel, add→CancelAdd), while bottom buttons still work.

## Risks / notes
- Header X uses `icon::x()` (E1B2) everywhere, including the one-line swap in `details_dialog` (was E10B), so all dialogs show the same X glyph.
- Footer: caller brings `.spacing(10).align_y(Center)` on the buttons row; the component only adds the leading `Space(Fill)` + `width(Fill)` + `align_y(Center)` (no wrapper spacing) to avoid double-spacing. about's single close button is passed bare.
- The current dialogs insert small `Space` spacers between body and footer (about: height 8; close/confirm: height 4). The unified Dialog drops these in favor of the single `.spacing` value — a minor, intentional spacing unification; verify it looks acceptable in the manual check.
- Do not change `theme::style::card` / `theme::style::overlay`; they are the shared style anchors.
- `add_dialog` and `details_dialog` have custom widths/spacing — keep their values via `.width()` / `.spacing()` to avoid visual regressions.
