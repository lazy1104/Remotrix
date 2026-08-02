# 自动清理设置（Download 设置页 Auto Cleanup 分组扩展）

## 现状与目标

用户要求在「设置 → 下载」中添加自动清理设置，共三项：

1. **下载完成后删除种子文件** — **已存在**。`Settings.delete_torrent_after_complete`（默认 `false`）已完整实现：
   - 本地种子（AddTorrent）：`state.torrent_files` 记录路径，完成时在 `app.rs` Progress 事件里删除（app.rs:1097-1104）
   - URL 种子（follow-torrent）：`EngineCmd::FollowTorrent { delete_after }` 在引擎侧删除（engine.rs:767-799）
   - UI 开关已存在于 `settings_page.rs` AutoCleanup 分组（Tr::DeleteTorrentAfterComplete）
   - **无需改动**，仅保留。

2. **自动清理已完成任务（应用关闭时）** — 新增。
3. **本地文件缺失时自动删除对应任务** — 新增，按用户确认：**仅检查 Completed 且 total>0 的任务**；**启动同步完成后检查一次 + 每 30 秒定期复查**。

## 设计决策

- 关闭清理（功能 2）语义与现有 `ClearCompleted` 一致：**只删除任务记录 + `remove_download_result`，不删除已下载文件**。
- 缺失检查（功能 3）删除语义相同：记录 + aria2 结果清除；文件本身已缺失无需删除。多文件任务要求**所有文件路径都不存在**才判定缺失（部分缺失保留）。判定在引擎侧用 `collect_file_paths()` 的真实路径，避免用 `save_dir/name` 启发式误判多文件种子。
- 缺失检查在引擎中 `tokio::spawn` 后台执行（不阻塞 supervisor 命令循环），参照 `CheckAria2Update` 模式。
- 新设置默认均为 `false`，`#[serde(default)]` 保证旧 settings.json 兼容。
- 新设置立即在 UI 生效（live `state.settings`），与现有 `delete_torrent_after_complete` 行为一致，无需等待 Apply 才生效于行为逻辑；持久化仍需点击 Apply。

## 实现步骤

### 1. `src/config.rs` — 新增设置字段
在 `Settings` 结构体中添加（位于 `delete_torrent_after_complete` 附近）：
```rust
#[serde(default)]
pub cleanup_completed_on_close: bool,
#[serde(default)]
pub remove_task_if_files_missing: bool,
```
- `Settings::default()` 中初始化为 `false`。
- `Settings::apply_fields_equal()` 中追加两个字段的相等比较（保证脏检测/Apply/Reset 生效）。

### 2. `src/message.rs` — 新增 SettingKey 与 Message
- `SettingKey` 追加：`CleanupCompletedOnClose`、`RemoveTaskIfFilesMissing`。
- `Message` 追加：`CheckMissingFiles`。

### 3. `src/engine.rs` — 缺失文件检查
- `EngineCmd` 追加：`CheckMissingFiles`。
- `EngineEvent` 追加：
  ```rust
  FilesMissing { gids: Vec<String> },
  ```
- 新增异步辅助函数：
  ```rust
  async fn check_missing_files(client: &Client) -> Vec<String> {
      let mut missing = Vec::new();
      for s in fetch_all_tasks(client).await {
          if s.status != Aria2TaskStatus::Complete || s.total_length == 0 { continue; }
          let paths = collect_file_paths(&s);
          if paths.is_empty() { continue; }
          if paths.iter().all(|p| !Path::new(p).exists()) {
              missing.push(s.gid.clone());
          }
      }
      missing
  }
  ```
- `handle_client_cmd` 中新增分支（后台 spawn，不阻塞）：
  ```rust
  EngineCmd::CheckMissingFiles => {
      let client = client.clone();
      let tx = event_tx.clone();
      tokio::spawn(async move {
          let gids = check_missing_files(&client).await;
          if !gids.is_empty() {
              let _ = tx.send(EngineEvent::FilesMissing { gids });
          }
      });
  }
  ```

### 4. `src/app.rs` — 接线
- `revert_apply_settings()`：从 `applied_settings` 恢复两个新字段。
- `SettingChanged` 匹配新增两分支：
  ```rust
  SettingKey::CleanupCompletedOnClose => {
      state.settings.cleanup_completed_on_close = value == "true";
  }
  SettingKey::RemoveTaskIfFilesMissing => {
      state.settings.remove_task_if_files_missing = value == "true";
  }
  ```
