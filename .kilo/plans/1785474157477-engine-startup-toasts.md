# 引擎启动状态 Toast 通知

## 目标
应用启动时，根据 aria2-next 引擎启动状态显示 toast：
- 尚未下载 → 信息提示「下载中」**（不自动关闭，直到下载完成或失败才关闭）**
- 已下载、正在启动 → 信息提示「启动中」
- 启动成功 → 成功提示
- aria2 出错 → 错误提示

## 现状
- toast 基础设施已存在：`src/ui/components/toast.rs` 的 `Toast`/`ToastKind`，`Message::ShowToast` 处理位于 `app.rs:1125`，`push_toast` 位于 `app.rs:1467`（含每位置 6 条上限与淘汰）。目前无任何地方产生 toast。
- `Toast::new(kind, msg)` 默认 `close_after = Some(3s)`、`show_close = false`、`position = BottomRight`；可用 `.close_after(Option<Duration>)` 覆盖。
- 引擎事件已包含所需信号：
  - `EngineEvent::Aria2Status { stage, message }`：`aria2_fetcher.rs` 经 `emit_status` 发出 `"downloading"`（会连发两次：查_release / 下载中）、`"verifying"`、`"ready"`（扫描命中 / 下载完成）。
  - `EngineEvent::EngineReady`：sidecar 连接成功。
  - `EngineEvent::Aria2FetchFailed { error }`：启动失败。
  - `EngineEvent::EngineDegraded { reason }`：sidecar 不可用时收到命令（运行期信号，非下载失败）。

## 关键设计决策
1. **持久化 toast 必须直推并持有 id**：`Message::ShowToast` 在其 handler 内部用 `state.next_toast_id` 分配 id，调用方无法可靠获知 id（派发与处理之间可能有其它消息推进计数器），因此无法在之后按 id 关闭。改为新增 `spawn_toast` 直推 helper，自行分配并返回 id。
2. **「下载中」toast 持久化**：`close_after = None`（无自动关闭、无关闭按钮），由 `downloading_toast_id: Option<u64>` 跟踪；在下载完成（`"ready"` / `"starting"`）或失败（`Aria2FetchFailed`）时按 id 从 `state.toasts` 移除。`Option::take` 保证幂等。
3. **不单独显示 verifying toast**：校验仍属「获取二进制」过程，复用同一条持久化「下载中」toast 即可，避免多条。
4. **`downloading_toast_id.is_none()` 同时充当去重**：`"downloading"` 阶段连发两次时只在首次创建 toast。
5. **`EngineDegraded` 不弹 toast**：它在 sidecar 缺失时对每条用户命令都会触发，弹 toast 会刷屏；仅 `Aria2FetchFailed` 代表启动/下载失败，弹错误 toast。`EngineDegraded` 维持原行为（只设 `aria2_fetch_error`）。
6. **新增 `"starting"` 阶段**：`ensure_aria2_next` 在缓存命中 / `ARIA2_BIN` 路径下不发任何状态，UI 无法稳定触发「启动中」。在 `boot()` 里 `ensure_aria2_next` 返回后、`Sidecar::spawn` 前统一发 `"starting"`，覆盖所有路径。

## 实现步骤

### 1. `src/engine.rs` — 发出「starting」阶段
在 `boot()` 中，`ensure_aria2_next(event_tx).await?` 成功后、`Sidecar::spawn` 前发送：
```rust
let (bin_path, applied) = crate::aria2_fetcher::ensure_aria2_next(event_tx).await?;
let _ = event_tx.send(EngineEvent::Aria2Status {
    stage: "starting".to_string(),
    message: "Starting aria2-next engine...".to_string(),
});
let mut sidecar = Sidecar::spawn(&bin_path, config).await?;
```
（engine.rs 无 `emit_status`，直接 `event_tx.send`。）

