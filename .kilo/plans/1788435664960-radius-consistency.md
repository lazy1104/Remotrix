# 圆角统一提取与一致性 + 可复用 tooltip

## 背景

iced 0.14 的 `Theme` 是纯颜色容器（仅 `name` + `palette` + `extended`），**不包含 radius 配置**。圆角只能通过每个 widget 的 `.style()` 控制。所有 widget 默认硬编码 `radius: 2.0`（iced 内置 `container::rounded_box` 也是 2.0），与应用的 `RADIUS_BUTTON`（6.0）不一致。

`sidebar_nav` 使用字面量 `20.0`，未提取为命名常量。

## 修改清单

### 1. `src/ui/theme.rs`

#### a) 新增常量

在 `RADIUS_PROGRESS`（第 37 行）之后添加：

```rust
pub const RADIUS_NAV: f32 = 20.0;
```

#### b) 新增 `style::tooltip` 函数

在 `style` 模块添加（`pub fn active_filter` 之后，约第 167 行）：

```rust
pub fn tooltip(t: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        border: iced::Border {
            radius: super::RADIUS_BUTTON.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
```

#### c) 更新 `sidebar_nav` 圆角

| 行号 | 当前 | 新值 |
|------|------|------|
| 373 | `border: iced::border::rounded(20.0),` | `border: iced::border::rounded(super::super::RADIUS_NAV),` |
| 386 | `border: iced::border::rounded(20.0),` | `border: iced::border::rounded(super::super::RADIUS_NAV),` |

### 2. `src/ui/components/tooltip.rs`（新文件）

将 `tooltip(content, label, position).style(...).into()` 模式封装为可复用的函数：

```rust
use iced::widget::tooltip;
use iced::Element;

use crate::ui::theme;

pub fn standard<'a, Message>(
    content: impl Into<Element<'a, Message, iced::Theme, iced::Renderer>>,
    label: impl Into<Element<'a, Message, iced::Theme, iced::Renderer>>,
    position: tooltip::Position,
) -> Element<'a, Message, iced::Theme, iced::Renderer>
where
    Message: 'a,
{
    iced::widget::tooltip(content, label, position)
        .style(theme::style::tooltip)
        .into()
}
```

### 3. `src/ui/components/mod.rs`

新增模块声明：

```rust
pub mod tooltip;
```

### 4. `src/ui/task_list.rs`

替换所有 `tooltip(..., ..., ...).style(container::rounded_box).into()` 为 `tooltip::standard(..., ..., ...)`。

需要：
- 新增 import：`use crate::ui::components::tooltip;`
- 移除 `container` 的 import（如果不再需要，即该文件中没有其他 `container::` 用法）

| 行号 | 当前 | 新值 |
|------|------|------|
| 33–35 | `tooltip(btn, text(tip), tooltip::Position::Bottom).style(container::rounded_box).into()` | `tooltip::standard(btn, text(tip), tooltip::Position::Bottom)` |
| 99–105 | `tooltip(...).style(container::rounded_box).into()` | `tooltip::standard(..., ..., ...)` |
| 228–237 | `tooltip(...).style(container::rounded_box).into()` | `tooltip::standard(..., ..., ...)` |
| 247–257 | `tooltip(...).style(container::rounded_box).into()` | `tooltip::standard(..., ..., ...)` |
| 266–277 | `tooltip(...).style(container::rounded_box).into()` | `tooltip::standard(..., ..., ...)` |
| 280–293 | `tooltip(...).style(container::rounded_box).into()` | `tooltip::standard(..., ..., ...)` |

### 5. `src/ui/sidebar.rs`

新增 import：`use crate::ui::components::tooltip;`

| 行号 | 当前 | 新值 |
|------|------|------|
| 41–43 | `tooltip(btn, text(tip), tooltip::Position::Right).style(container::rounded_box).into()` | `tooltip::standard(btn, text(tip), tooltip::Position::Right)` |

### 6. `src/ui/components/path_picker.rs`

新增 import（`path_picker` 在 `components` 内，可直接用 `super::tooltip`，或统一用 `crate::ui::components::tooltip`）：

| 行号 | 当前 | 新值 |
|------|------|------|
| 174–177 | `tooltip(btn, text(...), tooltip::Position::Bottom).style(container::rounded_box).into()` | `tooltip::standard(btn, text(...), tooltip::Position::Bottom)` |
| 193–196 | `tooltip(...).style(container::rounded_box).into()` | `tooltip::standard(..., ..., ...)` |

## 验证

```bash
cargo clippy --workspace && cargo build
```
