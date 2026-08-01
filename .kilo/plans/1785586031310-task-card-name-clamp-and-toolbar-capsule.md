# 任务卡片名称两行截断省略 + 工具栏胶囊样式

## Goal
1. **名称不再按单词/字形边界断行**：当前 `Wrapping::WordOrGlyph` 会在 `][`、`&` 等位置断行（用户反馈：`[DMG&SumiSora]` 后即换行，浪费行宽）。改为 `Wrapping::Glyph` 连续排布。
2. **最多两行，超出省略**：名称最多显示 2 行，超出部分以 `…` 截断（省略号）。
3. **鼠标移入显示全名**：名称 hover 时 tooltip 显示完整文件名。
4. **工具栏胶囊化**：外包一圈外边框，左右端为圆形（胶囊），且位置靠顶部。
5. 保留上一轮已完成的速度 "0 B/s" 修复（`is_download_active() || speed > 0`），本次不触碰。

## 根因与约束（已验证）
- iced 0.14 `Text` 无 `max_lines` / 省略号（overrun/ellipsis）API：
  - `iced_core-0.14.0/src/widget/text.rs` 仅 `wrapping(Word/Glyph/None/WordOrGlyph)`，无 max_lines；
  - 底层 `cryoglyph-0.1.0` paragraph 同样无 `max_lines`/`overrun_strategy`；
  - 因此"两行截断 + …"需自研 widget。
- 可行性已验证：`iced::widget::text` 公开导出 `layout`、`draw`、`State`(= `paragraph::Plain`)、`Format`、`Style`、`Wrapping`；`iced::widget::core::text::{Renderer, paragraph::Plain}` 可达（`iced_widget` 第 7 行 `pub use iced_renderer::core`）。可在 `Widget::layout` 中用 renderer 实测行高并二分截断，`draw` 复用 `text::draw`。
- `layout::sized(limits, Fill, Shrink, ...)` 使 node 宽=可用宽、高=段落 min_bounds 高，可直接用 `node.size().height` 判断是否超行。

## Changes by file

### 1. 新建 `src/ui/components/truncated_text.rs`
自定义叶节点 widget `TruncatedText<'a>`（仿 `iced::widget::text::Text` 实现，约 130 行）：

```rust
pub struct TruncatedText<'a> {
    content: &'a str,
    max_lines: u16,              // default 2
    size: Option<Pixels>,
    font: Option<Font>,
    color: Option<Color>,        // None = 继承默认文字色（同 text() 现行为）
    line_height: LineHeight,
    width: Length,               // default Fill（卡片内占满剩余空间）
    wrapping: Wrapping,          // default Glyph（用户明确不要按单词换行）
}
// builder: size/color/font/line_height/width/max_lines/wrapping
// 自由函数 truncated_text(content) -> TruncatedText<'a>；impl From<TruncatedText> for Element
```

`impl Widget<Message, Theme, Renderer>`，其中 `Theme: iced::widget::text::Catalog`，`Renderer: iced::widget::core::text::Renderer`：

- **State**：`struct TruncState { paragraph: paragraph::Plain<Renderer::Paragraph>, last_input: String, last_width: f32 }`；`tag()`/`state()` 返回 `Tag::of::<TruncState>()` / `State::new(TruncState{ paragraph: Plain::default(), .. })`。
- **layout**（核心）：
  1. 构造 `text::Format { width: Length::Fill, height: Length::Shrink, size, font, line_height, align_x: text::Alignment::Left, align_y: alignment::Vertical::Top, shaping: Shaping::Auto, wrapping: self.wrapping }`。
  2. 缓存命中（`last_input == content && |last_width − bounds.width| < 0.5`）→ 直接 `text::layout(&mut state.paragraph, renderer, limits, &state.paragraph.content().to_string(), format)`（content 已存于 paragraph 内），返回 node，避免每帧重复二分。
  3. 用 `text::layout(..., content, ...)` 实测，`node.size().height` > `line_height.to_absolute(size).0 * max_lines` 则超行。
  4. 未超行 → 记录缓存，返回该 node。
  5. 超行 → 对**字符前缀长度**二分（`lo=0, hi=content.chars().count()`；候选 = 前 n 个字符 + `"…"`；可行条件 = 该候选 `text::layout` 实测高度 ≤ 2 行高；高度随 n 单调不减，二分成立），取最长可行前缀为最终显示串；记录缓存（`last_input`、`last_width`）后返回最终 node。