- 新增 `Message::CheckMissingFiles` 分支：`cmd_tx.send(EngineCmd::CheckMissingFiles)`（失败仅 warn）。
- `Engine` 事件新增分支 `EngineEvent::FilesMissing { gids }`：
  - 对每个 gid 调用 `remove_task_local(state, gid)`（已有函数，负责状态/DB 清理）
  - `cmd_tx.send(EngineCmd::PurgeResults(gids.clone()))`，防止下次启动从 session/stopped 重新出现
  - 显示一条 toast：`spawn_toast(state, ToastKind::Normal, fluent.get(Tr::FilesMissingRemoved), Some(Duration::from_secs(3)), false)`（不传数量，Fluent 封装不支持参数）
- `SyncComplete` 处理器末尾（`state.sync_done = true` 之后）：若 `state.settings.remove_task_if_files_missing`，发送 `EngineCmd::CheckMissingFiles`。
- `begin_close()`（app.rs:295）：在发送 `EngineCmd::Shutdown` **之前**，若 `state.settings.cleanup_completed_on_close`：
  - 收集 `Completed | Removed` 的 gid 列表
  - `db.clear_completed(&gids)`（若 db 存在）
  - `cmd_tx.send(EngineCmd::PurgeResults(gids.clone()))`
  - 清 `dirty`、`tasks.retain(...)`、`task_order.retain(...)` —— 完全复用 `ClearCompleted`（app.rs:665-692）的本地清理逻辑
  - 顺序保证：PurgeResults 先于 Shutdown 被 supervisor 顺序处理，session 不会重存这些结果
- `subscription()`：追加定期触发器（app.rs:2083 batch 中加入）：
  ```rust
  let missing_check = if state.settings.remove_task_if_files_missing && state.sync_done {
      iced::time::every(Duration::from_secs(30)).map(|_| Message::CheckMissingFiles)
  } else {
      Subscription::none()
  };
  ```

### 5. `src/ui/settings_page.rs` — UI 开关
`download_view` 的 AutoCleanup 分组（settings_page.rs:438-443）在 `DeleteTorrentAfterComplete` 之后追加两个 `labeled_toggle`：
```rust
.push(labeled_toggle(
    fluent.get(Tr::CleanupCompletedOnClose),
    settings.cleanup_completed_on_close,
    SettingKey::CleanupCompletedOnClose,
))
.push(labeled_toggle(
    fluent.get(Tr::RemoveTaskIfFilesMissing),
    settings.remove_task_if_files_missing,
    SettingKey::RemoveTaskIfFilesMissing,
))
```

### 6. `src/i18n.rs` + FTL 文件
- `Tr` 枚举与 `key()` 各追加三个变体：
  - `CleanupCompletedOnClose` → `"cleanup-completed-on-close"`
  - `RemoveTaskIfFilesMissing` → `"remove-task-if-files-missing"`
  - `FilesMissingRemoved` → `"files-missing-removed"`
- `i18n/locales/en/main.ftl`：
  ```
  cleanup-completed-on-close = Clean up completed tasks on close
  remove-task-if-files-missing = Remove tasks whose files are missing
  files-missing-removed = Removed tasks with missing files
  ```
- `i18n/locales/zh-CN/main.ftl`：
  ```
  cleanup-completed-on-close = 关闭时清理已完成任务
  remove-task-if-files-missing = 本地文件缺失时删除任务
  files-missing-removed = 已删除文件缺失的任务
  ```

## 边界情况

- 活动/等待/暂停/出错任务：不检查（已确认范围仅 Completed）。
- 引擎不可用（EngineDegraded）：`CheckMissingFiles` 命令被 supervisor 丢弃并发送 EngineDegraded，无害；定期订阅同时用 `sync_done` 门控。
- 重复 FilesMissing 事件：`remove_task_local` 对不存在 gid 为幂等 no-op。
- 已完成任务完成时 aria2 会自行删除 `.aria2` 控制文件，无需额外清理。
- 关闭清理依赖 `begin_close`，用户若在关闭对话框选择取消则不会执行；SIGINT/SIGTERM → `ShutdownRequested` → `begin_close`，同样覆盖。

## 验证

```bash
cargo build
cargo clippy --workspace    # 无警告
cargo fmt --check
```
手工验证：
1. 设置页 Download → Auto Cleanup 显示 3 个开关；切换后点击 Apply，重启后设置保留。
2. 关闭清理：有已完成任务时关闭应用 → 重启后列表无这些记录（aria2 stopped 结果已 purge）。
3. 缺失检查：完成一个下载任务后删除磁盘文件 → 30 秒内任务自动消失并弹 toast；启动时同步完成后同样检查一次。
4. 多文件种子任务仅删除部分文件 → 任务保留。
