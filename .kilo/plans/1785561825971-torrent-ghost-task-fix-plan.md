# 修复 follow-torrent 自动创建任务的“幽灵”问题（无法暂停/开始、工具栏缺按钮）

## Goal
- 用 URL 添加 `.torrent` 时，aria2 会先后产生两个任务：①先下载 `.torrent` 文件本身，②自动创建种子内容下载。应用应把这两个任务都显示为**两个独立的任务卡片**（不嵌套、各自可控制、工具栏按钮齐全）（已与用户确认）。
- 内容下载任务（②）必须获得正确元数据（dir/url/info_hash），不再是无 url/dir 的“幽灵任务”。
- 设置中已有的“完成后删除种子文件”（`delete_torrent_after_complete`）扩展覆盖该场景：启用时，任务①（下载到的 `.torrent` 文件）完成后自动删除种子文件并移除任务①，只留任务②。
- 孤儿任务（真实 aria2 任务已消失）自动从 UI+DB 清理。

## 根因（依据日志 + DB 实证）
用户把 `https://…/x.torrent` 作为 URL 添加（`AddDownload`→`add_uri`）。aria2 下载完 `.torrent` 文件后按默认 `follow-torrent=true` 自动新建**内容下载任务（独立 gid）**。应用只为 `add_uri`/`add_torrent` 自己发起的任务发送 `Added`；对 aria2 自动创建的任务（follow-torrent、外部 RPC 客户端）**从不发 `Added`**，只在轮询 `Progress` 时走“幽灵任务”分支创建任务，例如 DB 中的幽灵任务：
```
gid=5eb12d63… name="[DORASUB]….mp4" url='' dir='' status='active' downloaded=0 total=1034103907
```
导致：
- 任务② `url`/`dir` 为空 → 文件夹/复制链接按钮置灰（“工具栏少了按钮”）。
- 任务②的真实 aria2 gid 随后消失（源 `.torrent` 任务被暂停/删除、或重启后会话未保存该任务，日志显示重启后只 `synced 3 existing tasks`）→ UI 任务成为永久孤儿，`Pause`/`Resume` 发给不存在的 gid 静默失败（“无法暂停/开始”），无法自动清除。

## Design decisions（已与用户确认）
1. **两个独立任务**：任务①（`.torrent` 文件下载，有 url+dir，本就正常）与任务②（内容下载）都作为独立卡片显示；任务②不再表现为任务①的子任务/幽灵。
2. **引擎发现新 gid 即发 `Added`**：轮询慢速扫描发现新 gid 时，用完整 `Status` 元数据（name/url/dir/info_hash）发 `Added`，让 follow-torrent/外部任务获得正确元数据（文件夹/复制按钮可用）。
3. **`delete_torrent_after_complete` 扩展**（用户确认）：任务①（`.torrent` 文件任务）变为终态且 `followed_by` 非空（即已产生内容任务②）时，若该设置启用 → 删除磁盘上的种子文件（`save_dir.join(name)`）+ 从 UI/DB 移除任务① + `PurgeResults` 清理 aria2 结果；若设置未启用 → 保留任务①（两个独立任务都在）。
4. **会话内孤儿清理**：引擎全量扫描检测到“曾见过、现已不在任何列表中、且非终态”的 gid → 发 `Removed`，应用从 UI+DB 移除。
5. **启动对账**：`SyncComplete` 时移除“不在 aria2、无法重新添加”的非终态幽灵任务（URL 为空且无 info_hash）；**保留终态（完成/出错/移除）历史**，避免 aria2 修剪结果导致已完成记录丢失。
6. **修复 `pending_torrent_path` 竞态**：`Added` 现在可能来自多个来源，全局 `pending_torrent_path` 会被错误的 gid 消费。改为 `AddTorrent` 成功后单独发 `EngineEvent::TorrentAdded { gid, path }`。
7. **种子任务复制 magnet**（用户确认）：`DownloadTask` 新增 `info_hash`（持久化到 DB），复制链接按钮对种子任务复制 `magnet:?xt=urn:btih:<info_hash>`；`SyncComplete` 时带 info_hash 的种子幽灵任务可通过 magnet 重新添加。

