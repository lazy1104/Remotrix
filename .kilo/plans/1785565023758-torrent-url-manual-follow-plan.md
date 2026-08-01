# .torrent URL 自动转种子任务：禁用 aria2 follow，由应用接管两阶段流程

## Goal
用 URL 添加 `.torrent` 时，不再依赖 aria2 的 `follow-torrent` 自动创建（该机制产生无 url/dir 的"幽灵内容任务"，无工具栏）。改为：
1. 任务①：`.torrent` URL 作为**普通下载任务**（带 url/dir，工具栏完整），aria2 侧禁用 follow-torrent（避免自动创建第二个任务）。
2. 任务① 完成后（同一 Progress 事件立即置为 Completed），应用读取已下载的 `.torrent` 文件，走与文件选择器相同的 `add_torrent` 流程，创建**独立的新任务②**（内容下载），带 dir + info_hash（文件夹/复制 magnet 按钮可用）。
3. `delete_torrent_after_complete` 扩展覆盖该场景：启用时任务②添加成功后删除 `.torrent` 文件并移除任务①；禁用时保留任务①（已完成的种子文件）。
4. 顺带修复：任务① 完成后不再出现"仍显示未完成，下一轮进度才变完成"（根因是 aria2 follow 过渡期任务① 短暂报告 `active`/`total=0`，禁用 follow 后过渡消失）。

## 根因
- 上一方案依赖 aria2 `follow-torrent=true` 自动创建内容任务（独立 gid），该任务 url/dir 为空 → 工具栏按钮置灰；且任务① 在 follow 过渡期（下载完 `.torrent` 后转为 BT 元数据阶段）短暂报告 `active`+`total=0`，Progress handler 的 `total == 0 && t.total > 0` 分支将其状态回写为 Active → UI 显示未完成，下一轮进度才变 complete。
- 现改为应用接管：`.torrent` URL 添加时传 `follow-torrent=false`，任务① 是干净的单阶段 HTTP 下载；完成后应用读文件、调 `add_torrent`（与文件选择器 `AddTorrent` 同一路径）创建任务②。

## 已确认决策（与用户确认）
- 任务② 的下载选项使用**设置默认值**（`settings.split` + 空 `TaskAdvancedOptions` + 全局 aria2 选项），不追踪原对话框的 split/advanced。
- **路线确认：应用接管（本方案）**，不采用"直接登记 aria2 follow 子任务"路线。后者（保留 follow-torrent，靠 `emit_added` 补元数据 + magnet 重启重加）虽改动更少且能继承对话框选项，但固有缺陷：①元数据经 10s 慢扫才到，子任务早亡则幽灵永久残留；②子任务不入 session.txt，重启即孤儿，只能靠 magnet 恢复；③follow 过渡期任务① 状态回跳无法根治。应用接管把任务② 生命周期完全控制在本应用内。

## Changes by file

### 1. `src/engine.rs`
- 新增 `pub(crate) fn is_torrent_url(url: &str) -> bool`：`basename(url)` 转小写后以 `.torrent` 结尾（basename 已存在，处理 query）。
- `EngineCmd::AddDownload` handler：循环内每个 url 若 `is_torrent_url(&url)`，则 `options.extra_options.insert("follow-torrent", serde_json::Value::String("false".into()))` 后再 `add_uri`（`options` 每 URL clone，需在循环内按 url 设置）。
- 新增 `EngineCmd::FollowTorrent { gid: String, path: PathBuf, save_dir: PathBuf, split: u16, advanced: TaskAdvancedOptions, delete_after: bool }`。
- 抽取共享助手 `async fn add_torrent_and_emit(client, &Path, save_dir, split, advanced, event_tx) -> Result<String, String>`：读字节 → `add_torrent` → `tell_status` 成功后 `emit_added`+`emit_progress`，失败则回退内联 `Added`（info_hash: None），返回新 gid。`EngineCmd::AddTorrent` 与 `FollowTorrent` 共用（AddTorrent 先发 `TorrentAdded`，FollowTorrent 不发）。
- `FollowTorrent` handler：
  - 读文件失败（不存在/已删除）→ `tracing::warn`，保留任务①，返回 Ok（不删文件、不删任务①，用户可手动用选择器添加）。
  - 调用 `add_torrent_and_emit` 成功创建任务②（**不**发 `TorrentAdded`，避免注册进 `torrent_files`——URL 流的清理在这里完成，而非内容完成时）。
  - `delete_after == true`：`let _ = tokio::fs::remove_file(&path)`；`remove_task_from_aria2(client, &gid)`（内部含 remove_download_result）；`let _ = client.save_session().await`；发 `EngineEvent::Removed(gid)`（应用据此清理 UI/DB）。
  - `delete_after == false`：任务① 保留为已完成任务。
