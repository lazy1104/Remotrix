# 修复：重启后非默认目录任务无法 resume（ghost 任务对账）

## 问题根因（已与日志/数据交叉验证）

- aria2 sidecar 用 `--input-file` + `--save-session` 同一个 `session.txt`，且 `--save-session-interval=60`。
- 任务在 session.txt 中才能在重启后被 aria2 重新加载、才能 `unpause`（resume）。
- 当引擎在 60s 保存窗口内被非优雅退出（`kill_on_drop` / 进程被杀 / 崩溃）时，最近添加/暂停的任务来不及写入 session.txt → 永久丢失。
- 现状：SQLite DB 仍保留该任务记录，UI 显示为「paused」并带 resume 按钮；但 aria2 已不认识该 GID。
  - `Resume` → `EngineCmd::Resume(gid)` → `client.unpause(gid)` → 静默失败（日志：`GID ... is not found`）。
- 实测数据：DB 有 4 个任务，session.txt 只有 2 个；Music 任务 `68cc4d3165fcd4f6`（02:18:59 暂停，距 sidecar 启动仅 5s）在窗口内丢失，Downloads 任务因更早暂停已落盘。
- 「非 Downloads 路径」是巧合（Music 任务恰好是最近暂停的那个），与目录本身无关。代码中 `Resume` 无任何目录判断。

## 修复方向（用户已确认）

DB 对账 + 强制原 GID 重新 `add_uri`，依靠全局 `--continue=true` 从 `<dir>/<name>.aria2` 控制文件断点续传。让 remotrix 成为任务恢复的事实来源，不再单依赖 session.txt。

## 实施步骤

### 1. `src/engine.rs` — 新增 `ReaddTask` 命令

在 `EngineCmd` 中新增：
```rust
ReaddTask { gid: String, url: String, save_dir: PathBuf, split: u16, paused: bool },
```
在 `handle_client_cmd` 中处理：
- 构造 `TaskOptions { gid: Some(gid.clone()), dir: Some(save_dir...), split: Some(split as i32), max_connection_per_server: Some((split as i32).max(1)), r#continue: Some(true), auto_file_renaming: Some(true), ..Default::default() }`
- `client.add_uri(vec![url.clone()], Some(options), None, None).await` → 强制返回原 GID。
- 失败：`tracing::warn!` 并发送 `EngineEvent::TaskDetailsFailed { gid }`（复用现有事件，UI 即停止 loading；此处仅作失败信号），不影响其他任务。
- 成功且 `paused == true`：立即 `client.pause(&gid).await`（add_uri 无法直接以暂停态入队，靠事后 pause 保留用户意图）。
- 成功后发送 `EngineEvent::Added { gid, name: basename(&url), url, dir }`，并 `emit_progress`（`tell_status` 后）。
- 注意：对账仅在 aria2 不存在该 GID 时触发，故 add_uri 的强制 GID 不会冲突。

### 2. `src/engine.rs` — 新增 `SyncComplete` 事件并在同步后发送

- `EngineEvent` 新增 `SyncComplete`。
- 在 `on_sidecar_ready` 的 sync 任务中，`sync_existing_tasks(...).await` 之后发送 `let _ = sync_event_tx.send(EngineEvent::SyncComplete);`。

### 3. `src/engine.rs` — 优雅退出/重启前显式 `save_session`（轻量加固）

- `EngineCmd::Shutdown` 处理：在 `s.client.shutdown().await` 之前先 `let _ = s.client.save_session().await;`。
- `EngineCmd::RestartEngine` 处理：在 `s.client.shutdown().await` 之前先 `save_session`。
- `run_supervisor` 中 `restart_rx`（sidecar 自身退出）分支无法调 save_session（进程已退出），保持现状——对账兜底。
- 将 `--save-session-interval` 从 `60` 改为 `5`（`Sidecar::spawn` 的 arg），缩小非优雅退出丢失窗口。

### 4. `src/app.rs` — 启动对账，重建 ghost 任务

