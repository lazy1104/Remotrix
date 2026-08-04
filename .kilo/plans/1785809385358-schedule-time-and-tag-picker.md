# tag_picker 组件视觉与交互改造

## 背景与目标
上一阶段已把生效日期多选做成通用组件 `tag_picker.rs`（`Settings > Download > Speed Limits` 的「生效日期」）。本次针对其视觉与交互细节做调整，需求如下：

1. **tag 圆角**：tag 与触发器边框使用相同的圆角矩形（`RADIUS_BUTTON`，而非当前 `RADIUS_PILL` 胶囊）。
2. **顶部间隙**：第一行 tag 与边框顶部应有间隙（当前只有 `PADDING_GROUPED=1px`，太紧）。
3. **单行垂直居中**：只有一行时，tag 在触发器内视觉上垂直居中。
4. **靠左对齐**：tag 应靠左（当前 `align_x(Center)` 会把单行内容水平居中，产生大左空），改为 `align_x(Start)`。
5. **换行左对齐**：换行后第二行也应左对齐（当前每次换行居中的问题，由 `align_x(Start)` 一并解决）。
6. **开关图标**：打开 dropdown 的图标由 `chevron_down` 改为 `plus`，样式与 tag 相同（`chip()`，同边框背景）。
7. **dropdown 无 checkbox**：浮层去掉 checkbox，选中项用 active/focus 高亮（复用 `chip()` 高亮样式）。
8. **宽度一致**：浮层宽度与组件宽度一致（DropDown 已 `.width(width)`，与触发器同一 `width` 参数）。
9. **最大宽度**：在 `settings_page.rs` 调用处传一个最大宽度（固定宽度），避免组件太宽。

## 现状梳理（已确认）
- `tag_picker.rs`（105 行）：触发器 `container(tag_row).padding(PADDING_GROUPED).style(grouped_frame_state(false,false))`；tag 行 `row(tag_items).wrap().vertical_spacing(SPACE_XS).align_x(Alignment::Center)`；开关用 `button(icon::chevron_down()).style(text())`；浮层每项 `row![text(label), checkbox(checked).on_toggle(...)]`，外层 `container(overlay).padding(PADDING_DROPDOWN).style(card)`；`DropDown::new(...).width(width)`。
- `theme.rs:530` `chip()` 当前 `border: rounded(RADIUS_PILL)`。
- `icon::plus()` 存在（settings_page.rs:791 已用）。
- `settings_page.rs:673` 调用 `tag_picker(..., Length::Fill)`。
- 触发器样式 `grouped_frame_state`（背景 base + 边框 + `RADIUS_BUTTON`）即所需圆角矩形。

## 实现任务

### 1. `src/ui/components/tag_picker.rs` 调整
- **触发器内边距**：`container(...).padding(PADDING_GROUPED)` → `padding([6, 8])`（上下 6、左右 8），使顶部/左侧与边框有间隙；对称内边距保证单行时 tag 垂直居中。
- **tag 行对齐**：`align_x(Alignment::Center)` → `align_x(Alignment::Start)`（靠左、换行左对齐）。
- **开关按钮**：`chevron_down` → `icon::plus()`，样式 `theme::style::button::text()` → `theme::style::button::chip()`，padding `[2, 8]`，`on_press(on_dismiss.clone())` 不变。
- **浮层去 checkbox**：把每项从 `row![text(label), checkbox(...)]` 改为整行可点按钮：
  - 选中项：`button(text(label).size(FONT_MEDIUM).width(Length::Fill)).on_press(切换消息).width(Length::Fill).padding(PADDING_BUTTON_XS).style(chip())`（accent 高亮）。
  - 未选中项：同上但 `.style(text())`。
  - 注意 if/else 会返回不同 opaque 类型，需在 if/else 内分别完整构造 `button`（参照 `time_picker` 处理方式）。
  - 切换消息按需急切计算：`on_toggle(value.clone(), !checked)`。
- **移除 `Rc`**：因浮层改为急切计算 `on_press` 消息（不再存 checkbox 闭包），可删除 `use std::rc::Rc;` 及 `Rc::new`/`Rc::clone`；`on_toggle` 直接急切调用。
- DropDown 组装保持 `.width(width)`（浮层宽度 = 组件宽度）。

### 2. `src/ui/theme.rs` — chip 圆角
- `chip()` 中 `RADIUS_PILL` → `RADIUS_BUTTON`，使 tag/加号/选中项与触发器边框同为圆角矩形。

### 3. `src/ui/settings_page.rs` — 最大宽度
- 调用处 `Length::Fill` → `Length::Fixed(360.0)`（最大宽度，避免组件过宽；可按需调整）。

## 验证
- `cargo clippy --workspace`（无警告）。
- `cargo fmt --check`。
- `cargo build`。
- 手动：Settings>Download>Speed Limits 勾选「启用限速计划」：
  - 生效日期：tag 为圆角矩形、顶部/左侧有间隙、单行垂直居中、多选换行左对齐；
  - 加号图标打开浮层，浮层无 checkbox、选中项高亮、宽度与组件一致；
  - 组件宽度不超过 360px。

## 风险与注意
- **if/else 样式 opaque 类型**：浮层选项按钮需在 if/else 分支内完整构造，避免 `.style(if .. {picker_item()} else {text()})` 的 opaque 类型不匹配（前次已遇到）。
- **内边距取值**：`[6, 8]` 为建议值，若单行视觉偏矮可调大上下值（如 `[8, 8]`）。
- **「靠左没有间隙」解释**：按“当前居中导致的大左空应改为靠左对齐”理解，配合对称内边距同时满足顶部/左侧间隙；如用户本意是左侧零间隙，可把左右内边距调小，但会破坏整体观感，暂按对称处理。
- 保留 `chip()` 复用：tag、加号按钮、浮层选中项三处共用，保证视觉一致。