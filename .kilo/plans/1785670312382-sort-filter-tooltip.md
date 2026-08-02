# Add Dynamic Tooltip to Sort/Filter Button

## Goal
Add a tooltip to the sort/filter dropdown trigger button in `src/ui/task_list.rs` whose content is dynamically derived from the current filter state (`sort_field` + `sort_order`), so it always reflects what the list is currently sorted by.

## Current State
- The sort button (`sort_underlay`, `src/ui/task_list.rs:47-60`) is a bare `Button` with no tooltip, unlike every other toolbar button.
- `view()` receives `sort_field: SortField` and `sort_order: SortOrder`.
- Defaults (app.rs:145-146): `SortField::AddedTime`, `SortOrder::Desc`.
- `Tr` is `Copy`; `SortField`/`SortOrder` are `Copy`.
- Existing i18n keys already exist: `Tr::Sort`, `Tr::SortAsc`, `Tr::SortDesc`, `Tr::SortByAdded/Name/Size/Progress/Status` (all in en + zh-CN ftl).
- `tip::standard(content, label, Position)` (src/ui/components/tooltip.rs) returns `Element<'a, Message>`, which satisfies `DropDown::new`'s `U: Into<Element>` bound.

## Changes (all in `src/ui/task_list.rs`, `view()`)

1. After building `sort_underlay` (line 60), build the dynamic label:
   ```rust
   let sort_field_label = match sort_field {
       SortField::AddedTime => Tr::SortByAdded,
       SortField::Name => Tr::SortByName,
       SortField::Size => Tr::SortBySize,
       SortField::Progress => Tr::SortByProgress,
       SortField::Status => Tr::SortByStatus,
   };
   let sort_order_label = match sort_order {
       SortOrder::Asc => Tr::SortAsc,
       SortOrder::Desc => Tr::SortDesc,
   };
   let sort_tip = format!(
       "{}: {} · {}",
       fluent.get(Tr::Sort),
       fluent.get(sort_field_label),
       fluent.get(sort_order_label)
   );
   ```
   Result examples: EN `Sort: Name · Descending`, ZH `排序: 名称 · 降序`.

2. Wrap the underlay and pass the wrapped Element to the dropdown (line 101):
   ```rust
   let sort_dropdown = drop_down::DropDown::new(
       tip::standard(
           sort_underlay,
           text(sort_tip).size(FONT_SMALL),
           tooltip::Position::Bottom,
       ),
       sort_overlay,
       sort_menu_open,
   )
   .on_dismiss(Message::CloseSortMenu)
   .width(Length::Fixed(170.0));
   ```

No changes needed to imports (`tip`, `text`, `tooltip`, `FONT_SMALL` already used in this file) or i18n files.

## Behavior Notes / Risks
- While the dropdown menu is open, `DropDown::overlay` returns the menu overlay instead of the underlay's tooltip overlay, so the tooltip is suppressed during an open menu. This is acceptable and consistent.
- Tooltip text updates automatically on each rebuild since it is recomputed from `sort_field`/`sort_order` every `view()` call.
- No fluent argument plumbing needed; `Fluent::get` takes only a `Tr` key, and existing keys are reused.

## Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings allowed)
- `cargo fmt --check`
- Manual: hover the sort button with default state (expect "Sort: Added Time · Descending" / "排序: 添加时间 · 降序"), then change sort field/order and re-hover to confirm the tooltip text updates.
