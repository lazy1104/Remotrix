# 修复：从 aria2 恢复的 paused/waiting 任务无进度，需 resume 才刷新

## 用户期望行为
- 应用启动后：用本地 DB 中的进度展示（不显示 0%）。
- resume 后：使用 aria2 反馈的真实进度。

## 根因（已通过 DB / 控制文件实测确认）
1. aria2-next 对**在本进程中从未激活过的 paused/waiting 任务**报告 `completedLength=0, totalLength=0`，直到 `unpause` 才解析元数据（既有 plan `1785230298127` 已实测确认）。
2. 启动时 `sync_existing_tasks`（`src/engine.rs:315`）对每个恢复任务发 `Added` + `Progress`。该 `Progress{0,0}` 进入 `src/app.rs` 的 Progress 处理器后**无条件覆写** `t.downloaded/t.total`，把 DB 中上次已知的好进度写成 0/0。
3. 1s 轮询循环（`src/engine.rs:660`）只调 `tell_active()`，paused/waiting 任务不在其中，永远不被刷新 -> 0% 一直停留，直到用户 resume（unpause 后任务进 active 列表被轮询到真实值）。

### 实测证据
- `~/.local/share/remotrix/remotrix.db` 中 paused 任务 `e9aed1e00fa7e816` 现为 `downloaded=0, total=0, status=paused`（已被历史覆写损坏）。
- `~/Downloads/splayer-3.0.0-x86_64.AppImage.aria2`（378B 控制文件）+ 162,068,351B 分片文件均存在 -> 真实进度（~162MB）在磁盘上，aria2 仅因未激活而不上报。

### 既有修复状态
- 既有 plan `1785230298127` 提出 3 项改动。其中**修复3**（Pause/Resume 后立即 `emit_progress`，`src/engine.rs:396-409`）与通知 `Start/Pause` 处理（`src/engine.rs:635-646`）**已应用**。
- **修复1**（app.rs 防御性 Progress：不覆写已知进度）与 **修复2**（轮询 `tell_waiting`）**未应用**。本 plan 仅实现修复1。

## 修复方案（核心：1 处改动）

### 改动：`src/app.rs` Progress 处理器 — aria2 报 `total==0` 视为“元数据未解析”，保留已有进度

当前（`src/app.rs:642-649`）：
```rust
if let Some(t) = state.tasks.get_mut(&gid) {
    t.downloaded = downloaded;
    t.total = total;
    t.speed = speed;
    t.status = TaskStatus::from_engine(&status);
    t.connections = connections;
    state.dirty.insert(gid);
}
```
改为：
```rust
if let Some(t) = state.tasks.get_mut(&gid) {
    if total == 0 && t.total > 0 {
        t.status = TaskStatus::from_engine(&status);
        t.speed = speed;
        t.connections = connections;
    } else {
        t.downloaded = downloaded;
        t.total = total;
        t.speed = speed;
        t.status = TaskStatus::from_engine(&status);
        t.connections = connections;
    }
    state.dirty.insert(gid);
}
```

### 为什么这正好实现用户期望
- **启动展示 DB**：`init()` 从 DB 载入任务（`src/app.rs:88-96`），`t.total>0`。sync 的 `Progress{0,0}` 命中 `total==0 && t.total>0` 分支 -> 保留 DB 的 `downloaded/total`，仅刷新 `status/speed/connections`。UI 显示 DB 真实进度，非 0%。
- **resume 后用 aria2**：resume 使任务 active，轮询 `tell_active` 上报 `total>0` 真实值 -> 走 else 分支覆写为 aria2 值。
- **今后不被覆写**：paused 任务跨重启时 aria2 仍报 0/0，但 DB 已是真实值且本改动保留之 -> 不再损坏。
- **不影响未知大小下载**：未知大小任务 `t.total` 始终为 0，条件 `t.total>0` 为假 -> 走 else 分支，按 aria2 的 `downloaded>0/total=0` 展示，行为不变。
- **不影响已完成任务**：completed 任务 aria2 报 `total>0` -> else 分支正常覆写。

## 当前已损坏 DB 的恢复（一次性，运行时操作，非代码改动）
本改动只能阻止**今后**被覆写；已被历史覆写为 0/0 的 paused 任务（如 `e9aed1e00fa7e816`）DB 仍为 0/0，启动时仍显示 0%，需 resume 一次让其激活解析元数据、落库后再 pause：
1. 启动应用 -> 对该 paused 任务点 resume -> 进度变为真实值（~162MB）-> 等 1s 落库 -> 再 pause。
2. 关闭重启 -> 确认仍显示真实进度（非 0%）、状态 paused。
3. 今后所有 paused 任务跨重启均由本改动保留，无需再 resume。

> 若希望连这一次 resume 也省掉，需改为启动时解析 `.aria2` 控制文件恢复 `totalLength/已完成字节`（用户本次未选择该方案，见“不在范围”）。

## 不在范围
- **修复2（轮询 `tell_waiting`）**：不实现。本改动已满足用户描述的“启动用 DB、resume 用 aria2”流程；paused/waiting 任务的 status 转换已由已应用的通知 `Start/Pause/Complete/Error/Stop/BtComplete`（`src/engine.rs:635-646`）与 Pause/Resume 命令的即时 `emit_progress`（`src/engine.rs:396-409`）覆盖。如后续发现 paused 任务 status/进度需周期刷新，可再补 `tell_waiting` 轮询。
- **解析 `.aria2` 控制文件**恢复进度（零 resume 方案）：不实现。
- `Added` 处理器对已有任务的 no-op（`src/app.rs:595-597`）保持不变。
- DB schema / `flush` / session 机制不变。

## 验证
1. `cargo fmt --check`
2. `cargo clippy --workspace`（无新增警告）
3. `cargo build`
4. 流程测试：
   - 先恢复当前损坏任务：启动 -> 对 paused 任务 resume -> 进度变真实（~162MB）-> 等 1s -> pause。
   - 关闭应用 -> 重启 -> 确认该任务仍显示真实进度（非 0%）、状态 paused、按钮为 resume。
   - 新增一个下载并 active 中 -> 关闭 -> 重启 -> 确认 active 任务进度正确（轮询 `tell_active` 正常）。
   - resume 一个 paused 任务 -> 确认进度切换为 aria2 实时值并持续更新；pause -> 进度定格、按钮翻转。
5. 查 DB：`sqlite3 ~/.local/share/remotrix/remotrix.db "SELECT gid,downloaded,total,status FROM tasks;"` -> paused 任务 `downloaded/total` 保持非零真实值，不再被覆写为 0。