## Changes by file

### 1. `src/engine.rs`
- `EngineEvent::Added` 增加 `info_hash: Option<String>`；`EngineEvent::Progress` 增加 `info_hash: Option<String>` 与 `followed_by: Vec<String>`；新增 `EngineEvent::TorrentAdded { gid: String, path: PathBuf }`。
- 新增 `emit_added(event_tx, s)` 辅助函数（从 `Status` 取 name/url/dir/info_hash，发 `Added`）；`sync_existing_tasks` 改为复用它。
- `emit_progress` 增加 `info_hash: s.info_hash.clone()`、`followed_by: s.followed_by.clone().unwrap_or_default()`。
- `EngineCmd::AddTorrent` 成功后除发 `Added` 外，再发 `TorrentAdded { gid, path }`。
- **轮询循环重构**（`on_sidecar_ready` 内）：
  - 快 tick（1s）：`tell_active` → 逐个 `emit_progress`；`get_global_stat`（不变）。
  - 慢 tick（10s）：改用 `fetch_all_tasks`（`tell_active` + `tell_waiting(-1,1000)` + `tell_stopped(-1,1000)`），**仅当三个 RPC 全部成功才执行本扫**，否则整轮跳过（避免误判孤儿）：
    - 构造 `current: HashSet<&str>`（本次全量 gid）。
    - 新增 gid（不在 `seen`）→ `emit_added` + 插入 `seen`。
    - 终态跟踪 `terminal: HashSet<String>`（status ∈ Complete/Error/Removed）。
    - 终态：`stopped_seen` 去重后 `emit_progress` 一次；非终态：`stopped_seen.remove` + `emit_progress`（沿用现状）。
    - **孤儿检测**：`seen` 中存在但 `current` 中没有、且不在 `terminal` 的 gid → `EngineEvent::Removed(gid)`，并从 `seen`/`terminal`/`stopped_seen` 移除。
    - 每轮结尾 `seen.retain(|g| current.contains(g))`、`terminal.retain(|g| current.contains(g))`。
- `ReaddTask`：无需改动（`add_uri` 已支持 magnet），其 `Added` 的 `info_hash` 传 `None`。

### 2. `src/task.rs`
`DownloadTask` 增加 `pub info_hash: Option<String>`。

### 3. `src/db.rs`
- 启动迁移（仿照现有 `upload_speed` 列）：`ALTER TABLE tasks ADD COLUMN info_hash TEXT NOT NULL DEFAULT '';`
- `load_all`：SELECT 增加 `info_hash`，空串→`None`。
- `upsert_meta`：参数增加 `info_hash: &str`，INSERT/`ON CONFLICT ... DO UPDATE SET` 包含 `info_hash=excluded.info_hash`。

### 4. `src/app.rs`
- `Remotrix` 删除 `pending_torrent_path` 字段及 init。
- `EngineEvent::Added` handler：
  - 删除 `if let Some(tpath) = state.pending_torrent_path.take() {...}`。
  - 新建/更新任务时写入 `info_hash`（`Some` 才覆盖）；两处 `db.upsert_meta` 调用带上 `info_hash`（空为 `""`）。
- 新增 `EngineEvent::TorrentAdded { gid, path }` handler：`state.torrent_files.insert(gid, path);`。
- `EngineEvent::Progress` handler：
  - 幽灵任务创建分支带 `info_hash`；`get_mut` 更新时若 `info_hash.is_some()` 则 `t.info_hash = info_hash`。
  - **源任务自动清理**：当 `status` 为终态（`complete`）且 `!followed_by.is_empty()` 时（任务①：`.torrent` 文件已下载并产生了内容任务）：
    - 若 `state.settings.delete_torrent_after_complete` 启用：删除磁盘种子文件 `t.save_dir.join(&t.name)`（`let _ = std::fs::remove_file(...)`），从 `state.tasks`/`task_order`/`dirty`/`paused_gids` 移除并 `db.delete(&gid)`，发送 `EngineCmd::PurgeResults(vec![gid])` 清理 aria2 结果。
    - 否则保留任务①（两个独立任务）。
