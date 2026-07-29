# Sidebar 圆形 hover + 中间栏图标

## Goal
两处 UI 质感改动：
1. **sidebar 图标 hover/active 背景改为正圆**：把全宽按钮改为居中 40×40 方块 tile，hover/active 背景半径=20（正圆）。
2. **中间栏（category_bar）每个文字项前加图标**：任务筛选 3 项 + 设置分类 6 项，共 9 个 Lucide 图标。

## Confirmed Decisions（用户已确认）
- sidebar：40×40 居中方块 + 正圆 hover（半径 20）。**新增独立 `sidebar_nav(active)` 样式**，不动 details_dialog 复用的 `sidebar_icon`。
- category_bar 图标映射（已验证 Lucide 名称存在于 iced_lucide 0.1.0 内置集）：
  - All -> `layers`，Downloading -> `arrow-down-to-line`，Completed -> `circle-check`
  - General -> `sliders-horizontal`，Download -> `download`，BitTorrent -> `magnet`，Ed2k -> `share-2`，Network -> `globe`，Advanced -> `wrench`
- 图标 size=15，**无显式 color**（与现有 label 文本一致，继承按钮/容器 text_color；激活时随 `active_filter` 容器着色路径与 label 相同）。

## Facts（已核对源码）
- 布局：`app.rs:904` 三列 row —— 左 sidebar(64px) / 中 category_bar(180px) / 右 content。`SIDEBAR_W=64.0`、`CATEGORY_W=180.0`（`src/ui/icons.rs`）。
- 图标生成：`build.rs` 调 `iced_lucide::build("fonts/icons.toml")`，按 toml 的 `key=fn名 / value=Lucide名` 生成 `src/ui/icon.rs`（含 `pub fn <key>() -> Text`）。**无效名构建时 panic 并提示相似名**，故安全。改 toml 后 `cargo build` 自动重新生成 icon.rs（按 hash 触发），无需手改 icon.rs。
- sidebar 当前按钮：`sidebar.rs:29-33`，`width(Length::Fill).padding([10,0])`，glyph size 20，style `sidebar_icon(active)`（radius=6）。`sidebar_icon` 被 `details_dialog.rs:63`(close) 与 `:86`(tabs) 复用 —— tabs 是宽文本按钮，加大半径会变胶囊，故必须隔离。
- `sidebar::view` 的 `page: Page` 参数当前未使用（pre-existing clippy `unused variable: page` 警告）。仅 Tasks / Settings 是页面；New / About 是开对话框（非页面）。
- `Column::align_x(Horizontal)` 存在（iced_widget-0.14.2 column.rs:127），用于居中子项。
- 40×40 + `padding([10,0])`：内容区 40×20 居中（上下各 10），glyph(size 20) 正好垂直居中；背景填满 40×40，半径 20 = 正圆。

## Tasks（按序）

### T1. `fonts/icons.toml` — 新增 9 个图标
在 `[icons]` 末尾追加（key=Rust fn 名，value=Lucide 名）：
```toml
layers = "layers"
download_arrow = "arrow-down-to-line"
circle_check = "circle-check"
sliders = "sliders-horizontal"
download = "download"
magnet = "magnet"
share = "share-2"
globe = "globe"
wrench = "wrench"
```
随后 `cargo build` 会重新生成 `src/ui/icon.rs`（新增 `layers/download_arrow/circle_check/sliders/download/magnet/share/globe/wrench` 九个 fn）。**不要手改 icon.rs。**

### T2. `src/ui/theme.rs::style::button` — 新增 `sidebar_nav(active)`
在 `sidebar_icon` 之后新增独立样式，逻辑与 `sidebar_icon` 相同（active=accent 0.25 底；Hovered/Pressed=rgba(1,1,1,0.08)；Disabled=无底），唯一区别：`border: iced::border::rounded(20.0)`（正圆，配合 40×40 tile）。复用已 import 的 `Style/Status/Color`。返回 `impl Fn(&iced::Theme, Status) -> Style + 'a`。**不修改 `sidebar_icon`**（details_dialog 继续用 radius 6）。

### T3. `src/ui/sidebar.rs` — 40×40 居中圆形 tile
- import：`use iced::{Alignment, Element, Length};`
- `icon_btn` 闭包内按钮（`sidebar.rs:29-33`）：
  - `.width(Length::Fill)` -> `.width(Length::Fixed(40.0))`
  - 增加 `.height(Length::Fixed(40.0))`
  - `.style(theme::style::button::sidebar_icon(active))` -> `.style(theme::style::button::sidebar_nav(active))`
  - 保留 `padding([10,0])` 与 `btn_content = container(glyph).center_x(Length::Fill).width(Length::Fill)`（glyph 自然垂直居中）。
