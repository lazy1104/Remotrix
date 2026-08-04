# 拆分 logo 资产：title-logo（侧栏） + 新 logo（About 对话框）

## Goal
- 将现有 `assets/logo.svg`（"Re Motrix" 字标，纯路径、无 `<text>`）重命名为 `assets/title-logo.svg`，继续用于应用左上图标（侧栏顶部）。
- 引入 `/home/caoyucong/Downloads/svg.svg`（512×512 的 R·M·O 彩色 logo）作为新 logo 资产，用于 `src/ui/about_dialog.rs`。

## Background (verified)
- `src/ui/components/logo.rs:10-12` 用 `include_bytes!("../../../assets/logo.svg")` 加载，并以 primary 色 tint（`Style { color: Some(primary) }`）。调用点：`sidebar.rs:13`（左上图标）、`about_dialog.rs:34`。
- 项目历史约定：SVG 资产中避免 `<text>`（旧计划 `1785674031974` 专门把字标转成纯路径）。新 svg 含 `<text font-family="'Andada Pro'">R·M·O</text>`，'Andada Pro' 未安装，需在入库前把文本展平为路径，保证跨机器渲染一致。
- `iced` 的 SVG 渲染链含 `usvg 0.45.1`（Cargo.lock 已固定，含 `text`/`system-fonts` 默认特性；`Tree::to_string(&WriteOptions)` 在 `preserve_text=false` 时输出路径化文本）。依赖均已在本地 registry 缓存，可离线构建一次性工具。
- `assets/logo.svg` 已被 git 跟踪；`icon.png`（窗口图标）不受影响。

## Decisions
- 新 About logo 保留原始配色（橙色描边 + 紫色 + 黑字），**不做** theme primary tint（与字标不同，字标继续 tint）。
- About 对话框显示为方形，新增尺寸常量（约 96px，居中）。
- 侧栏左上字标行为完全不变（tint + 40×24）。

## Tasks

### 1. 重命名侧栏字标资产
- `git mv assets/logo.svg assets/title-logo.svg`

### 2. 生成新的 `assets/logo.svg`（文本→路径）
- 复制源：`/home/caoyucong/Downloads/svg.svg`。
- 在 `/tmp/kilo/gen_about_logo` 建一次性 Rust 工具（**不提交仓库**），依赖 `usvg = "=0.45.1"`、`fontdb = "=0.23.0"`，`cargo build --offline`：
  1. `fontdb.load_system_fonts()` + `load_font_file("fonts/HarmonyOS_Sans_SC_Regular.ttf")`。
  2. 把输入 svg 中 `<text ...>` 的 `font-family` 改写为 `HarmonyOS Sans SC`（保证解析到已加载字体），再 `usvg::Tree::from_data`。
  3. `tree.to_string(&WriteOptions { preserve_text: false, coordinates_precision: 4, ..Default::default() })` 输出到 `assets/logo.svg`。
- 预期产物：矩形/描边保持不变，`R·M·O` 变成 `<path>` 字形（fill 黑色，stroke-width=0 无描边）。
- 回退方案：若离线构建/转换失败，直接 `cp /home/caoyucong/Downloads/svg.svg assets/logo.svg`（iced 运行时加载系统字体兜底渲染，跨机器字形不保证一致，作为已知降级）。

### 3. `src/ui/components/logo.rs`
- 现有 `view()`：`include_bytes!` 路径改为 `"../../../assets/title-logo.svg"`（其余不变）。
- 新增 `pub fn view_brand<'a, Message: 'a>(width: f32, height: f32) -> Element<'a, Message>`：`include_bytes!("../../../assets/logo.svg")`，**不加** `.style(...)`（保留原始配色）。

### 4. `src/ui/dims.rs`
- 新增 `pub const ABOUT_LOGO_SIZE: f32 = 96.0; // About 对话框 logo 边长（方形）`。

### 5. `src/ui/about_dialog.rs`
- 第 34 行 `logo::view(theme, SIDEBAR_LOGO_W, SIDEBAR_LOGO_H)` 改为 `logo::view_brand(ABOUT_LOGO_SIZE, ABOUT_LOGO_SIZE)`（移除 `theme` 参数不再需要）。

### 6. 验证
- `cargo build`
- `cargo clippy --workspace`（无警告）
- `cargo fmt --check`
- 手动 `cargo run`：侧栏左上仍是 "Re Motrix" 字标（跟随主题色）；打开 About，顶部显示新 R·M·O 彩色 logo（96px 居中），其余布局（Re Motrix 标题、可复制版本行、许可证行）不变。

## Risks
- 一次性工具离线构建依赖缓存 crates，若失败按回退方案处理。
- 展平后若字形位置/颜色与设计稿有出入，以实际渲染为准微调（仅改 `assets/logo.svg`，不涉及代码逻辑）。
