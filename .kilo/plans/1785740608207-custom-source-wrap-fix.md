# 自定义源列表换行方式改为逐字符换行

## Goal
自定义源添加列表的 URL 文本当前按单词换行（`text::Wrapping::Word`），改为普通（逐字符）换行。

## Context
- `src/ui/settings_page.rs:738-741`，自定义源卡片内的 URL 文本：
  ```rust
  text(url.clone())
      .size(FONT_SMALL)
      .width(Length::Fill)
      .wrapping(text::Wrapping::Word),
  ```
- iced 0.14 的换行枚举 `iced::widget::text::Wrapping` 变体：`Word`、`Glyph`、`WordOrGlyph`。
- 代码库已有先例使用 `text::Wrapping::Glyph`：`src/ui/task_list.rs:273`、`src/ui/details_dialog.rs:163`。`text` 经 glob 导入 `iced::widget::text`，路径写法可直接复用。

## Changes（`src/ui/settings_page.rs`）
1. `settings_page.rs:741`：`.wrapping(text::Wrapping::Word)` → `.wrapping(text::Wrapping::Glyph)`。
2. 无其他改动。卡片样式、删除按钮、对齐逻辑保持不变。

## Validation
- `cargo build`
- `cargo clippy --workspace`
- `cargo fmt --check`
- 手动：设置 → BitTorrent → 添加一个超长自定义源 URL，确认换行按字符进行，删除按钮仍在卡片内右对齐。

## Notes / Risks
- `Glyph` 使超长 URL 在无空格处也能逐字符折行，避免撑破卡片。改动仅一处，风险极低。
