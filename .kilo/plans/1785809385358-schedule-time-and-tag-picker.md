# Schedule 时间选择 + 生效日期(tag 多选)组件改造

## 背景与目标
`Settings > Download > Speed Limits` 下的两个控件需要改造：

1. **时间选择**（`time_picker.rs`）：当前用 iced_aw 时钟表盘，改为直接给出固定选项的 dropdown 列表 `[00:00, 00:30, ..., 23:30]`（共 48 项，30 分钟间隔），点击即选中并关闭。
2. **生效日期**（`weekday_select.rs` 的使用处）：把「日期多选」改成一个可复用的 tag 多选组件 ——
   - dropdown 浮层要有背景色（参照 `path_picker.rs`/`task_list.rs` 用 `container(...).padding(PADDING_DROPDOWN).style(theme::style::card)` 包裹）；
   - 触发器本身是圆角矩形；
   - 已选项渲染为带关闭图标的 tag，多个 tag 超出宽度自动换行；
   - 点击 tag 的关闭图标即取消选中；
   - 提取为通用组件，后续可多处复用。

## 现状梳理
- `time_picker.rs:35` 提供 `time_picker(value, open, on_toggle, on_change)`，内部用 iced_aw 时钟 + 自绘 `TimePickerStateful` 处理 open 状态回填。app.rs:1444/1452 在 `SettingKey::ScheduleStart/End` 变化时已把 `*_picker_open` 置为 false（即选中即关闭），因此新实现只需发出 `SettingChanged(ScheduleStart/End, Text("HH:MM"))` 即可自动关闭，无需改 app.rs。
- `weekday_select.rs` 仅被 `settings_page.rs:690` 使用；`iced_aw` 还被 `main.rs:37` 用于注册字体，故保留依赖，仅移除 time_picker 里的 iced_aw 用法。
- `drop_down::DropDown` 已是通用浮层组件（已有背景可自带，浮层背景需在外层容器上加 `theme::style::card`）。
- 主题样式：`theme::style::active_filter`（accent 底 + accent 文字 + RADIUS_BUTTON）适合做选中 tag；`theme::style::grouped_frame_state`（背景 base + 边框 + RADIUS_BUTTON）适合做触发器圆角矩形。
- 尺寸常量 `RADIUS_PILL=40`、`PADDING_DROPDOWN`、`PADDING_GROUPED`、`SPACE_XS` 等已存在。

## 实现任务

### 1. 重写 `src/ui/components/time_picker.rs`
- 保留公开签名 `pub fn time_picker<'a, M>(value: &'a str, open: bool, on_toggle: M, on_change: impl Fn(String) -> M + 'static) -> Element<'a, M, iced::Theme, iced::Renderer>`，使 `settings_page.rs` 调用点不变。
- 删除 iced_aw 时钟/`TimePickerStateful` 相关代码（`use iced_aw::...` 移除）。
- 触发器：复用现有 `picker_button()` 样式（圆角矩形 + 背景 + 边框），内容 = `text(value)` + `icon::clock()`，`on_press = on_toggle.clone()`。
- 浮层：`container( slim_scrollable(column of 48 slot buttons) ).padding(PADDING_DROPDOWN).style(theme::style::card)`（保证有背景色）。
  - 48 个选项：`(0..48).map(|i| format!("{:02}:{:02}", i/2, (i%2)*30))`。
  - 每个 slot 用 `button(text(...)).on_press(on_change(slot))`，当前值（`value`）的 slot 高亮（可复用 `button::picker_item` 或基于选中态加粗/着色）。
  - `slim_scrollable` 需要放在 `DropDown` 内并设高度上限：`DropDown::new(...).height(Length::Fixed(~250.0))`。
- 组装：`DropDown::new(underlay, overlay, open).on_dismiss(on_toggle.clone()).alignment(drop_down::Alignment::Bottom).width(Length::Fixed(~150.0)).into()`。
- `on_change` 闭包发出 `SettingChanged(ScheduleStart/End, Text(slot))` → app.rs 已自动关闭浮层，无额外改动。

### 2. 新增通用 tag 多选组件 `src/ui/components/tag_picker.rs`
- 签名（泛型 value 类型）：
  ```rust
  pub fn tag_picker<'a, M, V>(
      options: &'a [(V, String)],   // (value, label) 候选
      selected: &'a [V],            // 已选 value 列表
      placeholder: &'a str,         // 无选中时占位文本
      open: bool,
      on_toggle: impl Fn(V, bool) -> M + 'a,  // (value, checked)
      on_dismiss: M,
      width: Length,
  ) -> Element<'a, M, iced::Theme, iced::Renderer>
  where V: PartialEq + Clone + 'a, M: 'a + Clone;
  ```
