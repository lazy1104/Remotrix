# UI 质感提升：按钮圆角/阴影/Hover/Press + 卡片立体感

## Goal
提升整体 UI 质感：为所有按钮加上圆角、微妙阴影（轻微立体感）、清晰的 hover 与 pressed 反馈；修复 task_list 工具栏图标无 hover 的廉价感；并为卡片/对话框增加阴影与细边框。**严格不变更任何现有配色**（accent / danger / secondary / 文本色等一律沿用 opaline palette 现有取值），只新增形状、深度、状态反馈。

## Constraints（用户明确要求）
- **颜色保持现状**：填充按钮底色仍取 `palette.<role>.base.color`，hover 仍取 `palette.<role>.strong.color`（与 iced 内置 `primary/secondary/danger` 完全一致），文字色取 `pair.text` / `background.base.text`。不得引入新色值、不得改 hue。
- hover/press 反馈允许的“颜色变化”仅限：(a) 沿用 iced 已有的 palette `strong` 明度档；(b) ghost/text 按钮使用中性白色叠层 `rgba(1,1,1,a)`（与现有 `sidebar_icon` 一致）；(c) pressed 在 base 上做轻微 `darken`（仅在 iced 本来没有 pressed 态的地方新增）。
- 不改布局、不改 padding/spacing、不改文案、不改 Message 协议。

## iced 0.14 API 事实（已核对源码 iced_widget-0.14.2 / iced_core-0.14.0）
- `button::Style` 字段：`background: Option<Background>`, `text_color: Color`, `border: Border`, `shadow: Shadow`（**非 Option**，默认 `Shadow::default()` 即无阴影）, `snap: bool`。用 `..Default::default()` 补齐。
- `container::Style` 字段：`text_color: Option<Color>`, `background: Option<Background>`, `border: Border`, `shadow: Shadow`。
- `Shadow { color: Color, offset: Vector, blur_radius: f32 }`；`iced::Vector::new(x,y)`；`iced::border::rounded(f32)` → `Border { radius }`。
- 现有内置样式语义（`iced_widget/src/button.rs`）：
  - `styled(pair)`：bg=`pair.color`，text=`pair.text`，`border::rounded(2)`，无阴影。
  - `primary/secondary/danger`：Active=base；Hovered=`<role>.strong.color`；**Pressed=base（无区分）**；Disabled=`scale_alpha(0.5)`。
  - `text`：text=`background.base.text`；Hovered 仅把 text `scale_alpha(0.8)`（变暗，无背景）→ **这就是工具栏图标质感差的根因**；Pressed=base。
- 结论：内置按钮 radius≈2、无阴影、hover 极弱、无 pressed。我们要在不改色的前提下补齐 radius/shadow/hover/press。

## Decisions
1. **集中化**：在 `src/ui/theme.rs` 的 `style::button` 模块新增自定义 `text` / `primary` / `secondary` / `danger` / `toolbar_icon`，并改造现有 `new_download`。替换全部 ~28 处 `button::text/primary/secondary/danger` 调用点为 `theme::style::button::*`。命名与 iced 平行，便于机械替换。
2. **填充按钮**（primary/secondary/danger/new_download）：radius=`RADIUS_BUTTON`(6)，shadow=`button_shadow()`（rgba(0,0,0,0.18), offset(0,1), blur 2）。Active=base；Hovered=`<role>.strong.color`（沿用 iced）；Pressed=`darken(base,0.15)` + shadow 收为 offset(0,0)/blur1（按下感）；Disabled=`scale_alpha(0.5)`。
3. **ghost 按钮**（text / toolbar_icon）：透明背景，text=`background.base.text`，radius=`RADIUS_BUTTON`，**无阴影**。Hovered=bg `rgba(1,1,1,0.08)`；Pressed=bg `rgba(1,1,1,0.14)`（与 `sidebar_icon` 既有白色叠层一致，明暗主题均可见且不改色）；Disabled=text `scale_alpha(0.5)`、无 bg。
4. **toolbar_icon(active)**：ghost 样式 + active 时 bg=`rgba(accent,0.18)`、text=accent（复用 `active_filter` 思路）。用于 task_list 顶部工具栏与卡片内操作图标。
5. **new_download FAB**：替换现有 `accent*1.1` 钳位 hover 为 `primary.strong.color`（palette 一致，避免过曝）；Pressed=`darken(accent,0.15)`；加 floating shadow `rgba(0,0,0,0.25), offset(0,3), blur 6`；radius=`RADIUS_PILL`(40)；text_color 保持 `background.base.text` 不变。
6. **card 容器**：`theme::style::card` 增加 (a) 阴影 `card_shadow()`=rgba(0,0,0,0.12),offset(0,2),blur6；(b) 1px 细边框 `border_color(t)`=`background.strong.color`；(c) radius 改用 `RADIUS_CARD`(8，与原值一致)。影响 task 卡片、所有对话框面板、sort 下拉浮层（均受益）。
7. **Radius tokens**：在 `theme.rs` 顶层加 `RADIUS_CARD=8.0 / RADIUS_BUTTON=6.0 / RADIUS_PILL=40.0 / RADIUS_PROGRESS=4.0`，全文件复用。

