# Toast 消息分组：跨功能消息互不顶掉

## 目标
新增 `ToastGroup` 概念：`push_toast` 的「同位置替换瞬时 toast」逻辑只作用于**同分组**。不同功能的消息（如 tracker 同步 vs 引擎启动成功）可在同一位置共存，不再互相顶掉。同时清理已无用的 `.position(ToastPosition::Top)` 显式调用。

## 现状（已核实）
- `push_toast`（app.rs:2920）当前会 retain 移除**同位置所有** `close_after.is_some()`（瞬时）toast → 新 tracker toast（Top）会把引擎「启动成功」toast（Top）顶掉。
- 持久 toast（`close_after.is_none()`：`DownloadingAria2`、`Aria2FetchFailed`）受保护，不被替换。
- 共 18 个 toast 发射点：`spawn_toast` 调用 11 处（622/633/652/1268/1398/1621/1679/1704/1716/1780/2290），直接 `Toast::new` 7 处（1227/1235/1918/1947/1976/2013/2829）。

## 分组设计（5 组，全部调用点显式标记）

| 分组 | 调用点 | 行号 |
|---|---|---|
| `Engine` | EngineStarted / Aria2FetchFailed(持久) / DownloadingAria2(持久) / EngineStarting | 1268, 1679, 1704, 1716 |
| `Tracker` | TrackerCustomAdd 无效 URL / SyncTrackers 无源 / BtTrackerSyncFailed / 同步结果 | 1918, 1947, 1976, 2013 |
| `Task` | NoDownloadableContent / InvalidTorrent×2 / DropDetected / FilesMissingRemoved / SelectFilesFailed / ClipboardDetected / FileMissing | 622, 633, 652, 1398, 1621, 1780, 2290, 2829 |
| `Logs` | LogsCleared / LogsClearFailed | 1227, 1235 |
| `General` | 默认值（无显式使用点） | — |

## 实现

### 1. `src/ui/components/toast.rs`
- 新增枚举：
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
  pub enum ToastGroup {
      #[default]
      General,
      Engine,
      Tracker,
      Task,
      Logs,
  }
  ```
- `Toast` 结构体新增 `pub group: ToastGroup` 字段。
- `Toast::new`：初始化 `group: ToastGroup::General`。
- 新增 builder：`pub fn group(mut self, group: ToastGroup) -> Self { self.group = group; self }`。

### 2. `src/message.rs`
- 无改动（group 随 `Toast` 传递）。

### 3. `src/app.rs`
- 导入：`use crate::ui::components::toast::{Toast, ToastGroup, ToastKind};`（`ToastPosition` 移除——清理冗余 position 后不再引用，否则 clippy 报 unused import）。
- `spawn_toast`（2945）签名增加 `group: ToastGroup` 参数（放在 `kind` 前）：
  ```rust
  fn spawn_toast(
      state: &mut Remotrix,
      group: ToastGroup,
      kind: ToastKind,
      message: String,
      close_after: Option<Duration>,
      show_close: bool,
  ) -> (u64, Task<Message>)
  ```
  内部 `Toast::new(kind, message).group(group)...`。
- `push_toast`（2920）替换逻辑按分组作用域：
  ```rust
  fn push_toast(state: &mut Remotrix, toast: Toast) {
      const CAP: usize = 6;
      let pos = toast.position;
      let group = toast.group;
      let removed_hovered = matches!(
          state.hovered_toast_id,
          Some(h)
              if state.toasts.iter().any(|t| t.id == h && t.position == pos && t.group == group && t.close_after.is_some())
      );
      state.toasts.retain(|t| !(t.position == pos && t.group == group && t.close_after.is_some()));
      if removed_hovered {
          state.hovered_toast_id = None;
      }
      let at_pos = state.toasts.iter().filter(|t| t.position == pos).count();
      if at_pos >= CAP {
          if let Some(idx) = state.toasts.iter().position(|t| t.position == pos) {
              state.toasts.remove(idx);
          }
      }
      let mut toast = toast;
      toast.remaining = toast.close_after;
      state.toasts.push(toast);
  }
  ```
- 18 个发射点按上表显式传 `.group(...)`（`Toast::new` 直接调用处加 `.group(ToastGroup::X)`，`spawn_toast` 调用处传参）。
- **删除冗余 `.position(ToastPosition::Top)`（6 处，均已被 Top 默认值覆盖）**：
  - 直接 `Toast::new` 调用处：1922、1951、1977、2021、2830 —— 与 `.group(...)` 一起清理。
  - `spawn_toast` 内部：2955。
  - 同步从 use 语句移除 `ToastPosition`（见上文导入改动）。

## 边界 / 语义
- **同分组替换**：新瞬时 toast 替换同位置同分组的瞬时 toast；引擎「starting→started」流程不变（同组 Engine）。
- **跨组共存**：不同分组 toast 在同一位置纵向堆叠（顺序由 view 按 `ToastPosition::ALL` + 原顺序渲染）。
- **持久 toast 保护**：不变，仅 `close_after.is_some()` 被替换逻辑移除。
- **CAP=6 安全上限**：保持按位置（跨组累计），超过时移除该位置最早一条；实际最多约 5 条（4 组瞬时 + 1 持久）不触发。
- **悬停清理**：`removed_hovered` 条件同步加上 `t.group == group`，仅当被替换的是同组 toast 时清空悬停 id。

## 验证
- `cargo build`、`cargo clippy --workspace`（无警告）、`cargo fmt --check`。
- 手动：
  1. 触发 tracker 同步的同时引擎启动：Top 处「引擎启动成功」与「tracker 同步完成」两条 toast 同时可见；后续 tracker toast 只顶掉旧 tracker toast。
  2. 引擎「启动中」→「启动成功」仍逐条替换（同组 Engine）。
  3. 持久 toast（下载中/启动失败）仍不被任何 toast 顶掉。
