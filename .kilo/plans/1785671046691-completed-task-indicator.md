# 已完成任务视觉标示（绿色进度条 + 对勾图标）

## Goal
在任务卡片中为 `TaskStatus::Completed` 提供明确的视觉确认。用户已选定方案：进度条改用 success 绿色 + 任务名旁加 circle_check 对勾图标。

## Changes (all in `src/ui/task_list.rs`)

### 1. 进度条 Completed 用 success 色
`task_card` 中 `bar_color` 的 match（约 line 422-426）增加一个分支：

```rust
let bar_color = match t.status {
    TaskStatus::Paused => theme::primary_weak(theme),
    TaskStatus::Error => theme::danger(theme),
    TaskStatus::Completed => theme::success(theme),
    _ => theme::primary(theme),
};
```

与现有 "Error → danger" 的状态色惯例对称；Completed 进度本就 100%，颜色是主要区分信号。

### 2. 名称旁加 circle_check 图标
`task_card` 内容区第一行 `row![name, toolbar]`（约 line 488-492）。将 `name` 包一层：当 `t.status == TaskStatus::Completed` 时在名称前插入 success 色对勾图标。

```rust
let name_marker: Element<'a, Message> = if t.status == TaskStatus::Completed {
    row![
        icon::circle_check().size(FONT_ICON).color(theme::success(theme)),
        name,
    ]
    .spacing(SPACE_SM)
    .align_y(Alignment::Center)
    .into()
} else {
    name.into()
};
```

然后 `row![name_marker, toolbar].align_y(iced::alignment::Vertical::Top).spacing(SPACE_2XL)`。

要点：
- 图标尺寸 `FONT_ICON` 与任务名一致（name 也是 `FONT_ICON`）。
- 与分类栏 Completed 筛选图标 `icon::circle_check()`（`src/ui/category_bar.rs:47`）视觉一致。
- 图标为纯装饰，不加 tooltip，无需 i18n 改动。

## Out of Scope
- 不改 status 徽章、卡片淡化、名称变色等其他方案。
- 不改 details_dialog / category_bar。

## Validation
```bash
cargo clippy --workspace   # 无警告
cargo fmt --check
```
手动验证：完成一个任务后，卡片进度条变绿、名称前出现绿色对勾；深色/浅色主题下 `theme::success` 均可辨。