- `EngineEvent::Progress` 删除 `followed_by: Vec<String>` 字段；`emit_progress` 同步删除该字段。

### 2. `src/app.rs`
- `Remotrix` 新增字段 `torrent_followed: HashSet<String>` + init 空集合（防止同一任务重复触发 follow）。
- `remove_task_local`：追加 `state.torrent_followed.remove(gid);`（任务移除时清理，避免集合膨胀）。
- `EngineEvent::Progress` handler：
  - 解构去掉 `followed_by`。
  - **删除**原 `followed_by` 源任务清理块（`status == "complete" && !followed_by.is_empty()` 那一段，app.rs ~953-974），由下方新触发逻辑取代。
  - 任务更新块（`get_mut`）**之后**新增 follow 触发（此时 `t.name` 已是最新文件名）：
    ```
    if status == "complete"
        && state.sync_done
        && !state.torrent_followed.contains(&gid)
    {
        if let Some(t) = state.tasks.get(&gid) {
            if !t.url.is_empty() && crate::engine::is_torrent_url(&t.url) {
                state.torrent_followed.insert(gid.clone());
                let path = t.save_dir.join(&t.name);
                let save_dir = t.save_dir.clone();
                send EngineCmd::FollowTorrent {
                    gid: gid.clone(), path, save_dir,
                    split: state.settings.split,
                    advanced: TaskAdvancedOptions::default(),
                    delete_after: state.settings.delete_torrent_after_complete,
                }
                tracing::info!(?gid, "ui: auto-adding downloaded torrent as new task");
            }
        }
    }
    ```
- `AddDownload` / `Added` handler、`db.rs`、`ui/task_list.rs` **不改**（检测基于任务 url，restart 安全）。

## 保留上一方案的功能（不改动）
- 慢扫新 gid → `emit_added`（外部任务/其它 follow 场景元数据补齐）。
- `info_hash` 持久化 + 复制 magnet（任务② 经 `add_torrent` 自带 info_hash）。
- 轮询孤儿检测、`SyncComplete` 幽灵清理/re-add、`TorrentAdded`（选择器流程）、原子 config 保存等。

## Edge cases / 防重复
- `state.sync_done` 门禁：重启后 sync 对"已完成的任务①"重发 complete Progress，`sync_done` 尚为 false → 不触发 follow，避免崩溃窗口（已发 FollowTorrent 但任务② 已存在/未落盘）导致重复添加内容任务。
- 会话内重复 complete（通知 + 慢扫）：`torrent_followed` 保证只触发一次。
- `delete_after` 但 `add_torrent` 失败：不删文件、不移除任务①，用户可手动用选择器添加。
- 失败添加（读文件失败）只 warn 不报错，任务① 保留。
- magnet URL：basename 非 `.torrent` → 不设 follow-torrent=false，行为不变（同一 gid 元数据→内容）。
- 已知局限：URL 不带 `.torrent` 后缀却返回种子的服务器无法在添加时识别 → aria2 仍自动 follow，幽灵问题在该场景保留。

## Validation
1. `cargo build`、`cargo clippy --workspace`（无警告）、`cargo fmt --check`。
2. 手动验证：
   - 添加 `https://…/x.torrent` URL → 只出现一个任务①（可暂停/继续、文件夹/复制可用），aria2 侧无第二个任务。
   - 任务① 完成 → **同一轮**即显示 Completed（不再等下一轮）；自动出现任务②（有 dir + info_hash → 文件夹/复制 magnet 可用）。
   - `delete_torrent_after_complete` 关闭 → 任务① 保留为已完成，`.torrent` 文件保留。
   - 启用该设置 → 任务② 添加成功后任务① 从 UI/DB 移除、磁盘 `.torrent` 文件删除，只剩任务②。
   - 中途暂停/删除任务① → 不触发 follow、不崩溃。
   - 下载中途重启 → 任务① 恢复下载并完成后正常 follow。
   - 任务① 已完成且任务② 已存在时重启 → 不重复添加任务②。
   - 通过文件选择器添加种子 → `delete_torrent_after_complete` 原有行为不变。
