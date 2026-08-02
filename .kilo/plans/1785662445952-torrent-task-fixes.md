# Torrent/Magnet 任务三项修复：打开种子、删除不生效、[METADATA] 命名

## Goal

修复三个种子/磁链相关的问题：

1. **打开 .torrent 文件**：任务卡"打开"按钮/双击任务名，当本地下载产物是 `.torrent` 文件（`.torrent` URL 任务、或 `bt-metadata-only` 磁链任务产出的 `[METADATA]<hash>.torrent`）时，应打开"新建下载"对话框并预载该本地 `.torrent`（Torrent 标签页 + 文件选择），而不是把当前任务链接塞进 URL 输入框。
2. **种子任务删除后自动恢复**：删除正在下载/做种的种子任务后，任务有时在重启后复活。根因是 aria2 会话保存竞态 + 磁链父/子任务残留。
3. **重启后任务名变成 `[METADATA]` 开头**：磁链任务（默认 `bt_auto_download=false` 时是 metadata-only 任务）的名字来自 aria2 元数据下载的文件名 `[METADATA]<infohash>`；重启后会话以 magnet URI 恢复任务、重新抓取元数据，名字再次回落为 `[METADATA]`。应解析本地 `.torrent` 显示真实种子名，并持久化已知名字防止回落。

## 已确认的设计决策

- 问题 1（用户已确认）：**打开"新建下载"对话框并预载本地 .torrent**（复用 `add_dialog.set_torrent_path` + `active_tab = AddTab::Torrent`，即现有"拖入 .torrent"的流程），而非直接下发 `AddTorrent`。
- 问题 3（用户已确认）：**解析本地 .torrent 显示真实种子名**（复用 `torrent_meta::parse_torrent`），并持久化已解析/已解析出的名字，重启不回退。

## 背景（根因，已验证）

- `Message::OpenTaskFile`（src/app.rs:1721）当前把 `is_torrent_url(&t.url)` 或 `info_hash` 非空一律当作"预填链接"处理。
- aria2 会话（`--save-session`/`--input-file`，src/engine.rs:288-293）：
  - 磁链任务以 **magnet URI** 行保存，重启后重新抓元数据 → 名字 `[METADATA]<hash>`（问题 3 根因）。
  - `addTorrent` 任务依赖 `--rpc-save-upload-metadata` 保存 `<sha1>.torrent`，恢复时用本地文件、名字正常。
- `EngineCmd::Remove`（src/engine.rs:697-711）：`client.remove(gid)`（优雅移除，对种子会先联系 tracker 注销、耗时）→ `remove_download_result()` → 立即 `save_session()`。移除未完成时 `save_session()` 会把任务写回会话文件，重启后复活（问题 2 根因之一）。
- `bt_auto_download=true` 时，磁链元数据下载 M（父）会在会话中被保存，重启后 M 恢复并重新生成其 followed 内容任务 T；用户删除 T 后 T 随 M 复活（问题 2 根因之二）。
- `t.name` 在 Added/Progress 事件里被无条件覆盖（src/app.rs:1109、1213），且 DB 的 `name` 列只在 `upsert_meta`（Added 时）写入，名字后续变化不持久化 → 重启后 `[METADATA]` 名字覆盖好名字（问题 3 根因之二）。
- `aria2-ws` 的 `BittorrentStatus` 不含 `info.name`，无法从状态里直接拿真实种子名；但本地 `.torrent` 文件可用现有 `torrent_meta::parse_torrent` 解析（内部已读取 `info.name`，src/torrent_meta.rs:21，只是未暴露）。

## Files & Changes

### 任务 1：`src/torrent_meta.rs` — 暴露种子名
- `TorrentMeta` 增加字段 `pub name: String`（src/torrent_meta.rs:9-11），在 `parse_torrent` 里用已解析的 `name`（第 21 行）填充；构造处（第 67 行）同步。
- 现有测试只断言 `meta.files`，无需改断言；编译不破坏。

### 任务 2：`src/db.rs` — 名字持久化
- 新增：
  ```rust
  pub fn update_name(&self, gid: &str, name: &str) {
      let conn = self.conn.lock().expect("db lock");
      let _ = conn.execute(
          "UPDATE tasks SET name=?1 WHERE gid=?2",
          rusqlite::params![name, gid],
      );
  }
  ```