- 居中：`col`（`sidebar.rs:60`）链上加 `.align_x(Alignment::Center)`。logo 容器 width=Fill 不受影响；Space 仅 height 不受影响。
- tooltip（Position::Right）不受影响。

### T4. `src/ui/category_bar.rs` — 文字前加图标
- import：增加 `use crate::ui::icon;`
- `make_filter`（Tasks 分支，`:34-59`）：闭包内按 `target` 取图标，row 前置 icon：
  ```rust
  let icon = match target {
      TaskFilter::All => icon::layers(),
      TaskFilter::Downloading => icon::download_arrow(),
      TaskFilter::Completed => icon::circle_check(),
  };
  // row: .push(icon.size(15)).push(text(label_text).size(14)).push(Space Fill).spacing(8).align_y(Center).width(Fill)
  ```
- `make_cat`（Settings 分支，`:81-99`）：按 `SettingsCategory` 取图标，row 前置 icon：
  ```rust
  let icon = match target {
      SettingsCategory::General => icon::sliders(),
      SettingsCategory::Download => icon::download(),
      SettingsCategory::BitTorrent => icon::magnet(),
      SettingsCategory::Ed2k => icon::share(),
      SettingsCategory::Network => icon::globe(),
      SettingsCategory::Advanced => icon::wrench(),
  };
  // row: .push(icon.size(15)).push(text(label).size(14)).spacing(8).width(Fill).align_y(Center)
  ```
- 调用点签名不变（图标在闭包内 match，无需改 `make_filter(...)` / `make_cat(...)` 调用）。

### T5（推荐，顺带清掉 pre-existing 警告）— sidebar 激活态接线
当前 `page` 参数未用（clippy `unused variable`）。把页面型按钮的 `active` 接到 `page`，使新 `sidebar_nav(true)` 的 accent 圆形高亮真正生效：
- `list_area`：`active: page == Page::Tasks`
- `sett_area`：`active: page == Page::Settings`
- `new_area` / `about_area`：保持 `false`（开对话框，非页面）。
> 若用户不想加该功能，改为把参数前缀 `_page` 以消警告；但推荐接线（低风险且补全新样式的 active 路径）。

## Risks
- **图标名构建期校验**：若笔误 Lucide 名，`cargo build` 在 build.rs panic 并打印相似名提示 —— 失败明显、易修。已逐个 grep 确认 9 个名称存在于 `iced_lucide-0.1.0/assets/unicode.html`。
- **正圆依赖方形**：`sidebar_nav` 半径 20 仅在按钮 40×40 时为正圆。T3 已固定 width/height=40，耦合成立。若日后改 tile 尺寸需同步改半径（或改用 `>=半边` 的大值，iced 渲染会钳到半边）。
- **激活态图标颜色**：图标无显式色，走与 label 完全相同的 text_color 路径（按钮 `theme::style::button::text` 设 base.text；`active_filter` 容器设 accent）。若发现激活项图标/文字未变 accent（iced 层级覆盖问题），属既有 label 行为，不在本次范围；如需强制可在 active 时给图标显式 `.color(accent)`，但会与 label 不一致 —— 暂不做，留待视觉复核。
- **clippy 0 警告**：T5 接线后 `page` 警告消除；若跳过 T5 需改 `_page`，否则违反 repo “0 warnings” 标准。

## Validation
1. `cargo fmt --check`（失败先 `cargo fmt`）。
2. `cargo build`（触发 build.rs 重新生成 icon.rs；校验 9 个图标名；离线可过）。
3. `cargo clippy --workspace`（须 0 warning —— T5 接线后 `page` 警告应消失）。
4. `cargo run --` 手动核验（明/暗各一次）：
   - sidebar 4 个图标为居中 40×40 圆形 tile；hover 出现正圆浅底；当前页面（Tasks/Settings）tile 有 accent 圆形高亮（T5）。
   - 中间栏任务筛选 3 项、设置分类 6 项文字前各有对应图标；激活项图标与文字着色一致。
   - details_dialog 的 close 按钮 / tabs 仍为原 radius 6（未受影响）。

## Out of Scope
- 系统托盘、动画/过渡、tooltip 样式、非按钮控件样式、进度条配色。
- 不改任何 palette/主题文件、opaline 适配层。
- 不改 details_dialog 的 `sidebar_icon` 用法。
