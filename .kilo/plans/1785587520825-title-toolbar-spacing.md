# 任务卡片：标题与工具栏胶囊留间隔

## Goal
任务卡片第一行（截断标题 + 胶囊工具栏）当前无间距（`row` 默认 spacing=0），标题与工具栏贴在一起。给两者之间加一些水平间隔。

## Change
`src/ui/task_list.rs:387`（`content` column 内第一行）：
```rust
.push(row![name, toolbar].align_y(iced::alignment::Vertical::Top))
```
改为：
```rust
.push(
    row![name, toolbar]
        .align_y(iced::alignment::Vertical::Top)
        .spacing(8),
)
```

- **间隔值选 8**：与卡片内部 `column![].spacing(8)` 的垂直节奏一致；如需更疏可改 10–12。
- 名称仍为唯一 Fill 子项，胶囊工具栏 `Shrink` 不变；仅视觉间距，不影响截断/布局逻辑。
- 不触碰：`TruncatedText` widget、胶囊样式、其余行。

## Validation
1. `cargo build`
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. 手动：默认窗口下标题与工具栏胶囊之间有明显间隔；名称 2 行时工具栏仍贴顶部（`align_y` 保持 `Top`），短文件名无回归。
