# 关闭时立即隐藏窗口，后台完成引擎收尾

## Goal
有下载任务时关闭较慢（引擎 pause_all → 轮询等待暂停(≤2s) → save_session → shutdown），期间 UI 一直可见可交互。用户选定方案：确认关闭后**立即隐藏窗口**，UI 瞬间消失；引擎在后台完成收尾，完成后进程退出。5s 超时兜底保留。

## Changes（仅 `src/app.rs` 一处）

### `begin_close`（约 line 387-414）把返回的 `shutdown_timeout_task()` 改为先隐藏窗口、再排超时

`src/app.rs:413` 现为：

```rust
    shutdown_timeout_task()
```

改为：

```rust
    let hide = state
        .window_id
        .map(|id| iced::window::set_mode::<Message>(id, iced::window::Mode::Hidden))
        .unwrap_or_else(Task::none);
    hide.chain(shutdown_timeout_task())
```

要点：
- `state.window_id: Option<iced::window::Id>`（`Id` 已在 app.rs:11 导入；`Task` 已在 app.rs:11 导入）。
- `iced::window::set_mode::<Message>` / `iced::window::Mode::Hidden` 是 iced 0.14 标准 API（iced_runtime window.rs:395；`Mode::Hidden` 隐藏窗口但保持应用运行）。
- `Task::chain` 已存在于 iced_runtime 0.14（task.rs:159）：先执行 hide，再排 5s 超时任务。
- `window_id` 为 None（启动早期收到 SIGTERM）时跳过隐藏，行为与现状一致。

## 改动后行为
1. 用户点关闭 → 确认（`CloseDialogChoice::Close`）→ `begin_close`：窗口**立即隐藏**（UI 消失）。
2. 引擎在后台继续：`pause_all` → 等暂停 → `save_session` → `shutdown` → 发 `EngineStopped`。
3. 收到 `EngineStopped` 或 5s 超时 → `finalize_close`（不变）→ flush DB / 保存配置 / `iced::window::close(id)` → 进程退出。
4. 隐藏 ≠ 关闭：隐藏状态下 iced 事件循环与引擎 subscription 照常运行，`window::close` 仍会退出应用。
5. 无活动任务时引擎立即返回 `EngineStopped`，隐藏+退出几乎瞬时。

## Risks
- Wayland 下 winit 对窗口隐藏支持有限，`set_mode(Hidden)` 可能无效，窗口会保持可见直到真正关闭。本方案不破坏现状（`EngineStopped`/超时仍会关窗）；如需兜底可后续加"正在关闭"遮罩，本次不做。
- `finalize_close` 中 `geometry_dirty` 走 `is_maximized` → `WindowMaximized` → `window::close` 的异步链路在隐藏态仍有效（窗口属性仍在），无需改动。

## Out of Scope
- 不改引擎端暂停等待循环（engine.rs:1373-1400）与 5s 超时时长。
- 不加"正在关闭…"提示文案/遮罩（用户已选立即隐藏方案）。
- 不改 `engine.rs`、`main.rs`、`ui/close_dialog.rs`、i18n。

## Validation
```bash
cargo build
cargo clippy --workspace   # 无警告
cargo fmt --check
```
手动验证：
1. 启动 → 添加下载任务 → 点关闭 → 确认：窗口立即消失，进程约 2-5s 内退出（日志可见 pause_all → save_session → shutdown）。
2. 无任务时关闭：窗口立即消失并快速退出。
3. 重新启动：任务以暂停状态恢复（会话保存正常，行为与改动前一致）。
4. 引擎未运行（`ARIA2_BIN` 指向无效路径）时关闭：5s 超时兜底退出。
