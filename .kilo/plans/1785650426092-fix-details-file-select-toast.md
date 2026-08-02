# 修复详情页文件勾选的误报 toast + 保持实时生效（带去抖）

## Goal
详情页 Files 标签里勾选/取消勾选种子文件时，不再弹“修改文件选择失败”误报；勾选继续**实时生效**（用户已确认），并加入短时间去抖，避免连续勾选导致进行中的下载被反复重启。

## Root cause（已验证）
- `aria2-ws 0.5.1` 的 `change_option`（`method.rs:191`）内部走 `call_and_wait::<()>`；`wait_for_id`（`client.rs:230-231`）执行 `serde_json::from_value::<()>(res.result)`。
- aria2-next 的 `ChangeOptionRpcMethod::process` 成功后返回 `"OK"`（`src/RpcMethodImpl.cc` 的 `createOKResponse()`）。
- **`()` 只能从 `null` 反序列化**，遇到 `"OK"` 必然报 JSON 错误（已用独立测试实测：`"OK"` → `ok=false`）。
- 因此 `change_option` 在 aria2 已成功应用选择后仍返回 `Err`；`engine.rs` 的 `SelectFiles` 处理器把任何 `Err` 当失败 → `EngineEvent::SelectFilesFailed` → toast。
- 请求本身已被 aria2 处理，所以勾选确实实时生效——这正是“修改是实时的”且伴随误报 toast 的原因。
- 其他 `Result<()>` 方法（pause/unpause/remove/save_session/shutdown）同样永远返回 Err，但引擎用 `let _ =` 忽略，故不可见；不在本次范围（`remove` 因此恒走 `force_remove`，属既有无害行为）。

## 改动 1 — `src/engine.rs`：修复误报（`EngineCmd::SelectFiles`，约 878-900 行）
把 `client.change_option(&gid, options).await` 替换为对 `serde_json::Value` 的裸 `call_and_wait`（`Value` 可反序列化任意 JSON 结果，真实 aria2 错误仍走 `res.error` → Err）：

```rust
EngineCmd::SelectFiles { gid, files } => {
    let Some(csv) = select_file_csv(&files) else {
        return Ok(());
    };
    tracing::info!(?gid, ?files, "change file selection");
    let options = TaskOptions {
        extra_options: {
            let mut map = serde_json::Map::new();
            map.insert("select-file".to_string(), serde_json::Value::String(csv));
            map
        },
        ..Default::default()
    };
    let params = match serde_json::to_value(options) {
        Ok(v) => vec![serde_json::Value::String(gid.clone()), v],
        Err(e) => {
            tracing::warn!(?gid, error = ?e, "serialize select-file options failed");
            let _ = event_tx.send(EngineEvent::SelectFilesFailed { gid });
            return Ok(());
        }
    };
    match client.call_and_wait::<serde_json::Value>("changeOption", params).await {
        Ok(_) => {
            let _ = client.save_session().await;
        }
        Err(e) => {
            tracing::warn!(?gid, error = ?e, "changeOption select-file failed");
            let _ = event_tx.send(EngineEvent::SelectFilesFailed { gid });
        }
    }
}
```

要点：
- `call_and_wait` 是 `InnerClient` 上的 `pub` 方法，`Client` 经 `Deref` 可调；会自动前置 rpc-secret token（与 `change_option` 一致）。
- 不匹配错误类型（`Error` 各变体为 `pub(crate)`，外部不可见），只用 `Err(e)` 记录日志并照旧发 `SelectFilesFailed`。

## 改动 2 — 勾选去抖（`src/message.rs` + `src/app.rs`）
目标：350ms 窗口内的多次勾选合并为一次 `EngineCmd::SelectFiles`，进行中的下载至多重启一次。

状态（`Remotrix` 增加两个字段，见 `app.rs:33` 结构体）：
```rust
details_pending_select: Option<(String, Vec<u64>)>, // 最近一次 (gid, 已选 indices)
details_select_gen: u64,                           // 代际计数，用于丢弃过期 flush
```

新消息（`message.rs`，`DetailsFilesScroll` 附近）：
```rust
DetailsFilesFlush(u64), // 参数为触发时的 gen
```

处理器改动（`app.rs`）：
- `DetailsTreeToggle` / `DetailsFilesSelectAll` / `DetailsFilesSelectNone`（约 1531-1604 行）：保留现有乐观翻转 `details.files[].selected`；随后：
  ```rust
  let gid = state.details.gid.clone();
  let selected = selected_details_indices(state);
  if let Some(gid) = gid {
      if !selected.is_empty() {
          state.details_pending_select = Some((gid, selected));
          state.details_select_gen += 1;
          let gen = state.details_select_gen;
          return Task::perform(
              async move {
                  tokio::time::sleep(Duration::from_millis(350)).await;
                  gen
              },
              Message::DetailsFilesFlush,
          );
      }
  }
  ```
- 新增 `Message::DetailsFilesFlush(gen)`：
  - `gen != state.details_select_gen` → 过期，`Task::none()`。
  - 否则 `take()` 出 `details_pending_select`，发送 `EngineCmd::SelectFiles { gid, files }` + `EngineCmd::FetchTaskDetails(gid)`。
- `Message::CloseTaskDetails`（约 1501-1503 行）：先 `state.details_select_gen += 1`（取消在途 debounce），若 `details_pending_select` 非空则立即发送 `SelectFiles`（不丢失用户已点选、未及刷新的变更），再清空该字段，最后 `state.details.close()`。
- `Message::OpenTaskDetails`（约 1490 行）：重置 `details_select_gen = 0`、`details_pending_select = None`。

注意：`DetailsFilesSelectNone` 现有实现保证至少选中 1 个文件，`selected` 永不为空，与去抖逻辑一致。

## 验证
- `cargo build`（离线可过，无新依赖）、`cargo clippy --workspace`、`cargo fmt --check`。
- 手工 QA：
  1. 活动任务 Files 页勾选/取消文件 → 无错误 toast，刷新后勾选状态保持。
  2. paused/waiting 任务同样操作 → 无 toast，直接生效。
  3. 快速连续勾选 3 个文件 → 引擎日志 `change file selection` 只出现一次（单次重启）。
  4. 勾选后立即关闭对话框 → 变更仍被应用且无 toast。
  5. completed/removed 任务 → 复选框已禁用（既有 `enabled` 逻辑不变），不发 changeOption。

## 风险
- debounce 350ms 为调参常量；活动任务的重启从“每次勾选”变为“每轮连点最多一次”。
- `FetchTaskDetails` 移到 flush 时机，勾选后 <350ms 内 UI 依赖乐观状态（打开时及每次 flush 后均已拉取，影响很小）。
- `save_session` 仍用 `Result<()>`（恒 Err 但被忽略），本次不改。
