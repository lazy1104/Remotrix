# 原生输入控件圆角化

## 目标
为 iced 原生 `text_input`、多行 `text_editor`、下拉框 `pick_list` 增加圆角，使其与整体样式（`RADIUS_BUTTON = 6.0`）一致。**仅改圆角**：保留 iced 默认配色与状态逻辑（Active/Hovered/Focused/Disabled 的边框颜色等），只把 `radius` 从默认的 `2.0`（菜单 popup 默认 `0.0`）提升到 `6.0`。`pick_list` 的下拉弹出菜单亦圆角化为 `6.0`。

## 决策（已与用户确认）
- 视觉规格：**仅改圆角**，不改 iced 默认配色/状态。聚焦边框仍用 iced 默认 `primary.strong.color`。
- 弹出菜单：圆角用 `RADIUS_BUTTON = 6.0`。
- 分组控件内层 `text_input`（`number_stepper.rs`、`path_picker.rs`）保持 `input::grouped`（透明背景、无边框，外框 `grouped_frame_state` 负责渲染），不在本次范围。

## 实现依据（iced_widget 0.14.2 源码确认）
- `iedwidget/src/text_input.rs:1758` `pub fn default(theme, status) -> Style`，active border radius=2.0、width=1.0、color=background.strong；Hovered→border color=background.base.text；Focused→primary.strong.color；Disabled→bg=background.weak。
- `text_editor.rs:1460` `default` 同构（radius=2.0）。
- `pick_list.rs:917` `default`：text/bg/handle/placeholder 调色，border radius=2.0、width=1.0、color=background.strong；Hovered/Opened→primary.strong。
- `overlay/menu.rs:646` `default`：bg=background.weak、border width=1.0 **radius=0.0**、color=background.strong。
- `pick_list::PickList` 暴露 `.style(impl Fn(&Theme, Status) -> Style)` (line 288) 与 `.menu_style(impl Fn(&Theme) -> menu::Style)` (line 298)。
- iced 0.14 重导出：`iced::widget::{text_input, text_editor, pick_list}`（模块与同名 builder 函数共存，`use` 后 `module::Type`/`module::default(...)` 均可用）；`iced::overlay::menu`（`iced_widget::overlay::menu`，经 `iced lib.rs:625 pub use iced_widget::overlay::*` 暴露）。
- `iced::Border::radius: iced::border::Radius`，`impl From<f32>`，故 `s.border.radius = RADIUS_BUTTON.into()` 可行。

## 改动清单

### 1. `src/ui/theme.rs` — 新增标准样式函数（在 `pub mod style` 内）

在现有 `pub mod input`（约 481-495 行）中追加 `standard`：
```rust
pub fn standard(t: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let mut s = text_input::default(t, status);
    s.border.radius = super::super::RADIUS_BUTTON.into();
    s
}
```
（该模块已有 `use iced::widget::text_input;`，无需新增 import。）

新增 `pub mod text_editor`（紧随 `input` 模块之后）：
```rust
pub mod text_editor {
    use iced::widget::text_editor;

    pub fn standard(t: &iced::Theme, status: text_editor::Status) -> text_editor::Style {
        let mut s = text_editor::default(t, status);
        s.border.radius = super::super::RADIUS_BUTTON.into();
        s
    }
}
```

新增 `pub mod pick_list`（紧随 `text_editor` 之后）：
```rust
pub mod pick_list {
    use iced::widget::overlay::menu;
    use iced::widget::pick_list;

    pub fn standard(t: &iced::Theme, status: pick_list::Status) -> pick_list::Style {
        let mut s = pick_list::default(t, status);
        s.border.radius = super::super::RADIUS_BUTTON.into();
        s
    }

    pub fn menu(t: &iced::Theme) -> menu::Style {
        let mut s = menu::default(t);
        s.border.radius = super::super::RADIUS_BUTTON.into();
        s
    }
}
```
> 注意 `super::super` 解析：这些 fn 位于 `style::<mod>::fn`，`super::super` = `style` 模块的父 = `crate::ui::theme` 模块，`RADIUS_BUTTON` 定义在此（theme.rs:35）。`ice::widget::pick_list` 与 `iced::widget::pick_list(...)` builder 函数同名共存，`use` 后 `pick_list::default`/`pick_list::Style` 走模块命名空间，无冲突。

### 2. `src/ui/settings_page.rs` — 4 处调用点加 `.style`

- `labeled_text_input`（约 706-711）：`text_input("", value).on_input(...).width(Fill).padding(8).size(13)` 末尾链 `.style(theme::style::input::standard)`。
- `labeled_editor`（约 723-727）：`text_editor(content).on_action(on_edit).height(Fixed(80)).padding(8).size(13)` 末尾链 `.style(theme::style::text_editor::standard)`。
- `labeled_pick`（约 747-751）：`pick_list(options, sel, on_select).placeholder(&placeholder).width(Fixed(180))` 末尾链 `.style(theme::style::pick_list::standard).menu_style(theme::style::pick_list::menu)`。
- 速度单位 `pick_list`（约 832-833）：`pick_list(unit_opts, sel, move |o| on_unit(o.value)).width(Fixed(80))` 末尾链 `.style(theme::style::pick_list::standard).menu_style(theme::style::pick_list::menu)`。

（`settings_page.rs` 顶部已 `use iced::widget::{pick_list, text_editor, text_input, ...}` 与 `use crate::ui::theme`，无需改 import。）

### 3. `src/ui/add_dialog.rs` — 1 处 `text_editor` 加 `.style`

- `url_input`（约 62-67）：`text_editor(&state.url_editor).placeholder(placeholder).on_action(Message::UrlEditor).height(Fixed(120)).padding(10).size(14)` 末尾链 `.style(theme::style::text_editor::standard)`。

## 不改动
- `number_stepper.rs:125`、`path_picker.rs:157` 的内层 `text_input`（保持 `input::grouped`）。
- 任何 `theme.rs` 既有的 style 函数与配色常量。
- `setting_row` 的高度、分组控件 `grouped_frame_state`、`grouped_icon` 等。

## 验证
1. `cargo fmt --check && cargo clippy --workspace`（须无 warning）。
2. 运行应用，逐项视觉核对圆角（应与 NumberStepper/PathPicker 外框 `RADIUS_BUTTON=6` 一致）：
   - 设置页所有 `labeled_text_input` 字段（AllProxy、UserAgent 等）：圆角 + 聚焦/悬停边框颜色与原一致。
   - 设置页 `text_editor`（如有使用多行的设置项，如自定义 headers/body）：圆角。
   - 设置页所有 `pick_list`（语言、主题、速度单位等）：选择框圆角 6.0；展开弹窗菜单亦圆角 6.0，与选择框连贯。
   - 新建下载对话框 URL 多行输入框（AddDialog）：圆角。
3. 确认未误改分组控件外观（NumberStepper/PathPicker 仍为单外框 + 内部无边框）。

## 风险
- `text_input::default` / `text_editor::default` / `pick_list::default` / `menu::default` 必须经 `iced::widget::*` / `iced::overlay::menu` 公开可达；已确认 iced 0.14 重导出路径。若编译报路径不可见，回退为内联实现（复制各 `default` 函数体并改 radius），但优先用重导出以保持与 iced 默认配色同步（主题变更时自动跟随）。
- 仅修改 radius 不改其他字段，故主题切换（dark/light）下边框/背景配色仍由 iced 默认提供，保持一致。