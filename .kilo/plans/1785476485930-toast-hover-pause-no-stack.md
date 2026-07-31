# Toast 悬停暂停倒计时 + 默认不堆叠

## 目标
1. **悬停暂停**：鼠标移入 toast 卡片时暂停其自动关闭倒计时；移出后从暂停处继续倒计时（「继续倒计时」，非重启）。
2. **默认不堆叠**：同一位置默认只显示一条瞬时 toast；新 toast 出现时**替换**同位置的已有瞬时 toast；**持久 toast**（`close_after = None`，即「下载中」「错误」）受保护，不被替换。

## 根因 / 现状（已核实）
- 自动关闭当前用一次性 `Task::perform(tokio::time::sleep(d))`（`app.rs:1570 auto_dismiss`），sleep 无法中途暂停 → 必须改为**订阅式倒计时**：每个 toast 持有可变 `remaining`，按固定节拍递减，悬停时跳过递减。
- `iced::widget::mouse_area` 提供 `.on_enter(Message)` / `.on_exit(Message)`（iced_widget 0.14.2 `mouse_area.rs:106/120`，经 `iced::widget::*` 重导出可用）。
- `iced::time::every(Duration) -> Subscription<Instant>` 已在 `app.rs:1463/1467` 使用，模式现成。
- 堆叠由 `push_toast`（`app.rs:1527`）按位置 cap=6 纵向堆叠；改为替换语义。
- 持久 toast 标志：`close_after.is_none()`（`Toast::close_after`，`toast.rs:69`）。

## 设计决策
- 倒计时机制：`Toast` 新增 `remaining: Option<Duration>`（可变），在 `push_toast` 入栈时由 `close_after` 初始化。`None` = 持久，永不递减。
- 节拍：`iced::time::every(200ms)` → `Message::ToastTick`；每拍对 `remaining.is_some()` 且 `id != hovered_toast_id` 的 toast 递减 200ms；`remaining <= 200ms` 时归零并移除该 toast。订阅仅当存在 `remaining.is_some()` 的 toast 时激活（`Subscription::none()` 否则），避免空转。
- 悬停：`card()` 外包 `mouse_area`，`on_enter → ToastHovered(id)`、`on_exit → ToastUnhovered(id)`；`Remotrix.hovered_toast_id: Option<u64>` 记录当前悬停 toast。
- 不堆叠：`push_toast` 入栈前 `retain` 移除同位置所有 `close_after.is_some()`（瞬时）toast；持久 toast 保留。若被移除的包含当前 `hovered_toast_id`，清空之（避免悬停 id 指向已消失 toast）。保留原 cap=6 作为持久 toast 安全上限。
- 删除 `auto_dismiss`（不再使用，否则 clippy 报死代码）。
- 硬编码默认，不加 config 字段、不加设置页 UI、不动 i18n / engine.rs / settings_page.rs。

## 实现

### 1. `src/ui/components/toast.rs`
- 导入：`use iced::widget::{button, column, container, mouse_area, row, stack, text};`
- `Toast` 结构体新增字段：`pub remaining: Option<Duration>`。
- `Toast::new`：初始化 `remaining: None`（由 `push_toast` 在入栈时覆写为 `close_after`）。
- `card()` 末尾把 `container(content)... .into()` 改为：
  ```rust
  mouse_area(
      container(content)
          .width(Length::Fixed(CARD_WIDTH))
          .padding(Padding { top: 10.0, right: 12.0, bottom: 10.0, left: 12.0 })
          .style(theme::style::toast),
  )
  .on_enter(Message::ToastHovered(toast.id))
  .on_exit(Message::ToastUnhovered(toast.id))
  .into()
  ```

### 2. `src/message.rs`
- 在 `DismissToast(u64)` 后新增：
  ```rust
  ToastHovered(u64),
  ToastUnhovered(u64),
  ToastTick,
  ```