### 2. i18n — 新增键
- `src/i18n.rs`：`Tr` 枚举加 `EngineStarting` / `EngineStarted` / `EngineStartFailed`，并加 `key()` 分支：
  - `EngineStarting` → `"engine-starting"`
  - `EngineStarted` → `"engine-started"`
  - `EngineStartFailed` → `"engine-start-failed"`
- `i18n/locales/zh-CN/main.ftl`：
  - `engine-starting = 正在启动 aria2-next 引擎`
  - `engine-started = aria2-next 引擎启动成功`
  - `engine-start-failed = 引擎启动失败`
- `i18n/locales/en/main.ftl`：
  - `engine-starting = Starting aria2-next engine`
  - `engine-started = aria2-next engine started`
  - `engine-start-failed = Engine failed to start`

### 3. `src/app.rs` — 产生与管理 toast
- 导入：`use crate::i18n::{Fluent, Locale, Tr};`、`use crate::ui::components::toast::{Toast, ToastKind};`（`Duration` 已在文件顶部导入）。
- `Remotrix` 新增字段（init：`None` / `false`）：
  - `downloading_toast_id: Option<u64>` — 持久化「下载中」toast 的 id（兼去重）。
  - `startup_starting_toast_shown: bool` — 「启动中」toast 去重。
- 新增 helper（置于 `push_toast` 附近），直推并返回 id 与（可选）自动关闭 Task：
  ```rust
  fn spawn_toast(
      state: &mut Remotrix,
      kind: ToastKind,
      message: String,
      close_after: Option<Duration>,
      show_close: bool,
  ) -> (u64, Task<Message>) {
      let id = state.next_toast_id;
      state.next_toast_id += 1;
      let mut toast = Toast::new(kind, message).close_after(close_after);
      if show_close { toast = toast.show_close(); }
      toast.id = id;
      push_toast(state, toast);
      let task = match close_after {
          Some(d) => Task::perform(
              async move { tokio::time::sleep(d).await; },
              move |_| Message::DismissToast(id),
            ),
          None => Task::none(),
      };
      (id, task)
  }
  ```
  （沿用现有 `ShowToast` handler 里 `Task::perform(tokio::time::sleep ...)` 的写法；`show_close=true` 时由 toast 卡片自带的关闭按钮发 `Message::DismissToast(id)`。）
- 新增内联关闭小工具（幂等）：
  ```rust
  fn dismiss_toast(state: &mut Remotrix, id: u64) {
      state.toasts.retain(|t| t.id != id);
  }
  ```
- `Message::Engine` 各分支：
  - **`EngineReady`**（在现有清空 `aria2_fetch_error`/`synced_gids`/`sync_done` 之后）：
    - 防御性关闭残留下载 toast：`if let Some(id) = state.downloading_toast_id.take() { dismiss_toast(state, id); }`
    - `state.startup_starting_toast_shown = false;`（允许重启后再显示）
    - `state.aria2_status = Some(("ready".to_string(), state.fluent.get(Tr::Aria2Ready)));`（避免 settings 页残留 "starting"）
    - `let (_, task) = spawn_toast(state, ToastKind::Success, state.fluent.get(Tr::EngineStarted), Some(Duration::from_secs(3)), false); return task;`
  - **`Aria2Status { stage, message }`**（保留现有 `if stage == "ready" { state.aria2_fetch_error = None; }` 与 `state.aria2_status = Some((stage, message));`，其后追加）：
    - 下载完成则关闭持久化下载 toast（幂等）：
      `if stage == "ready" || stage == "starting" { if let Some(id) = state.downloading_toast_id.take() { dismiss_toast(state, id); } }`
    - `if stage == "downloading" && state.downloading_toast_id.is_none()`：
      `let (id, task) = spawn_toast(state, ToastKind::Normal, state.fluent.get(Tr::DownloadingAria2), None, false);` `state.downloading_toast_id = Some(id);` `return task;`
      （`None` = 不自动关闭；`"verifying"` 不在此分支，复用同一条持久化 toast。）
    - `if stage == "starting" && !state.startup_starting_toast_shown`：
      `state.startup_starting_toast_shown = true;`
      `let (_, task) = spawn_toast(state, ToastKind::Normal, state.fluent.get(Tr::EngineStarting), Some(Duration::from_secs(3)), false); return task;`
    - 其余（`"verifying"`、重复 `"downloading"`、`"update-*"` 等）落到末尾 `Task::none()`。
  - **`Aria2FetchFailed { error }`**：
    - `let msg = format!("{}: {error}", state.fluent.get(Tr::EngineStartFailed));`（先借用 `error` 格式化）
    - `state.aria2_fetch_error = Some(error);`（再 move）
    - 关闭持久化下载 toast：`if let Some(id) = state.downloading_toast_id.take() { dismiss_toast(state, id); }`
    - `let (_, task) = spawn_toast(state, ToastKind::Error, msg, None, true); return task;`
      （**错误 toast 持久化 + 关闭按钮**：`close_after=None`、`show_close=true`，由用户手动关闭，避免错过。）
  - **`EngineDegraded { reason }`**：维持原状（`state.aria2_fetch_error = Some(reason);`），**不弹 toast**（避免命令刷屏）。
