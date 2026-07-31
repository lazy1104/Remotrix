# 速度 HUD 修复：折叠态图标居中 + 暂停即时归零

## 背景
上一轮实现了右下角全局速度胶囊 HUD 与每任务 `upload_speed` 数据链路。用户反馈两个问题：
1. **无速度（折叠态）时下载图标未居中**——图标贴在胶囊左上角。
2. **暂停任务时 HUD 速度慢慢降下来**——非瞬时归零，观感像 bug。

## 根因（已确认，非推测）

### 问题 1：折叠态图标未居中
`src/ui/speed_hud.rs:14-19` 折叠分支只有 `.width(44).height(44)`，未设置对齐。iced 0.14 `Container` 默认 `horizontal_alignment = Left`、`vertical_alignment = Top`（见 `iced_widget-0.14.2/src/container.rs:108-109` 及 `new` 默认值），因此 18px 图标被放在 44px 框的左上角。尺寸不隐含居中，须显式 `center_x`/`align_x`。

附带 latent bug：展开态 `icon_col`（`speed_hud.rs:21-24`）写的是 `.width(44).center_x(Length::Shrink).center_y(Length::Shrink)`。`center_x(length)` 内部调用 `self.width(length)`（container.rs:131-133），会把已设的 44 又覆盖回 `Shrink`，导致左块实际塌缩到图标自身宽度，44px 图标列未实现。

### 问题 2：暂停速度慢慢下降
HUD 显示值来自 `EngineEvent::GlobalSpeed`，由引擎每秒 `poll_client.get_global_stat()` 产生（`engine.rs` 轮询循环）。aria2 的 `getGlobalStat` 返回的 `downloadSpeed`/`uploadSpeed` 是**指数平滑均值**，非瞬时值；任务暂停（离开 active 集）后其报告值随滑动窗口在数个轮询周期内衰减到 0，而非瞬间置 0。代码侧无任何客户端平滑逻辑，衰减完全来自 aria2。

按任务速度：暂停任务离开 `tell_active()` 后不再被轮询，仅 Pause 通知触发一次 `tell_status` → 发出一次 Progress 后冻结。故“持续下降”只可能来自全局统计的衰减，确认是 HUD 侧。

aria2 native 行为本身合理，但用户已确认希望**客户端覆盖：无 Active 任务时 HUD 立即显示 0（折叠）**，忽略 aria2 仍在衰减的 `get_global_stat`。

## 实施步骤

### 1. `src/ui/speed_hud.rs` — 折叠态居中 + 展开 icon_col 修正
- 折叠分支（`download == 0 && upload == 0`）：
  将 `.width(44).height(44)` 改为显式居中并设定尺寸：
  ```rust
  container(icon::download().size(18).color(strong))
      .center_x(Length::Fixed(44.0))
      .center_y(Length::Fixed(44.0))
      .style(theme::style::speed_hud_background)
      .into()
  ```
  `center_x(L)` = `width(L).align_x(Center)`（container.rs:131-133），等同设宽 44 + 居中；`center_y` 同理。保持 `Length` import 被使用。
- 展开 `icon_col`（`speed_hud.rs:21-24`）：
  ```rust
  let icon_col = container(icon::download().size(18).color(strong))
      .center_x(Length::Fixed(44.0));
  ```
  删除多余的 `.center_y(Length::Shrink)`（行内垂直由外层 `row![...].align_y(Center)` 负责）。
  注意：移除 `Length::Shrink` 后确认 `Length` 仍因 `Length::Fixed(44.0)` 使用，import 无需删。
- 验证半径：`speed_hud_background` 用 `RADIUS_PILL(40)`，折叠态 44×44 框会被 clamp 成圆形；展开态更宽则呈胶囊。无需改样式。

### 2. `src/app.rs::view` — 无 Active 任务时 HUD 立即归零
当前位置 `src/app.rs` 中（替换上一轮 `let (dl, up) = state.global_speed.unwrap_or((0, 0));` 一行）：
```rust
let (dl, up) = if state
    .tasks
    .values()
    .any(|t| t.status == TaskStatus::Active)
{
    state.global_speed.unwrap_or((0, 0))
} else {
    (0, 0)
};
```
- `TaskStatus` 已在 `app.rs` 顶部 import（`use crate::task::{DownloadTask, TaskStatus};`），无需新增 import。
- 语义：有任意 Active 任务 → 显示 aria2 全局统计（含下载中实时速度；seeding-only 的 BT 任务仍为 Active，upload 照常显示）；无 Active → 强制 (0,0)，HUD 折叠为圆点，忽略仍在衰减的 `global_speed`。
- 时序：Pause 通知会把 `t.status` 置为 Paused（Progress 事件内同次 `update` 完成），下一次 `view` 渲染即满足“无 Active”→ 视觉近乎瞬时折叠。`global_speed` 字段本身不清空（仍被轮询更新），仅显示侧覆盖；`EngineStopped` 清空逻辑保持不变。
- 不改 `EngineEvent` / 引擎，纯展示覆盖。

### 3. `src/app.rs` Progress 处理器 — 暂停任务强制速度清 0（防御）
位置：`Message::Engine(EngineEvent::Progress {...})` 分支内，现有 `if total == 0 && t.total > 0 { ... } else { ... }` 之后、`state.dirty.insert(gid);` 之前，插入：
```rust
if t.status == TaskStatus::Paused {
    t.speed = 0;
    t.upload_speed = 0;
}
```
- 目的：即便 aria2 对刚暂停任务的 `tell_status` 返回残余平滑速度，任务卡也立即显示 “—”（`task_list.rs:314-318` 在 `t.speed == 0` 时渲染 “—”），避免暂停后任务卡残留旧速度再冻结。
- 不动 Waiting/Complete：aria2 对这两态本就报 0，无需覆盖。

## 不做 / 范围外
- 不改引擎轮询频率、不引入客户端速度平滑、不改 `get_global_stat` 调用点。
- 不新增 i18n 键（HUD 纯数字 + 图标）。
- 不改 DB / 持久化（上一轮已完成 `upload_speed` 链路）。
- `src/ui/components/slim_scrollable.rs` 既存 `Anchor` 未用 warning 不在本轮范围。

## 验证
1. `cargo fmt --check`
2. `cargo clippy --workspace`（无新 warning）
3. `cargo build`（离线可构建）
4. 运行观察：
   - 无任务/全部暂停/全部完成：右下为圆形胶囊，下载图标**几何居中**。
   - 启动一个下载并使其有速度：胶囊左展开，上=上传(strong)、下=下载(primary)，图标位于左侧 44px 列内居中。
   - 暂停该任务：HUD **瞬间**折叠为圆点（不等待 aria2 衰减）；被暂停任务卡速度列显示 “—”。
   - 切换 Tasks/Settings 页面：胶囊位置不变；打开任意对话框：对话框覆盖胶囊。