### 任务 3：`src/app.rs` — 问题 3：`[METADATA]` 名字保护 + 解析
- 新增辅助函数（app.rs 内，靠近 `remove_task_local` 附近）：
  ```rust
  fn resolve_metadata_name(path: &std::path::Path) -> Option<String> {
      let bytes = std::fs::read(path).ok()?;
      crate::torrent_meta::parse_torrent(&bytes).map(|m| m.name)
  }

  fn apply_task_name(state: &mut Remotrix, gid: &str, t: &mut DownloadTask, incoming: String) {
      if incoming.starts_with("[METADATA]") {
          if !t.name.is_empty() && !t.name.starts_with("[METADATA]") {
              return; // 保留已解析的好名字
          }
          if let Some(real) = resolve_metadata_name(&t.save_dir.join(&incoming)) {
              t.name = real;
              if let Some(ref db) = state.db {
                  db.update_name(gid, &t.name);
              }
              return;
          }
      }
      if t.name != incoming {
          t.name = incoming;
          if let Some(ref db) = state.db {
              db.update_name(gid, &t.name);
          }
      }
  }
  ```
  - 注意：`resolve_metadata_name` 用 **incoming** 的 `[METADATA]` 名字拼 `save_dir`（两处名字在此场景相同）；只在该事件发生时读一次文件，解析成功后 `t.name` 变为真名，后续事件被首个分支挡住，不再重复解析。
  - 空名字任务首次出现 `[METADATA]` 事件时（`t.name.is_empty()`）允许接受，之后尝试解析。
- 替换两处无条件赋值：
  - Added 分支（src/app.rs:1108-1110）：`existing.name = name;` → `apply_task_name(state, &gid, existing, name);`（`name` 改为 move，注意调用顺序）。
  - Progress 分支（src/app.rs:1211-1216）：`t.name = name;` → `apply_task_name(state, &gid, t, name);`。
- `flush_dirty` 不涉及 name，无需改。

### 任务 4：`src/app.rs` — 问题 1：Open 预载本地 .torrent
- 重构 `Message::OpenTaskFile`（src/app.rs:1721-1762）分支顺序：
  1. `let path = t.save_dir.join(&t.name);`
  2. 新分支（放在 `is_bt` 判断**之前**）：`if path.exists() && (crate::engine::is_torrent_url(&t.name) || t.name.starts_with("[METADATA]"))`：
     ```rust
     let default_dir = if t.save_dir.as_os_str().is_empty() {
         state.settings.download_dir.clone()
     } else {
         t.save_dir.clone()
     };
     state.add_dialog.save_picker.close_history();
     state.add_dialog.open(default_dir, state.settings.split);
     state.add_dialog.set_torrent_path(path.to_string_lossy().to_string());
     state.add_dialog.active_tab = AddTab::Torrent;
     return Task::none();
     ```
     （`open()` 会重置 `active_tab` 与 `torrent_upload`，因此顺序必须是 open → set_torrent_path → active_tab；`set_torrent_path` 解析失败会置 `torrent_parse_failed`，对话框内可见，无需 toast。）
  3. 保留 `is_bt` 分支（磁链尚未产出文件时仍预填 magnet/URL，src/app.rs:1725-1743 现有逻辑）。
  4. 保留普通文件打开 + 缺失 toast 分支。
- 说明：`[METADATA]` 前缀判断兼容元数据文件无 `.torrent` 扩展名的情况。

### 任务 5：`src/engine.rs` — 问题 2a：移除确认后再存会话
- 重写 `remove_task_from_aria2`（src/engine.rs:532-537）：
  ```rust
  async fn remove_task_from_aria2(client: &Client, gid: &str) {
      if client.remove(gid).await.is_err() {
          let _ = client.force_remove(gid).await;
      }
      let mut gone = false;
      for _ in 0..25 {
          if client.tell_status(gid).await.is_err() {
              gone = true;
              break;
          }
          if client.force_remove(gid).await.is_ok() {
              // keep polling
          }
          tokio::time::sleep(Duration::from_millis(200)).await;
      }
      let _ = gone.then(|| ());
      let _ = client.remove_download_result(gid).await;
      if !gone {
          tracing::warn!(?gid, "remove: task still present after grace period");
      }
  }
  ```
  - 目标：`save_session()`（`EngineCmd::Remove`/`RemoveAll`/`FollowTorrent` 的 delete_after）执行前任务确实已从 aria2 移除，避免写回会话。轮询期间对仍未消失的任务补 `force_remove`。
  - `Duration` 已在该文件使用（如 engine.rs:742），无需新 import。

