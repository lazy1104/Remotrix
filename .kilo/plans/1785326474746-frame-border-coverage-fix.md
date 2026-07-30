# Plan: grouped_frame 边框被图标按钮背景覆盖的修复

## 背景与确诊

用户反馈：path_picker 组件外框在图标按钮所在区段不显示，怀疑被按钮背景覆盖。已用代码验证：

1. **边框确实存在**：`grouped_frame`（`src/ui/theme.rs:131`）`border.width = 1.0`、`color = border_color(t)`，由 `container(row).style(grouped_frame)` 渲染。

2. **确实被覆盖（根因为几何，非纯色彩）**：iced 的 `container` 在布局时**不按 border 宽度内缩内容** —— `iced_core-0.14.0/src/layout.rs:181` 的 `layout::positioned` 只按 `padding` 收缩内容区。当前 `padding=0`，故按钮 `height(Length::Fill)` 一直顶到 frame 顶/底外缘。而 `container::draw`（`iced_widget-0.14.2/src/container.rs:338`）先调用 `draw_background`（含 border 描边），再调用 `content.draw`（按钮）—— 按钮随后把不透明的 `weak` 背景四边形画在 border 之上，**覆盖掉按钮 x-区段那 1px 的顶/底边线**。

3. **输入框段边框可见的原因**：`style::input::grouped` 背景是 `Color::TRANSPARENT`（`theme.rs:477`），透明四边形不遮 border。

4. **分隔线段边框也可见的的原因**：分隔线本身即 `strong`（=border 颜色）不透明填充，等价衔接该段边框。

> 旁证（色彩放大问题，非主因）：边框 `strong`（lighten base 0.15）vs 按钮 `weak`（lighten base 0.10）仅差 0.05，残留更难辨识；但即便高对比，也会被按钮不透明 quad 盖掉——主因是几何覆盖。

## 决策（用户已选 A）

给 `grouped_frame` 的容器加 `.padding(1.0)`，使内容按边框宽度内缩；按钮不再到达/覆盖 border。

关键验证（确保无副作用）：
- `iced_core-0.14.0/src/renderer.rs:83-84`：*"The border is drawn on the inside of the Quad."* → border 描边占据 bounds 最内侧 1px（顶 y∈[0,1)）。`padding(1.0)` 使内容从 y=1 起 → 与 border 内沿**相接无重叠、无空缝**。
- 因此内部 `strong` 分隔线（仍为内容高度）顶/底正好与 `strong` 顶/底边框相接，形成**连续 strong 网格**，无 T 字接点暗点。
- `padding.rs:216`：`impl From<f32> for Padding`（uniform）→ `.padding(1.0)` 直接可用。

保留外观取舍（与 Motrix 内缝观感一致，可接受）：
- 内容高度由 36 降为 34（外框 36 不变）；按钮 Fill = 34。
- 仅圆角内、内容方角与圆弧之间留 base 色小三角——即已存在的"out-of-scope corner artifact"，不改变既有行为。

## 变更清单

- [ ] `src/ui/path_picker.rs`：在 `group` 构造处加 `.padding(1.0)`：
  ```rust
  let group = container(row)
      .width(Length::Fill)
      .height(Length::Fixed(36.0))
      .padding(1.0)
      .style(theme::style::grouped_frame);
  ```
- 不改 `theme.rs`（`grouped_frame`、`grouped_icon`、`separator` 维持上一轮 separator 回滚后的状态）。
- 不改调用方（`add_dialog.rs` / `settings_page.rs`）。

## 验证

- `cargo build`
- `cargo clippy --workspace`
- `cargo fmt --check`
- 视觉确认：外框顶/底边线在输入框、分隔线、**以及各图标按钮**区段全程连续可见；内部分隔线与边框相接成网格；按钮 hover/pressed 仍保留弱色高亮。

## 已知遗留（不在本次范围）

- 内部分隔线 `strong`(+0.15) 与按钮 `weak`(+0.10) 对比仅 0.05，分隔线本身仍偏淡。用户本次仅聚焦"外框被覆盖"，故不在本计划处理；若随后仍诉求，再单列方案（如分隔线改 `strongest`(+0.20) 或半透明覆盖发丝）。

## 风险

- `padding(1.0)` 依赖 iced 把 border 描边画于 Quad 内侧（已由 `renderer.rs:83` 注释确认）。若未来 iced 改为外扩/居中描边，padding 取值需重新评估为 border 外扩分量。