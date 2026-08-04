# tag_picker 加号图标尺寸统一 + 生效日期提示信息对齐

## 目标
1. `tag_picker.rs` 中加号按钮的图标比 tag 标签大，导致背景比 tag 高，统一尺寸。
2. `settings_page.rs` 中「生效日期」的提示信息（ScheduleHint）移到 tag_picker 组件下方，且前面 label 留空（与控件列对齐）。

## 现状
- `tag_picker.rs:58`：`button(icon::plus().size(FONT_ICON))`，`FONT_ICON = 15`。
- tag 标签：`text(label).size(FONT_MEDIUM)`，`FONT_MEDIUM = 13`；tag 内 x 图标 `FONT_SMALL = 12`。tag 高度由 13px 文本决定。
- 加号按钮与 tag 同样 `padding [2, 8]`、`chip()`，故背景差异纯由图标高度（15 vs 13）造成。
- `settings_page.rs:691-693`：`ScheduleHint` 目前是 `enabled` 分支 column 内的普通 `text(...)`，左对齐于 label 列，未缩进对齐控件列。

## 实现任务

### 1. `src/ui/components/tag_picker.rs` 加号图标尺寸
- `icon::plus().size(FONT_ICON)` → `icon::plus().size(FONT_MEDIUM)`，与 tag 标签（13px）统一，背景高度一致。
- 其余（padding `[2, 8]`、`chip()`、`on_press(on_dismiss.clone())`）不变。

### 2. `src/ui/settings_page.rs` 提示信息对齐
- 将 `enabled` 分支末尾的：
  ```rust
  text(fluent.get(Tr::ScheduleHint))
      .size(FONT_SMALL)
      .style(theme::style::text::secondary),
  ```
  改为包在 `setting_row_auto` 中、label 留空：
  ```rust
  setting_row_auto(
      String::new(),
      text(fluent.get(Tr::ScheduleHint))
          .size(FONT_SMALL)
          .style(theme::style::text::secondary),
  ),
  ```
  使提示显示在 tag_picker 控件下方、与控件列左对齐，label 列（200px）留空。

## 验证
- `cargo clippy --workspace`（无警告）。
- `cargo fmt --check`。
- `cargo build`。
- 手动：Settings>Download>Speed Limits 勾选「启用限速计划」：
  - 加号图标背景与 tag 高度一致；
  - 生效日期提示信息显示在控件下方、label 列留空。

## 风险
- 无。改动仅涉及一处图标尺寸与一处提示包裹，均为局部视觉调整。