### 任务 6：`src/engine.rs` — 问题 2b：删除时清理父/子关系
- 在 `EngineCmd::Remove`（src/engine.rs:697-711）里，先 `tell_status(&gid)` 取 `followed_by` 与 `belongs_to`，对相关 GID 也执行 `remove_task_from_aria2`：
  ```rust
  let status = client.tell_status(&gid).await.ok();
  let related: Vec<String> = status
      .iter()
      .flat_map(|s| s.followed_by.iter().chain(s.belongs_to.iter()).cloned())
      .collect();
  for other in related {
      if other != gid {
          remove_task_from_aria2(client, &other).await;
      }
  }
  remove_task_from_aria2(client, &gid).await;
  let _ = client.save_session().await;
  ```
  - 解决：删除磁链内容任务 T 时，其父元数据任务 M 不再留在会话中，重启不会再生 T；反向删除 M 时同理会清 T。
  - `remove_download_result` 也应对 related GID 生效（`remove_task_from_aria2` 已含）。
  - 若 `delete_files`，`collect_file_paths(&s)` 仅取主任务文件；related 的文件路径可后续再补，本次保持与现状一致。

## 行为 / 边界

- 问题 1：
  - 未完成（文件不存在）的 `.torrent`/磁链任务 → 走原 is_bt 分支（预填链接）或缺失 toast，不打开对话框。
  - `[METADATA]` 前缀 + 文件存在 → 预载对话框（覆盖 metadata-only 磁链任务）。
- 问题 2：
  - 删除进行中/做种的种子：轮询确认移除后才写会话，重启不再复活。
  - `bt_auto_download=true` 的磁链父子链：删除时连带清理，避免父任务重建已删子任务。
- 问题 3：
  - metadata-only 任务完成、`.torrent` 文件就绪后 → 显示真实种子名（解析一次，缓存于 `t.name`）。
  - 已解析出真名的任务重启后 → `[METADATA]` 事件被保护分支拒绝，名字不回落。
  - 仍在下元数据的任务 → 暂时保持 `[METADATA]`，可接受。

## Validation

1. `cargo build`、`cargo clippy --workspace`（无警告）、`cargo fmt --check`。
2. 手动验证问题 1：
   - 完成一个 `.torrent` URL 下载任务 → 点"打开"/双击名字 → 对话框打开且处于 Torrent 标签页，文件树已加载，保存目录为该任务 save_dir。
   - `bt_auto_download=false` 下添加磁链 → 元数据任务完成后点"打开" → 对话框预载 `[METADATA]...torrent`。
3. 手动验证问题 2：
   - 复现：添加种子/磁链并开始下载或做种，删除任务，立即检查 `<session_path>/session.txt` 是否仍含该 gid（修复前应残留，修复后应无）。
   - 重启应用 → 任务不再出现。
   - `bt_auto_download=true` 场景：删除内容任务 → 重启后不复活。
4. 手动验证问题 3：
   - metadata-only 磁链任务完成后，任务名显示真实种子名（来自解析）。
   - `bt_auto_download=true` 磁链任务下载中重启 → 名字保持已解析真名，不回落 `[METADATA]`。
   - 检查 SQLite `tasks.name` 在名字变化后被更新。

## 风险 / 备注

- `remove_task_from_aria2` 轮询最长 ~5s，删除大体积/做种任务时删除操作略有延迟，可接受。
- `apply_task_name` 在 UI 主线程读一个小 `.torrent` 文件（KB 级），且仅在名字为 `[METADATA]` 前缀时触发一次，风险低。
- `TorrentMeta` 增加 `name` 字段为公开 API 变更，调用方仅 add_dialog（构造 `torrent_files`），不受影响。
- 问题 2b 的 related 清理仅覆盖 `followed_by`/`belongs_to` 直接一层；多层链（罕见）暂不处理。