- `EngineEvent::SyncComplete`：
  - 幽灵 re-add 过滤改为 `!t.url.is_empty() || t.info_hash.is_some()`；url 为空时用 `magnet:?xt=urn:btih:<hash>` 作为 re-add 的 url。
  - 新增**幽灵清理**：对 `!synced_gids.contains(gid)` 且状态为 Waiting/Active/Paused 且 `t.url.is_empty() && t.info_hash.is_none()` 的任务，从 `state.tasks`/`task_order`/`dirty`/`paused_gids` 移除并 `db.delete(&gid)`（若原为 Active，`active_count` 用 `saturating_sub(1)`）。
- `Message::CopyTaskLink(gid)`：`url` 非空复制 `url`；否则 `info_hash` 存在复制 `magnet:?xt=urn:btih:{hash}`；否则 no-op。

### 5. `src/ui/task_list.rs`
`copy_link_btn` 启用条件改为 `!t.url.is_empty() || t.info_hash.is_some()`（tooltip 沿用 `Tr::CopyLink`）。

## Edge cases / failure modes
- 慢扫三个 RPC 有任一失败 → 整轮跳过，不会误发 `Removed`。
- aria2 因 `max-download-result` 修剪终态结果 → 任务不在 `current` 但属于 `terminal` → 不判孤儿，UI/DB 历史保留。
- 源任务自动清理只在 `followed_by` 非空且终态时触发；若极少数时序下首条终态 `Progress` 的 `followed_by` 为空（孩子尚未创建），则该任务保留为普通已完成任务，可手动清理（安全降级）。
- 新 follow-torrent 任务②：快 tick `Progress` 先建“幽灵”（元数据空）→ 慢扫 `Added`（≤10s）补齐 dir/url/info_hash；若真实任务在补齐前消失 → 孤儿检测移除。
- `pending_torrent_path` 竞态消除（`TorrentAdded` 按 gid 精确关联），`delete_torrent_after_complete` 的 AddTorrent 流程（`torrent_files`）行为不变。
- magnet re-add：`continue=true`，已有文件可续传；无 info_hash 的种子幽灵（旧数据）按删除处理。
- 任务①的种子文件路径用 `save_dir.join(name)` 推算；若文件名被 Content-Disposition 改写导致删除失败，`let _ =` 静默忽略，任务仍被移除。

## Validation
1. `cargo build`、`cargo clippy --workspace`（无警告）、`cargo fmt --check`。
2. 手动验证：
   - 添加 `https://…/x.torrent` URL（`delete_torrent_after_complete` 关闭）→ 出现**两个独立任务**：`.torrent` 文件任务（可暂停/继续、文件夹/复制可用）+ 内容下载任务（有 dir，文件夹可用，可暂停/继续，复制出 magnet）。
   - 启用“完成后删除种子文件”→ 重新添加 `.torrent` URL → `.torrent` 文件任务完成并产生内容任务后自动消失，磁盘上的 `.torrent` 文件被删除，只剩内容任务。
   - 暂停/删除源 `.torrent` 任务 → 子任务（若随源消失）在 ≤10s 内被自动清理，不再残留不可控任务。
   - 重启应用 → 无 `url='' dir=''` 的幽灵任务；已完成任务历史仍在。
   - 种子任务重启后仍可复制 magnet（DB 持久化）。
   - 通过文件选择器添加种子 → `delete_torrent_after_complete` 原有行为不变。
   - 旧数据：DB 中 `5eb12d63` 这类幽灵任务在下次启动 `SyncComplete` 对账中被清理。