- **draw**：`text::draw(renderer, defaults, layout.bounds(), state.paragraph.raw(), &Style { color: self.color }, viewport)`。
- `size()` 返回 `Size { width: self.width, height: Length::Shrink }`。

在 `src/ui/components/mod.rs` 注册 `pub mod truncated_text;`。

### 2. `src/ui/theme.rs` — 新增胶囊容器样式
仿已有 `speed_hud_background`（L208-217）新增：
```rust
pub fn toolbar_capsule(t: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(t.extended_palette().background.strong.color.into()),
        border: iced::Border {
            color: super::border_color(t),
            width: 1.0,
            radius: super::RADIUS_PILL.into(),   // 左右全圆 → 胶囊
        },
        ..Default::default()
    }
}
```
（背景/边框色按视觉微调；`RADIUS_PILL=40` 对 ~27px 高容器即成胶囊。）

### 3. `src/ui/task_list.rs`
- **名称**（现 L222-225）：替换为
  ```rust
  let name = tip::standard(
      truncated_text(t.name.clone())
          .size(15)
          .max_lines(2)
          .wrapping(text::Wrapping::Glyph),
      text(t.name.clone()).size(12),
      tooltip::Position::Bottom,
  );
  ```
  即：2 行内按字形连续排布，超 2 行省略；hover 显示全名。width 保持 Fill（widget 内 default Fill）。
- **工具栏**（现 L312-319）：外包胶囊容器
  ```rust
  let toolbar = container(
      row![...5 个按钮...].spacing(2).align_y(Alignment::Center),
  )
  .padding([2, 6])
  .style(theme::style::toolbar_capsule);
  ```
- **卡片第一行**（现 L375）：`row![name, toolbar].align_y(Alignment::Top)`（工具栏贴顶部；名称 2 行时不再垂直居中）。
- 删除 `text` 对 name 的旧 `wrapping` 用法；`iced::widget::Space` 已删，勿复加（name 仍为唯一 Fill 子项）。

### 4. `src/ui/details_dialog.rs`（一致性）
- `key_value_row`（L144-149）value 由 `text(value).wrapping(WordOrGlyph)` 改为
  ```rust
  truncated_text(value).size(13).max_lines(2).wrapping(text::Wrapping::Glyph)
  ```
  长文件名在 640px 对话框中同样 2 行截断；短值（gid/dir/status/time）不受影响。

## Edge cases / failure modes
- 短名称：1 行、无省略；hover tooltip 仍显示全名（无害）。
- 极窄窗口：`"…"` 单独 1 行也可容纳（lo=0 可行），不会空卡片。
- 窗口 resize：缓存键含 `last_width`，宽度变化即重新截断。
- 性能：仅超行卡片在 layout 做 ~log2(87)≈7 次段落测量；缓存命中后每帧只 1 次 `layout`，可忽略。
- 多行名称抬高卡片：列表本就纵向滚动，可接受。
- 不触碰：页面顶部工具栏、速度 HUD、speed "0 B/s" 逻辑、详情对话框 Files/Activity 其余内容。
- 若 Glyph 使含空格名称出现中缀断行观感不佳，可回退该 widget 的 `wrapping` 参数为 `WordOrGlyph`（仅影响截断场景的断行点），不作为本次默认。

## Validation
1. `cargo build`
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. 手动（默认 921px 窗口 + 已有 87 字符种子任务 `[DMG&SumiSora][...][BIG5].mp4`）：
   - 名称单行排布，不再于 `[DMG&SumiSora]` 后断行；缩窄窗口 → 最多 2 行 + `…`，hover 显示全名。
   - 工具栏呈胶囊（外边框 + 左右圆角），与名称顶部对齐；5 个按钮完整可见。
   - 短文件名 URL 任务：无视觉回归。
   - 种子任务详情 → 概览页长文件名 2 行截断、hover 显示全名；活动页速度口径不变。