### 3. `src/app.rs`
- `Remotrix` 新增字段 `hovered_toast_id: Option<u64>`；`init()` 初始化 `hovered_toast_id: None`。
- `update` 新增分支：
  - `Message::ToastHovered(id) => { state.hovered_toast_id = Some(id); }`
  - `Message::ToastUnhovered(id) => { if state.hovered_toast_id == Some(id) { state.hovered_toast_id = None; } }`
  - `Message::ToastTick =>` 固定 `TICK = Duration::from_millis(200)`；遍历 `state.toasts.iter_mut()`，对 `remaining.is_some()` 且 `Some(t.id) != state.hovered_toast_id` 者：`*rem <= TICK` 则归零并收集 id 到 `expired`，否则 `*rem -= TICK`；遍历结束后对每个 expired id 调 `dismiss_toast`。
- `Message::ShowToast` 分支：去掉 `close_after` 读取与 `auto_dismiss` 调用，仅 `push_toast(state, toast);`（倒计时由订阅接管）。
- `spawn_toast`：去掉 `auto_dismiss` 任务构造，`task` 改为 `Task::none()`（保持返回 `(u64, Task<Message>)` 签名，调用处 `return task;` 不变）。
- 删除 `fn auto_dismiss`。
- `push_toast` 改为：
  ```rust
  fn push_toast(state: &mut Remotrix, toast: Toast) {
      const CAP: usize = 6;
      let pos = toast.position;
      let removed_hovered = matches!(state.hovered_toast_id,
          Some(h) if state.toasts.iter().any(|t| t.id == h && t.position == pos && t.close_after.is_some()));
      state.toasts.retain(|t| !(t.position == pos && t.close_after.is_some()));
      if removed_hovered { state.hovered_toast_id = None; }
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
- `dismiss_toast` 末尾追加：`if state.hovered_toast_id == Some(id) { state.hovered_toast_id = None; }`（手动关闭/替换时清理悬停）。
- `subscription()` 在 `Subscription::batch` 的 vec 中新增：
  ```rust
  let toast_tick = if state.toasts.iter().any(|t| t.remaining.is_some()) {
      iced::time::every(Duration::from_millis(200)).map(|_| Message::ToastTick)
  } else {
      Subscription::none()
  };
  ```
  并将 `toast_tick` 加入 batch。

## 状态机 / 边界
- **瞬时 toast（starting/started，3s）**：入栈 `remaining=Some(3s)`；未悬停时每 200ms 递减，~3s 后移除；悬停时跳过递减，移出后继续。
- **持久 toast（下载中/错误，`close_after=None`）**：`remaining=None`，永不递减，不被新 toast 替换；由引擎事件（`ready`/`starting`/`Aria2FetchFailed`/`EngineReady`）或手动关闭按钮显式移除。
- **不堆叠替换**：新瞬时 toast 替换同位置已有瞬时 toast；持久 toast 保留（可能瞬时+持久短暂共存，符合「保护持久」决策）。
- **替换时悬停**：若被替换的是当前悬停 toast，清空 `hovered_toast_id`；新 toast 重新计数（接受「替换瞬间鼠标静止未触发 on_enter」的极少数情况）。
- **订阅激活**：仅当存在 `remaining.is_some()` 的 toast 时 `toast_tick` 订阅生效，全持久时停转。

## 验证
- `cargo build`、`cargo clippy --workspace`（无警告，确认 `auto_dismiss` 已删无死代码）、`cargo fmt --check`。
- 手动：
  1. 触发瞬时 toast（引擎启动「启动中/启动成功」）：悬停 → 不自动消失；移出 → 约剩余时间后消失。
  2. 删除 aria2 数据目录强制下载：仅一条持久「下载中」toast 常驻；完成后被引擎事件关闭，继以「启动中」→「启动成功」逐条替换，同屏至多一条瞬时 toast。
  3. 构造启动失败：持久「错误」toast（带关闭按钮）出现且不自动消失；重试成功后由 `EngineReady` 关闭错误 toast 并显示成功 toast。
  4. 悬停持久 toast（无倒计时）：无副作用，移出无变化。

## 影响范围
- `src/ui/components/toast.rs`（`remaining` 字段、`mouse_area` 包裹）
- `src/message.rs`（3 个新 Message 变体）
- `src/app.rs`（`hovered_toast_id` 字段、3 个新 update 分支、`ShowToast`/`spawn_toast` 去除 `auto_dismiss`、`push_toast` 替换语义、`dismiss_toast` 清悬停、`subscription` 加 `toast_tick`、删除 `auto_dismiss`）
- 不涉及：config / i18n / settings_page / engine.rs。