## Implementation Tasks（按序）

### T1. `src/ui/theme.rs` — 基础设施
- 顶层加 radius 常量：`RADIUS_CARD`, `RADIUS_BUTTON`, `RADIUS_PILL`, `RADIUS_PROGRESS`。
- 加私有 helper（模块内或 `style` 内）：
  - `fn lighten(c: Color, amt: f32) -> Color`（向白混合）
  - `fn darken(c: Color, amt: f32) -> Color`（向黑混合，`c*(1-amt)`）
  - `fn button_shadow() -> Shadow` / `fn button_shadow_pressed() -> Shadow`
  - `fn card_shadow() -> Shadow`
- 补充 import：`iced::{Background, Shadow, Vector}`, `iced::widget::button::{Style, Status}`, `iced::widget::container`, `iced::border`。

### T2. `src/ui/theme.rs::style::button` — 新增/改造样式
- 新增 `pub fn text()` / `pub fn primary()` / `pub fn secondary()` / `pub fn danger()` / `pub fn toolbar_icon(active: bool)`，返回 `impl Fn(&Theme, Status) -> Style`，按 Decisions 2/3/4 实现。
- 改造现有 `new_download()`：按 Decision 5 实现。
- 保持 `sidebar_icon`、`window_control` 不变（已自带 hover；可顺手把硬编码 radius 换成 `RADIUS_BUTTON`，可选）。

### T3. `src/ui/theme.rs::style::card` — 阴影+边框
- 按 Decision 6 更新 `card`：加 `shadow: card_shadow()`、`border: Border{ color: border_color(t), width: 1.0, radius: iced::border::rounded(RADIUS_CARD) }`，background 不变。
- `progress::task` 的 `border::rounded(4)` 改为 `RADIUS_PROGRESS`（可选，保持一致）。

### T4. 替换调用点（机械替换 `button::<x>` → `theme::style::button::<x>`）
完整清单（行号为当前参考）：

| 文件 | 行 | 旧 | 新 |
|---|---|---|---|
| task_list.rs | 32 | `button::text`（toolbar_btn，传 `active`） | `theme::style::button::toolbar_icon(active)` |
| task_list.rs | 50 | `button::text`（sort underlay） | `theme::style::button::toolbar_icon(sort_active)` |
| task_list.rs | 63 | `button::text`（asc/desc 按钮） | `theme::style::button::text` |
| task_list.rs | 204,205 | `button::text`（卡片 toolbar_icon 闭包） | `theme::style::button::toolbar_icon(false)` |
| task_list.rs | 228,236 | `button::text`（show_in_folder） | `theme::style::button::toolbar_icon(false)` |
| task_list.rs | 245,253 | `button::text`（copy_link） | `theme::style::button::toolbar_icon(false)` |
| task_list.rs | 262 | `button::text`（details） | `theme::style::button::toolbar_icon(false)` |
| task_list.rs | 276 | `button::text`（delete） | `theme::style::button::toolbar_icon(false)` |
| category_bar.rs | 52,92 | `button::text` | `theme::style::button::text` |
| close_dialog.rs | 21 | `button::danger` | `theme::style::button::danger` |
| close_dialog.rs | 26 | `button::secondary` | `theme::style::button::secondary` |
| close_dialog.rs | 40 | `button::text` | `theme::style::button::text` |
| about_dialog.rs | 45 | `button::secondary` | `theme::style::button::secondary` |
| details_dialog.rs | 105 | `button::secondary` | `theme::style::button::secondary` |
| add_dialog.rs | 74,97,126 | `button::secondary` | `theme::style::button::secondary` |
| add_dialog.rs | 132-136 | `button::primary` / `button::secondary` | `theme::style::button::primary` / `theme::style::button::secondary` |
| settings_page.rs | 84,499 | `button::primary` | `theme::style::button::primary` |
| settings_page.rs | 514,521,678 | `button::secondary` | `theme::style::button::secondary` |

