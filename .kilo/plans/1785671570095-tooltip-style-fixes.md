# Tooltip 样式与字体修复计划

## 目标
1. 优化 tooltip 样式：当前 tooltip 背景 `background.weak` 与任务卡片背景（`theme::style::card` 同为 `background.weak`）完全一致，悬浮在卡片上时无法区分。添加边框（+阴影）使其在任意底层表面（页面、卡片、输入框组、侧栏）上都能清晰浮现。
2. 统一 task_list.rs 列表工具栏 tooltip 字体：`toolbar_btn` 与 `new_btn` 的 tooltip 文字目前用默认字号（16px），明显大于排序 tooltip 的 `FONT_SMALL`（12px），需对齐。

## 改动 1：`src/ui/theme.rs` — `style::tooltip`（约 377-387 行）
当前实现：
```rust
pub fn tooltip(t: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(t.extended_palette().background.weak.color.into()),
        text_color: Some(t.extended_palette().background.weak.text),
        border: iced::Border {
            radius: super::RADIUS_BUTTON.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
```
改为：保持背景/文字色不变，添加
- `border`: `color: super::border_color(t)`（即 `background.strong`），`width: 1.0`，`radius` 仍为 `RADIUS_BUTTON`
- `shadow: super::card_shadow()`（theme 模块内私有 `fn card_shadow`，子模块 `style` 可经 `super::` 访问）

与现有 `card`/`toast`/`capsule_pill` 的视觉语言一致（均使用 `background.strong` 边框）。iced `Tooltip` widget 自带默认 5px padding，文字不会贴边。

## 改动 2：`src/ui/task_list.rs` — 工具栏 tooltip 字号
- `toolbar_btn` 闭包（约 42 行）：`tip::standard(btn, text(tip), ...)` → `text(tip).size(FONT_SMALL)`
- `new_btn`（约 139-142 行）：`text(fluent.get(Tr::NewDownload))` → 加 `.size(FONT_SMALL)`

排序 tooltip（120-124 行）与任务卡内 `toolbar_icon`（287-291 行）已用 `FONT_SMALL`，改后列表内全部 tooltip 字号一致。

## 范围外（不修改）
- `sidebar.rs:40`、`settings_page.rs:228`、`path_picker.rs:178/194/215` 中未指定字号的 tooltip 保持现状（用户仅要求 task_list 工具栏）。

## 验证
- `cargo build`
- `cargo clippy --workspace`（无警告）
- `cargo fmt --check`
- 手动检查：任务卡片上悬浮按钮的 tooltip 有清晰描边/阴影；列表工具栏（全部开始/全部暂停等）tooltip 字号与排序 tooltip 一致。