- **触发器（圆角矩形）**：`container(...)` 用 `theme::style::grouped_frame_state(false, false)`（背景 base + 边框 + 圆角），内容为一个 `row!().spacing(SPACE_XS).wrap().vertical_spacing(SPACE_XS)`：
  - 每个已选 value 查找其 label 生成一个 **chip button**：`row![ text(label).size(FONT_MEDIUM), icon::x().size(FONT_SMALL) ]`，`on_press = on_toggle(value.clone(), false)`，`padding([2, 8])`，样式用新增的 `theme::style::button::chip()`（见任务 4）。
  - 末尾追加一个 `button(icon::chevron_down()).on_press(on_dismiss.clone())` 作为开关箭头（圆角矩形触发器不整体套 button，避免嵌套 button 双触发；点击箭头开/关浮层）。
  - 无选中时先 `push(text(placeholder).size(FONT_MEDIUM))`。
  - 触发器 `width(width)`。
- **浮层（有背景色）**：`container( options 的 column，每项 `row![ text(label).size(FONT_MEDIUM), checkbox(checked).on_toggle(move |b| on_toggle(value.clone(), b)) ]`，7 项以内可不滚动 ).padding(PADDING_DROPDOWN).style(theme::style::card)`。
- 组装 `DropDown::new(underlay, overlay, open).on_dismiss(on_dismiss).alignment(drop_down::Alignment::Bottom).width(width).into()`。

### 3. `settings_page.rs` 生效日期改用 `tag_picker`
- 替换 `weekday_select(...)`（约 688–703 行）为 `tag_picker(...)`：
  - `options`: 由 `day_labels`（[String;7]）zip 1..=7 构造 `Vec<(u8, String)>`，配合 `.iter().map(...).collect()` 以临时 Vec 传入（函数借用期为当前 frame）。
  - `selected`: `&settings.speed_limit_schedule.weekdays`。
  - `placeholder`: `fluent.get(Tr::ScheduleDays)`。
  - `on_toggle`: `move |day, enabled| Message::Settings(SettingsMsg::ScheduleDayToggled { day, enabled })`。
  - `on_dismiss`: `Message::Settings(SettingsMsg::ToggleScheduleDaysMenu)`。
  - `width`: `Length::Fill`。
- 把外层 `setting_row(...)` 改为 `setting_row_auto(...)`（tag 换行后高度可变，不能锁 36px）。
- 删除约 677–687 行不再需要的 `summary` 计算逻辑。
- 更新顶部 import：移除 `weekday_select`，新增 `tag_picker`。
- 时间选择调用点（约 644–666）保持不变（签名兼容）。

### 4. 主题新增样式 `src/ui/theme.rs`
- 在 `style::button` 模块内新增 `pub fn chip<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a`：选中 tag 样式 —— 背景 `Color::from_rgba(accent.r, accent.g, accent.b, 0.18)`、文字/图标 `accent`、`border: rounded(RADIUS_PILL)`、hover 时加强（可 `0.28`）、`shadow` 默认。可参考现有 `toolbar_icon(active)` 的写法。
- （可选）`container` 触发器样式直接复用 `grouped_frame_state(false,false)`，无需新增；若需要 hover 反馈可新增 `tag_field(hovered)`，但非必需。

### 5. 清理
- 删除 `src/ui/components/weekday_select.rs` 及其 `mod.rs` 条目（`mod weekday_select;`）。
- `mod.rs` 新增 `pub mod tag_picker;`（`time_picker` 模块保留）。
- 确认无其他文件引用 `weekday_select` / `picker_button`（picker_button 仍在 time_picker 内使用，保留 `pub(crate)`）。

## 验证
- `cargo clippy --workspace`（无警告）。
- `cargo fmt --check`。
- `cargo build`。
- 手动：Settings>Download>Speed Limits 勾选「启用限速计划」后：
  - 时间选择点击弹出 48 项列表，选中后数值更新且浮层关闭；
  - 生效日期显示已选天 tag，点 x 可取消，多选换行，浮层有背景色。

## 风险与注意
- **嵌套 button 问题**：tag 触发器不整体套 `button`，改为 `container`（圆角矩形）+ 内部 chip/chevron 各自为 button，避免外 button 吞掉 chip 点击。
- **高度可变**：tag 换行使触发器高度随内容变化，调用处必须用 `setting_row_auto`（已列入任务 3）。
- **临时借用**：`tag_picker` 的 `options` 传入临时 Vec 时，其生命周期仅限当前 `view` 调用 frame，iced 每帧重建 view，属安全用法（与现有 `labeled_pick` 构造 Vec 一致）。
- 保留 `iced_aw` 依赖（`main.rs:37` 字体仍用）。
