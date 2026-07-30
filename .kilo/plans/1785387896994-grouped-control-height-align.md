# 统一 number_stepper / path_picker 高度至原生 text_input 内禀高度

## 背景 / 问题
两个自定义组件把高度硬编码为 `Length::Fixed(36.0)` 并用 `Length::Fill` 撑满，导致它们在 `setting_row`（`settings_page.rs` 高度 `Fixed(36.0)`、`align_y Center`，行内如 `:657`、`:247`、`:767`）里**上下贴边**。

而 iced 原生控件在该行内是内禀高度后居中、留有边距：
- 原生 `text_input`（`labeled_text_input`：`.padding(8).size(13)`，默认 `LineHeight::Relative(1.3)`）= `13×1.3 + 8 + 8 ≈ 32.9px` → 行内 `~1.5px` 上下边距。
- 原生 `pick_list`（`labeled_pick`：默认 button padding `5/10`、text_size 默认 16）= `16×1.3 + 5 + 5 ≈ 30.8px`。

因原生 input（≈33）与原生 select（≈31）本身不等高，无法用一个常数同时等于两者。文字本身已因 `align_y Center` 垂直对齐，差异只在**外框高度/边距**。

## 决策
按"原生 text_input 内禀动态推导"：把两个自定义组件的外框高度从写死的 `36.0` 改为由公式 `size * line_height + vpad*2` 计算得到的常量（即 13×1.3 + 8×2 = 32.9），与原生 `labeled_text_input` 同源。之后若调整原生 input 的 `size`/`padding`，常量自动跟随。不动 `setting_row` 的高度（保持 36，作为留白行高），也不改其余原生控件。

## 改动点

### 1. `src/ui/components/mod.rs` — 新增共享常量
在文件顶部新增（按 AGENTS.md 约定，加一行非显然说明性注释）：
```rust
use iced::Length;

// 与 settings_page::labeled_text_input（text_input size=13、padding=8、默认 line-height 1.3）内禀高度同源
pub const NATIVE_INPUT_FONT_SIZE: f32 = 13.0;
pub const NATIVE_INPUT_LINE_HEIGHT: f32 = 1.3;
pub const NATIVE_INPUT_VPAD: f32 = 8.0;
pub const CONTROL_HEIGHT: f32 =
    NATIVE_INPUT_FONT_SIZE * NATIVE_INPUT_LINE_HEIGHT + NATIVE_INPUT_VPAD * 2.0; // ≈ 32.9
```

### 2. `src/ui/components/number_stepper.rs` — 用 CONTROL_HEIGHT
- `fn size()`（当前 `:301-303`）：`Size::new(self.width, Length::Fixed(36.0))` → `Length::Fixed(CONTROL_HEIGHT)`。
- `fn layout`（当前 `:311-318`）：两处 `Length::Fixed(36.0)` 以及 `limits.height(...)` 与 `limits.resolve(self.width, Length::Fixed(36.0), ...)` 全部替换为 `Length::Fixed(CONTROL_HEIGHT)`。
- 导入：`use crate::ui::components::CONTROL_HEIGHT;`（或全路径 `crate::ui::components::CONTROL_HEIGHT`，无需追加新 use）。

### 3. `src/ui/components/path_picker.rs` — 用 CONTROL_HEIGHT
- `view`（当前 `:217-224`）：`container(row).height(Length::Fixed(36.0))` → `Length::Fixed(crate::ui::components::CONTROL_HEIGHT)`。

## 不改动的部分
- `setting_row` / 各处 `Length::Fixed(36.0)` 行高保持 36（留白）。
- `labeled_text_input`、`labeled_pick`、`speed_labeled_input` 中的 `pick_list` 等原生控件配置保持不变。
- 两个组件内部的 `text_input(...).padding([0, 10]).size(13)` 保持不变（glyph 已垂直居中，无需改）。
- grouped frame 自身的 `.padding(1.0)` / `Padding::new(1.0)` / border 绘制逻辑保持不变。

## 验证
1. `cargo fmt --check`
2. `cargo clippy --workspace`（无 warning）
3. `cargo build`（离线成功）
4. `cargo run --` 打开设置页：让含 number_stepper / path_picker 的行与含 `labeled_text_input` 的相邻行对照，外框上下边距应一致（均约 1.5px），不再贴行顶/行底。与 `pick_list` 行对比，自定义组件仍比 select 高约 2px（属既有原生差异）。
5. 速限行（`speed_labeled_input`：number_stepper + pick_list 同行 `align_y Center`）检查外框高度对齐无错位。

## 风险 / 注意
- `CONTROL_HEIGHT ≈ 32.9` 是按当前字体度量与默认 line-height 1.3 推导；若将来改默认字体导致 1.3 行高变化，公式仍按 1.3 常量，需同步常量。
- 两个组件把 `CONTROL_HEIGHT` 作为 `Length::Fixed`，编译期常量，无运行期 layout 风险。