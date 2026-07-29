# 标题栏窗口控件图标垂直对齐修复

## Goal
右上角最小化 / 最大化 / 关闭三个窗口控件图标存在视觉高低落差、未共线居中。统一为 Lucide 图标字体 + 相同字号，消除落差。

## Root Cause（已核对源码 `src/ui/title_bar.rs:35-73`）
三个按钮各用系统默认字体（HarmonyOS Sans SC，`main.rs:30`）的不同 Unicode 字形、且字号不同：
- 最小化 `–`(U+2013 EN DASH) `.size(15)`
- 最大化 `▢`/`❐` `.size(13)`
- 关闭 `✕`(U+2715) `.size(14)`

问题链：
1. 各字形在自身 em-box 内墨迹位置不同：`–` 是标点，落在字身 x-height 中线（偏下）；`✕`/`▢` 为几何符号、近似 em-box 中心。
2. 每个按钮内层 `container(...).center_y(Length::Fill)` 只居中了「文本行盒」，而行盒内字形光学中心 y 各不相同 → 三者不共线 → 「高低落差」。
3. 字号不同(15/13/14) + 默认 `line_height(1.3)` 使各字形行盒高度不同(19.5/16.9/18.2px)，进一步放大偏移。
4. 字形取自系统字体，不同字符可能回退到不同 fallback 字体，跨平台不一致。

对照先例 `1785293197663-sidebar-icon-vertical-centering.md`：sidebar 已用 Lucide + `.line_height(1.0)` + `center_y` 解决同类问题；标题栏此处 `center_y` 已有，缺的是「统一字形度量」。

## The Fix
统一改用项目既有 Lucide 图标字体（`iced_lucide`，`main.rs:27` 已注册 `.font(icon::FONT)`，sidebar/task_list/details_dialog 均已使用）。Lucide 所有字形绘制在同一网格、共享一致 em-box 度量与光学居中，相同字号下必然共线。

### 1. `fonts/icons.toml` 新增三个标准 Lucide 图标
```toml
minus = "minus"
square = "square"
x = "x"
```
`build.rs:3` 已调用 `iced_lucide::build`，下次 `cargo build` 自动重新生成 `src/ui/icon.rs`（产出 `icon::minus()` / `icon::square()` / `icon::x()`）。

### 2. `src/ui/title_bar.rs`
- 顶部 import 增：`use crate::ui::icon;`（`text` import 仍被 left/mid/right_seg 拖拽区 `text("").size(1)` 使用，保留）。
- 删除 `let max_glyph = if maximized { "❐" } else { "▢" };`，改为：
  ```rust
  let max_icon = if maximized { icon::copy() } else { icon::square() };
  ```
  （`copy` 已在 toml 中，视觉为两个交叠方框 = 经典「还原」图标；`square` 为单方框 = 「最大化」。保留原 maximized 切换语义，1:1 替换 `❐`/`▢`。）
- 最小化按钮内容：`text("–").size(15)` → `icon::minus().size(15).line_height(1.0)`
- 最大化按钮内容：`text(max_glyph).size(13)` → `max_icon.size(15).line_height(1.0)`
- 关闭按钮内容：`text("✕").size(14)` → `icon::x().size(15).line_height(1.0)`
- 三者统一 `size(15)`、统一 `.line_height(1.0)`（与 `sidebar.rs` 图标居中先例一致，收紧行盒、使 `center_y` 居中可预测）。
- 外层 `container(...).center_x(Fill).center_y(Fill).width(Fill).height(Fill)` 与按钮 `.padding(0).width(Fixed(46.0)).height(Fill).style(window_control(...))` 保持不变。

## 不变量 / 无影响项
- 按钮宽 46、高 Fill、`window_control` 样式（hover 浅底、close 红 hover `theme.rs:365`）、拖拽区、`BAR_HEIGHT=38` 均不变。
- `left_seg`/`mid_seg`/`right_seg` 结构不变。
- 其余已用 Lucide 的页面不受影响。

## Validation
1. `cargo fmt --check`（失败先 `cargo fmt`）。
2. `cargo build`（确认 `minus`/`square`/`x` 三个 lucide 名称可解析）。
3. `cargo clippy --workspace`（须 0 warning）。
4. `cargo run --` 明 / 暗主题各一次目视：
   - 三个控件图标在同一水平中线、垂直居中，无高低落差。
   - 最大化 / 还原切换图标正确（单方框 ↔ 交叠方框）。
   - hover 底色与 close 红 hover 正常。

## Risks
- `minus`/`square`/`x` 为 Lucide 标准图标，bundled 字体含之，构建风险极低。若 `cargo build` 报某名称未找到：`square` 可换 `"square-dashed"` 或 `"rectangle"`；`minus`/`x` 无近义替代风险（均为基础图标）。
- 还原态复用 `copy` 为语义复用、视觉正确；如需独立命名可在 toml 加 `restore = "copy"` 别名（需确认 `iced_lucide` 允许同一 lucide 名映射多个本地名，否则直接调用 `icon::copy()`）。若嫌 `copy` 观感不符，可改试 `square-stack`（仅当其存在于 bundled lucide 且 `cargo build` 通过）。
- 兜底回退（仅当 Lucide 方案不可行）：保留 Unicode `✕`/`▢`，三者统一字号 + `.line_height(1.0)`，并对 `–` 单独加垂直像素偏移补偿；跨平台仍弱于 Lucide 方案。

## Out of Scope
- 不改 bar 高度、按钮宽度、样式、拖拽区。
- 不引入 SVG / 图片图标。
- 不改 details_dialog / close_dialog 等其它关闭按钮。
