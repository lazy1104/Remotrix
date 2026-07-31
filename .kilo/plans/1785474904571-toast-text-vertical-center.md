# Toast 图标与文字垂直对齐

## 目标
修复 toast 卡片内图标与文字未垂直对齐的问题：单行文字情况下，16px 图标与 13px 文字应按各自中心对齐，toast 保持单行高度不变高。

## 根因（已核实）
`src/ui/components/toast.rs` `card()`（第 128-163 行）：

```rust
let icon = ... .size(16)...;          // 图标是图标字体 Text，16px（src/ui/icon.rs:176）
let icon_col = container(icon).align_y(Vertical::Top);
let message_col = text(&toast.message).size(13).width(Length::Fill);  // 13px Text

let mut content = row![icon_col, message_col]
    .spacing(8)
    .align_y(Vertical::Top);
```

- 图标并非 SVG，而是 `text(codepoint).font(lucide)` 的图标字体 `Text`，`.size(16)`；消息是 `.size(13)` 的 `Text`。
- 二者字号不同 → iced 中行盒高度不同（16px 行盒 > 13px 行盒）。
- row 与 icon_col 均为 `Vertical::Top`：两元素贴顶，16px 图标的垂直中心比 13px 文字的中心更低 → 图标视觉偏低，即“没对齐”。
- 单纯固定 toast 高度而不改交叉轴对齐，无法消除该中心偏移。

## 实现
单文件修改 `src/ui/components/toast.rs` `card()`：

1. 图标容器改为垂直居中：`container(icon).align_y(Vertical::Center)`（`Vertical` 已在第 3 行导入）。
2. 行交叉轴改为居中：`row![icon_col, message_col].spacing(8).align_y(Vertical::Center)`。

效果：图标与文字按各自行盒中心对齐；卡片为内容自适应高度（单行时即一行高），不会变高。多行文字时图标相对整块文字垂直居中（本次不要求处理多行，可接受）。

## 验证
- `cargo build`
- `cargo clippy --workspace`（无警告）
- `cargo fmt --check`
- 手动：触发任意 toast，确认单行情况下图标与文字垂直对齐；含关闭按钮时仍正常。

## 影响范围
- 仅 `src/ui/components/toast.rs` `card()` 一处（第 138、143 行）。