- `Remotrix` 新增字段：`synced_gids: HashSet<String>`、`sync_done: bool`。
- `init()` 中初始化两者为空/false。
- `EngineEvent::EngineReady`：`state.synced_gids.clear(); state.sync_done = false;`。
- `EngineEvent::Added { gid, .. }`：在现有逻辑里 `state.synced_gids.insert(gid.clone());`（含对账后 re-add 触发的 Added，幂等）。
- `EngineEvent::Progress { gid, .. }`：`state.synced_gids.insert(gid.clone());`（双保险）。
- `EngineEvent::SyncComplete`：
  - `state.sync_done = true;`
  - 遍历 `state.tasks`：对 `status ∈ {Waiting, Active, Paused}` 且 `!url.is_empty()` 且 `!synced_gids.contains(&gid)` 的任务，发送：
    ```rust
    EngineCmd::ReaddTask { gid, url, save_dir, split: state.settings.split, paused: status == Paused }
    ```
  - torrent 任务（`url.is_empty()`）跳过——无法仅凭 URL 重建（见「已知限制」）。
  - `Completed`/`Removed`/`Error` 跳过。
- 对账后 re-add 产生的 `Added` 会命中 app.rs 中 `if let Some(existing) = state.tasks.get_mut(&gid)` 分支（仅标记 dirty，不重复插入），随后 `Progress` 更新进度——UI 无重复行。

### 5. 一致性

- `EngineCmd`/`EngineEvent` 定义在 `engine.rs`，`message.rs` 仅 `use`/透传 `EngineEvent`，无需改动 message.rs。
- `_ => {}` 通配分支在 `handle_client_cmd` 末尾已覆盖非客户端命令；新增的 `ReaddTask` 会被 `run_supervisor` 的 `_ => { handle_client_cmd(...) }` 分支正确派发。

## 数据流

```
app 启动 → DB load_all() 填充 tasks (含 ghost 68cc)
engine 启动 → aria2 --input-file session.txt (无 68cc)
            → sync_existing_tasks → Added(0b31,41c0) + SyncComplete
app: SyncComplete → 68cc 未在 synced_gids → ReaddTask{gid:68cc,url,dir:Music,paused:true}
engine: add_uri(强制 gid=68cc, dir=Music) → aria2 --continue=true 命中 Music/*.aria2 → 断点续传
      → pause(68cc) → Added(68cc) + Progress
app: 68cc 现可 resume（unpause 命中真实 GID）
```

## 已知限制 / 边界

- torrent ghost 任务（DB 中 `url` 为空）无法自动重建（缺少 torrent 字节）；保持现状，后续若需支持须在 DB 持久化 torrent 路径或元数据。本次跳过。
- `Error` 态任务不自动 re-add（避免循环重试坏链接）；用户可手动删除/重试。
- re-add 时 `add_uri` 以 URL basename 推导文件名，须与原任务一致才能命中 `.aria2` 控制文件；当前添加路径同样用 basename，一致。`auto-file-renaming=true` 下若存在同名完整文件会重命名——可接受。
- 非优雅退出（进程被 SIGKILL）仍可能产生 ghost，但本方案在下次启动对账兜底恢复，不再依赖 60s 窗口。

## 验证

1. 现网复现：修复后重启 app，观察 ghost 任务 `68cc4d3165fcd4f6` 被 re-add，`tell_status` 不再报 `not found`，进度从 ~25MB 继续。
2. 构造测试：添加一个下载到非默认目录，暂停，`kill -9` 强杀 app 进程，重开 → 任务自动 re-add 并续传。
3. torrent 任务重启后不被误重建（跳过），不报错。
4. `cargo clippy --workspace` 无警告、`cargo fmt --check` 通过、`cargo build` 成功。

## 改动文件

- `src/engine.rs`：新增 `EngineCmd::ReaddTask`、`EngineEvent::SyncComplete`；处理逻辑；sync 后发 `SyncComplete`；shutdown/restart 前 `save_session`；`--save-session-interval=5`。
- `src/app.rs`：新增 `synced_gids`/`sync_done`；`EngineReady`/`Added`/`Progress`/`SyncComplete` 处理；对账派发 `ReaddTask`。
