# Toolbar 新增按钮与工具图标一致化

## Goal
`task_list` 工具栏的「新增」(+) 按钮当前是 28×28 实心 accent 圆形 + drop shadow（`new_download()` 样式），比其余扁平 ghost 工具图标视觉上更大/更突出。将其改为与其余工具图标完全一致的 footprint，并保持永久 active 高亮。

## Root Cause（已核对源码）
- `src/ui/task_list.rs:93-107` `new_btn`：glyph `size(15)` 包在 `container(glyph).center(Length::Fixed(28.0))` -> 固定 28×28 盒；`padding(0)`；样式 `new_download()` = 实心 accent 填充 + `RADIUS_PILL`(40) 圆形 + drop shadow。
- `src/ui/task_list.rs:21-36` `toolbar_btn`：glyph `text(..).font(lucide_font).size(15)`（默认 `line_height` 1.3，行盒 ~19.5px）；`padding([6,8])`；样式 `toolbar_icon(active)` = 扁平 ghost（仅 hover 白底），`RADIUS_BUTTON`(6)，text_color `base_text`/`accent`。footprint ≈ 31.5px 高。
- 静止状态下其余图标只是 15px 小字 glyph，而 + 是 28px 实心圆盘 + 阴影 -> 视觉「更大」。
- 两者均已使用主题配色：`new_download` 读 `extended_palette().primary`；`toolbar_icon` 读 `primary` / `background.base.text`。

## Decision（用户已确认：方案 A — 扁平化）
将 + 按钮改为 `toolbar_icon(true)`，与其余工具图标同款 footprint，保持永久 active（accent 0.18 底色 + accent 字色）。
- 不保留 `new_download()` 实心圆 / 阴影。
- **不引入 `line_height(1.0)`**：必须与 `toolbar_btn` 完全一致（toolbar_btn 用默认 1.3），否则高度又会不一致（与上个 sidebar 任务不同，此处不能加）。
- **不需要 add_dialog 可见态**：`active` 是永久 `true`，非切换。`task_list::view` 签名不变，app.rs 调用点不变。

## The Fix — `src/ui/task_list.rs` `new_btn` 块（L93-107）
替换为与 `toolbar_btn` 同构、`active=true`：

```rust
let new_btn: Element<'a, Message> = {
    let glyph = text('\u{E13D}'.to_string()).font(lucide_font).size(15);
    let btn = button(glyph)
        .on_press(Message::OpenAddDialog)
        .padding([6_u16, 8])
        .style(theme::style::button::toolbar_icon(true));
    tooltip(
        btn,
        text(fluent.get(Tr::NewDownload)),
        tooltip::Position::Bottom,
    )
    .style(container::rounded_box)
    .into()
};
```

变更点：
1. 移除 `container(glyph).center(Length::Fixed(28.0))` 内层盒及 `padding(0)`。
2. `glyph` 构造与 `toolbar_btn` 完全一致（`size(15)`，默认 `line_height`，**不加** `line_height(1.0)`）。
3. `padding([6_u16, 8])` 对齐 `toolbar_btn`。
4. 样式 `new_download()` -> `toolbar_icon(true)`（永久 active：accent 0.18 底 + accent 字色）。
5. 保留 tooltip（`Tr::NewDownload` 文案、`Bottom` 位置）与 `container::rounded_box` tooltip 样式。

## 不变量 / 无影响项
- `toolbar_btn` 闭包不动（其余 5 个工具图标 + `sort_underlay` 不变）。
- tooltip 文案 `Tr::NewDownload`、位置不变。
- `task_list::view` 签名不变；`app.rs:877` 调用点不变。
- `new_download()` 样式函数（`theme.rs:384-424`）将变为未使用：crate 根有 `#![allow(dead_code)]`（`main.rs:1`），**不会**产生 warning，可保留备用；如需整洁可一并删除（非必须；删除时 `darken`/`scale_alpha`/`button_shadow_pressed` 仍被 `filled` 等引用，不会连带失效）。
- `container` import 仍被多处使用（L86/L152/L370），不会孤儿。
- sidebar 的 + 按钮（`sidebar.rs` `new_area`）不动。

## Validation
1. `cargo fmt --check`（失败先 `cargo fmt`）。
2. `cargo build`。
3. `cargo clippy --workspace`（须 0 warning）。
4. `cargo run --` 目视：
   - 工具栏 + 按钮与其余工具图标同高、同 padding，不再是大圆盘。
   - + 按钮呈 active 态（accent 底色 + accent 字色），与 sort 激活态视觉一致。
   - hover/press 反馈正常；tooltip「NewDownload」正常。
   - 其余工具图标、sort 下拉、task card 工具图标无变化。

## Out of Scope
- 不改 `toolbar_btn` / sort / task_card 任何样式。
- 不给 + 按钮加 add-dialog-open 切换态。
- 不改图标集、不改主题半径常量。
