# Sidebar 圆形 tile 图标垂直居中修复

## Goal
侧边栏 4 个导航图标的圆形 tile 内，glyph 偏下、未垂直居中。修复为真正垂直居中。

## Root Cause（已核对源码）
- `sidebar.rs:25-34` glyph 为 `text(..).size(20)`，**未设 line_height**。
- iced 0.14 默认 `LineHeight::Relative(1.3)`（`iced_core-0.14.0/src/text.rs:215-218` 实测）。故 size=20 的文本行盒高度 = `1.3 × 20 = 26px`。
- 按钮 `height(40).padding([10,0])` -> 内容区高 = `40 − 10 − 10 = 20px`。
- 行盒(26px) 放进 20px 内容区，iced 将子项贴内容区顶部布局，向下溢出 ~6px；Lucide glyph 位于行盒靠基线上部，导致可见 glyph 落在圆形下半部 -> "偏下"。
- `btn_content = container(glyph).center_x(Fill).width(Fill)` **只有水平居中，无垂直居中**（`center_y` 未调用），无法补偿。

## Scope（已核对，仅 sidebar）
- `details_dialog.rs:56-63`(close) 与 `:83-87`(tabs) 用 `sidebar_icon` 但均为 **shrink-fit**（无固定 height，按钮随文本+padding 自适应），行盒即按钮内容、无溢出，**不存在同一问题**。本次不动 details_dialog。

## The Fix — `src/ui/sidebar.rs` `icon_btn` 闭包（L25-34）
三处协同改动，消除溢出 + 显式垂直居中：

1. **glyph 加 `.line_height(1.0)`**（`impl From<f32> for LineHeight` 已存在，无需新 import）：
   ```rust
   let glyph = text(codepoint.to_string())
       .font(iced::Font::with_name("lucide"))
       .size(20)
       .line_height(1.0);
   ```
   行盒收紧为 20px = glyph em，去掉 1.3× 带来的 6px 多余高度。

2. **`btn_content` 容器加垂直居中**：
   ```rust
   let btn_content = container(glyph)
       .center_x(Length::Fill)
       .center_y(Length::Fill)
       .width(Length::Fill)
       .height(Length::Fill);
   ```
   `container::center_y` 实存于 `iced_widget-0.14.2/src/container.rs:150`。

3. **按钮 padding 改 0**，让容器获得完整 40×40 居中空间：
   ```rust
   .padding(0)
   ```
   注：按钮背景（`sidebar_nav` 的圆形底色/hover）绘制在整 40×40 widget 上，与 padding 无关，故改 padding **不影响圆形外观**，仅影响内容定位。

合成后：20px 行盒在 40px tile 中经 `center_y` 居中于 y=10..30，glyph 视觉中心 ≈ 20 = tile 中心。

## 不变量 / 无影响项
- `sidebar_nav` 样式、半径 20、active 接线（`page == Page::Tasks/Settings`）、`align_x(Center)` 列居中、tooltip 均不动。
- `details_dialog` 的 `sidebar_icon` 用法不动。
- `fonts/icons.toml`、`icon.rs` 不动。

## Validation
1. `cargo fmt --check`（失败先 `cargo fmt`）。
2. `cargo build`。
3. `cargo clippy --workspace`（须 0 warning）。
4. `cargo run --` 明/暗各一次目视：
   - 4 个 sidebar 图标在圆形 tile 内垂直居中（不再偏下）。
   - hover 圆形浅底、当前页面 accent 圆形高亮仍正常，圆形不变形。
   - details_dialog close/tabs 外观无变化。

## Out of Scope
- 不改 details_dialog、不改样式半径、不改图标集、不加动画。