注意：
- 各文件 `use iced::widget::{button, ...}` 保留（`button(...)` 构造器仍用）。不会产生 unused import。
- task_list.rs 内已有局部闭包名 `toolbar_icon`（L201），与新 `theme::style::button::toolbar_icon` 路径不同、不冲突；如担心可读性可把局部闭包重命名为 `icon_button`（可选）。
- 卡片内禁用态按钮（无 `on_press`）会进入 `Status::Disabled`，ghost 样式已处理（text 半透明、无 bg），表达“不可点”。

## Risks / Limitations
- **阴影裁剪**：iced 将 button 阴影绘制在 widget 布局范围内，padding 极小（如 4）的图标按钮若 blur>2 可能在边缘被裁。已选 blur=2/offset=1 的极弱阴影；如仍裁，把图标按钮 padding 提到 5 或将 ghost 按钮 shadow 设为 `Shadow::default()`（ghost 本就不带阴影，仅填充按钮需关注）。卡片 padding 较大（16/28），无裁剪风险。
- **dark 主题阴影可见性**：纯黑阴影在深色背景上偏弱；如需更强可把 `button_shadow`/`card_shadow` 的 alpha 按 `detect_dark` 加大（本计划默认保持简单、不强分主题，避免改色嫌疑）。
- **pressed `darken` 严格算“改色”**：但 iced 原生无 pressed 态，用户明确要求 pressed 效果；`darken(base,0.15)` 是最小可行的按压反馈，且仅在 pressed 瞬态出现，不影响默认配色。如用户更保守，可改为 pressed=base + shadow 收缩（完全不改色），作为 fallback。
- iced 无 scale 动画，“按下”只能靠明度/阴影收缩模拟，无法做真实形变。
- 阴影仅 wgpu 后端渲染（本项目即 wgpu），无兼容问题。

## Validation
1. `cargo fmt --check`（如失败先 `cargo fmt`）。
2. `cargo clippy --workspace`（须 0 warning）。
3. `cargo build`（离线可过，不涉及网络）。
4. `cargo run --` 手动核验（明/暗主题各一遍）：
   - 顶部工具栏图标：hover 出现圆角浅底，按下更深；sort 激活态有 accent 浅底。
   - 卡片内 5 个操作图标同样有 hover/press 反馈；禁用态（如完成态的 pause）半透明。
   - New Download 圆形 + 按钮：hover 不过曝、有浮动阴影、按下下沉。
   - 对话框（Add/About/Close/Details）面板有阴影+细边框，浮层有立体感；其中 primary/secondary/danger 按钮有圆角、阴影、hover/press。
   - Settings 的 Apply/Check Update/Retry/Browse 按钮、category_bar 过滤项 hover 有反馈。
   - 对比改动前后：默认态颜色应与改动前完全一致（仅多了圆角/阴影/状态）。
5. 切换浅色/深色主题各检查一次阴影与边框可见性。

## Out of Scope
- 系统托盘、动画/过渡（iced 0.14 不支持按钮过渡）、文本输入框/下拉等非按钮控件样式、tooltip 样式（保持 `container::rounded_box`）、进度条配色。
- 不改任何 palette 主题文件、不改 opaline 适配层。
