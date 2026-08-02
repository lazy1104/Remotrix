# 侧栏 logo 改为纯 SVG 矢量路径（无 <text>）+ 位置微调

## Context / 决策

- 用户确认：**直接用 SVG `<path>` 矢量路径绘制 R·M·O 字母**（不用 `<text>` 元素，与字体渲染完全无关，消除"字母偏大需反复调字号"的问题）。
- 设计保持：**单色线框**——圆角矩形描边 + 字母 R·M·O（字母间中点 `·`），全部使用主题 **primary base**（运行时用 iced `Svg` 组件的颜色滤镜统一着色，`Svg::style` → `Style { color: Some(primary) }`，渲染器会把所有非透明像素替换为主题色）。
- 字母字形来源：从应用内置 `fonts/HarmonyOS_Sans_SC_Regular.ttf` **一次性提取轮廓**生成 `assets/logo.svg`（与界面字体字形一致）；生成工具为一次性临时 Rust 程序（本机无 pip/fonttools，`ttf-parser 0.25.1` 已在本地 registry 缓存，可离线）。
- "字母偏大"解决：字母比例（cap 高度 ≈10px / 24px 高框）在生成 SVG 时一次性固定，运行时不需任何字号参数。
- 位置（用户反馈 logo 需高于中间栏标题）：中间栏（category_bar）标题顶部在内容区 20px（`PADDING_CATEGORY_BAR=[20,14]`）；侧栏 logo 当前顶部也在 20px（`PADDING_SIDEBAR=[12,0]` + `PADDING_SIDEBAR_LOGO=[8,0]`）。方案：`PADDING_SIDEBAR_LOGO` 由 `[8, 0]` 改为 `[2, 0]`，logo 顶部落到 14px，高于标题 6px。
- 已完成的（保留不动）：`components/mod.rs` 的 `pub mod logo;`、`app.rs` 的 `logo_handle` 清理、`sidebar.rs` 调用点、`dims.rs` 的 `SIDEBAR_LOGO_W=40.0` / `SIDEBAR_LOGO_H=24.0`。

## 任务清单

1. **生成 `assets/logo.svg`**（一次性工具，产物提交入库，工具本身不进入仓库）：
   - 在 `/tmp/kilo/gen_logo` 建临时 crate，依赖 `ttf-parser = "=0.25.1"`（本地缓存，`cargo build --offline` 可用）。
   - 读 `fonts/HarmonyOS_Sans_SC_Regular.ttf`，用 `ttf_parser::Face` 提取 `R`、`M`、`O`、`·`(U+00B7) 的字形轮廓（实现 `OutlineBuilder`，move_to/line_to/quad_to/curve_to/close 输出 SVG path `d` 字符串）；用 `glyph_hor_advance` / `units_per_em` 计算布局。
   - SVG 规格：`viewBox="0 0 40 24"`；目标 cap 高度 ≈10px（相对 24px 框约 42%）；按累计 advance 排布并整体水平、垂直居中；每字形输出 `<g transform="translate(x,y) scale(k)"><path d="..."/></g>`。
   - 圆角矩形描边：`<rect x="1" y="1" width="38" height="22" rx="8" fill="none" stroke-width="2"/>`（颜色随意，运行时整体着色为 primary base）。
   - 兜底：若字体无 U+00B7 字形，用 `<circle>` 圆点替代。
   - 写入 `assets/logo.svg`。

2. **`Cargo.toml`**：`iced` 的 features 增加 `"svg"`（引入 resvg/usvg，均已在 registry 缓存）。

3. **重写 `src/ui/components/logo.rs`**：
   - 用 `iced::widget::svg::{Handle, Style, Svg}`：
     ```rust
     Svg::new(Handle::from_memory(include_bytes!("../../../assets/logo.svg")))
         .width(Length::Fixed(width))
         .height(Length::Fixed(height))
         .style(move |_t, _s| Style { color: Some(primary) })
     ```
   - 签名保持 `pub fn view<'a, Message: 'a>(theme: &'a iced::Theme, width: f32, height: f32) -> Element<'a, Message>`；`primary = theme.extended_palette().primary.base.color`。
   - 删除 TEXT_SCALE/STROKE_WIDTH/RADIUS_SCALE 常量（比例已固化在 SVG 内）。

4. **`src/ui/dims.rs`**：`PADDING_SIDEBAR_LOGO` 由 `[8, 0]` 改为 `[2, 0]`（logo 抬升至中间栏标题之上；后续再微调只改此常量）。

5. **验证**：
   - `cargo build`（首次会编译 resvg，一次性成本，离线可用）
   - `cargo clippy --workspace`（零警告）、`cargo fmt --check`
   - 运行目视：圆角矩形描边 + R·M·O 全部为 primary base；字母较之前更小且居中；logo 顶部落位高于中间栏标题；切换深/浅色与强调色颜色实时跟随。

## 风险 / 边界

- resvg/usvg 编译时间增加（一次性）。
- 生成工具为临时 crate，不加入仓库依赖；若字形比例/字距不理想，调工具参数重新生成 SVG 即可，无需改运行时代码。
- `logo::view(theme, width, height)` 接口不变，后续可在 About 对话框等处复用。
