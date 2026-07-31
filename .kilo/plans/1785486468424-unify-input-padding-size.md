# 统一输入框 padding / size

## 目标
当前 6 处 `text_input` 与 1 处 `text_editor` 重复书写相同的 `.padding(x).size(13)`。在 `src/ui/theme.rs` 提取常量并新增 builder 辅助函数，统一使用。

## 现状（已确认）
| 调用点 | padding | size | style |
|---|---|---|---|
| `task_list.rs:109` search_input | 8 | 13 | `input::standard` |
| `add_dialog.rs:149` rename_input | 8 | 13 | `input::standard` |
| `add_dialog.rs:240` advanced_field | 8 | 13 | `input::standard` |
| `settings_page.rs:723` labeled_text_input | 8 | 13 | `input::standard` |
| `number_stepper.rs:144` stepper input | `[0, 10]` | 13 | `input::grouped` |
| `path_picker.rs:158` path input | `[0, 10]` | 13 | `input::grouped` |
| `settings_page.rs:741` labeled_editor (text_editor) | 8 | 13 | `text_editor::standard` |

不改动：`add_dialog.rs:98` URL 多行编辑器（padding 10 / size 14，设计上更大，保持现状）。

## 实施步骤

### 1. `src/ui/theme.rs` — 新增常量与辅助函数
在 `theme.rs` 顶层（与 `RADIUS_CARD` 等常量并列）新增：

```rust
pub const INPUT_SIZE: u16 = 13;
pub const INPUT_PADDING: iced::Padding = iced::Padding::new(8.0);
pub const INPUT_PADDING_GROUPED: iced::Padding = iced::Padding::from([0, 10]);
```

```rust
pub fn input_layout<'a, Message>(
    input: iced::widget::TextInput<'a, Message>,
) -> iced::widget::TextInput<'a, Message> {
    input.padding(INPUT_PADDING).size(INPUT_SIZE)
}

pub fn grouped_input_layout<'a, Message>(
    input: iced::widget::TextInput<'a, Message>,
) -> iced::widget::TextInput<'a, Message> {
    input.padding(INPUT_PADDING_GROUPED).size(INPUT_SIZE)
}

pub fn editor_layout<'a, Message>(
    editor: iced::widget::TextEditor<'a, Message>,
) -> iced::widget::TextEditor<'a, Message> {
    editor.padding(INPUT_PADDING).size(INPUT_SIZE)
}
```

注意：`TextInput`/`TextEditor` 的默认泛型（`Theme = iced::Theme`, `Renderer = iced::Renderer`）即可满足全部调用点；`Message` 泛型需显式声明。

### 2. 替换调用点（去掉原 `.padding(...)` 与 `.size(13)`，改为包裹辅助函数）

- `src/ui/task_list.rs:109`：`let search_input = theme::input_layout(text_input(...).on_input(...).width(...).style(...));` — 在 `text_input(...)` 后、链式调用最外层包裹。
- `src/ui/add_dialog.rs:149` rename_input：同上用 `theme::input_layout(...)` 包裹（先 `.width().style()`，最后 `.on_input()` 也可，保持原链式顺序，仅在 `text_input(...)` 处包裹）。
- `src/ui/add_dialog.rs:240` advanced_field：`theme::input_layout(...)`。
- `src/ui/settings_page.rs:723` labeled_text_input：`theme::input_layout(...)`。
- `src/ui/components/number_stepper.rs:144` stepper input：`theme::grouped_input_layout(...)`。
- `src/ui/components/path_picker.rs:158` path input：`theme::grouped_input_layout(...)`。
- `src/ui/settings_page.rs:741` labeled_editor：`theme::editor_layout(...)`（text_editor 的 `.padding(8).size(13)` 移除）。

包裹时注意：辅助函数接受 `TextInput`，需放在 `.on_input()` 等链式方法之后或之前皆可，只要链式调用类型一致。建议写法：

```rust
let search_input = theme::input_layout(
    text_input(&fluent.get(Tr::Search), search_query)
        .on_input(Message::SearchChanged)
        .width(Length::Fixed(220.0))
        .style(theme::style::input::standard),
);
```

### 3. 校验
```bash
cargo fmt --check
cargo clippy --workspace   # 不允许警告
cargo build
```

## 风险与注意
- `ice::Padding::from([0, 10])` 中的 `[0, 10]` 推断为 `[i32; 2]` 时 `Padding::from` 可能存在类型推断问题；若编译报错，显式写 `iced::Padding::new(0.0).horizontal(10.0)` 或 `iced::Padding::from([0u16, 10])`。
- 不要改动 `add_dialog.rs:98` 的 URL 编辑器。
- 不改动各调用点的 `.style(...)` 与 `.width(...)`。
- 引入 `iced::widget::TextInput`/`TextEditor` 类型后，确认无与 `iced::widget::text_input` 函数名的命名冲突。