- 注意：在 engine 分支内 `return task;` 前完成所有 state 赋值；未命中 toast 的分支回落到 `update` 末尾 `Task::none()`。

### 4. `src/ui/settings_page.rs`（小改，可选但建议）
- `aria2_status` 上色：把 `stage == "starting"` 加入 accent 分组（与 `"update-downloading"` / `"update-verifying"` 同），避免瞬时「starting」状态行显示为弱色。

## 状态机 / 边界
- **全新下载**：`downloading`(info, 持久) → `verifying`(复用同条) → `ready`(关闭下载 toast) → `starting`(info, 3s) → `EngineReady`(success, 3s)。`"downloading"` 连发两次只创建一条（`is_none()` 去重）。
- **缓存命中 / `ARIA2_BIN`**：无 `downloading`，直接 `starting`(info) → `EngineReady`(success)。
- **启动失败（网络/目录/连接）**：`downloading`(持久) → `Aria2FetchFailed`(关闭下载 toast + error，**持久 + 关闭按钮**)。若 `Sidecar::spawn` 在 `starting` 之后失败：`starting` toast 已显示且 3s 自闭，随后 error toast 出现（持久，需手动关闭）。
- **引擎重启/重试**：`EngineReady` 重置 `startup_starting_toast_shown` 并清 `downloading_toast_id`，重启后能重新展示整套 toast。
- **持久化 toast 卡死风险**：下载 toast 与错误 toast 均为 `close_after=None`。下载 toast 依赖引擎最终发 `ready`/`starting`/`Aria2FetchFailed` 之一关闭（必有其一）；错误 toast 带关闭按钮由用户关闭。两者都不会因超时消失。

## 验证
- `cargo build`
- `cargo clippy --workspace`（无警告）
- `cargo fmt --check`
- 手动：
  1. 删除 aria2 数据目录（强制下载）后启动 → 「下载中」toast 出现且**不自动消失**；下载完成后关闭并出现「启动中」(3s) → 「启动成功」(3s)。
  2. 下载过程中断网/构造失败 → 「下载中」toast 在失败时关闭并出现「错误」toast。
  3. 二次启动（缓存命中）→ 「启动中」(3s) → 「启动成功」(3s)，无「下载中」。
  4. 设置无效 `ARIA2_BIN` 启动 → 错误 toast。

## 影响范围
- `src/engine.rs`（boot 发一个 `"starting"` 状态）
- `src/app.rs`（`spawn_toast`/`dismiss_toast` helper、`downloading_toast_id` + `startup_starting_toast_shown` 字段、engine 分支 toast 触发与关闭、导入）
- `src/i18n.rs`、`i18n/locales/{zh-CN,en}/main.ftl`（3 个新键）
- `src/ui/settings_page.rs`（`"starting"` 阶段配色，可选）
