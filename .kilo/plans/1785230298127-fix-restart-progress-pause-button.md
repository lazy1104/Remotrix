# 修复：重启后进度不正确 + 暂停按钮状态不更新

## 根因（已通过日志和 aria2-next 实测确认）

### 问题1：重启后任务不显示正确进度
- 重启时 `sync_existing_tasks`（engine.rs:291）对每个 aria2 任务发 `Added`+`Progress`。
- aria2 对**从未激活过的 paused 任务**报告 `completedLength=0, totalLength=0, path=""`（实测 `tellWaiting` 与 `tellStatus` 均如此，直到任务被 unpause 才解析元数据）。
- 该 `Progress{0,0}` 进入 `app.rs` Progress 处理器后，**覆写**了 DB 中上次已知的好进度（如 18MB/153MB → 0/0）。当前 DB 实测即 `downloaded=0,total=0`。
- 轮询循环（engine.rs:595-610）只调 `tellActive()`，paused 任务不在其中，永远不会被刷新。

### 问题2：进行中暂停时卡片按钮不变
- `Pause`/`Resume`/`PauseAll`/`ResumeAll` 命令（engine.rs:352-380）执行 RPC 后**不发任何事件**。
- 轮询循环只查 `tellActive()`；任务暂停后离开 active 列表，状态永不到达 UI → 按钮停在“暂停”图标。
- （恢复“看似”可用：unpause 后任务回到 `tellActive`，≤1s 内被轮询到 → 变回暂停图标。）

## 修复方案（3 处改动）

### 改动1：`src/app.rs` Progress 处理器 —— 不用 aria2 的“未解析”0/0 覆写已知进度
当前（app.rs:543-550）：
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
改为（aria2 报 `total==0` 视为“元数据未解析”，保留已有 `downloaded/total`，仅刷新运行时状态）：
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
原理：一旦任务被激活过，aria2 即使再次 paused 也继续报真实值（日志已证实 paused 时 `tellStatus` 返回 18MB/153MB）；`total==0` 只出现在“从未激活的 paused 任务”上，此时保留 DB/上次已知进度即可。新任务（existing total==0）走 else 分支，行为不变。

### 改动2：`src/engine.rs` 轮询循环 —— 增加 `tellWaiting` 轮询
当前（engine.rs:595-610）只查 `tell_active`。改为同时查 `tell_waiting`，使 paused/waiting 任务的状态与进度被周期性上报：
```rust
handles.push(tokio::spawn(async move {
    let mut ticker = interval(Duration::from_millis(1000));
    loop {
        ticker.tick().await;
        let mut all = Vec::new();
        if let Ok(list) = poll_client.tell_active().await {
            all.extend(list);
        }
        if let Ok(list) = poll_client.tell_waiting(-1, 1000).await {
            all.extend(list);
        }
        for s in &all {
            emit_progress(&poll_event_tx, s).await;
        }
    }
}));
```
不查 `tell_stopped`（completed/error 由通知订阅 `Complete/Error/Stop` 处理；且 stopped 队列可能积压大量已完成任务，每秒上报会造成 UI 抖动与无谓 DB 写入）。`tell_waiting` 队列仅含 active/paused/queued，规模小，安全。

### 改动3：`src/engine.rs` Pause/Resume/PauseAll/ResumeAll —— 执行后立即上报进度
在 `handle_client_cmd`（engine.rs:352-380）中，每个命令执行 RPC 后追加 `emit_progress`，实现即时按钮反馈（无需等 1s 轮询）：
```rust
EngineCmd::Pause(gid) => {
    tracing::info!(?gid, "pause");
    let _ = client.pause(&gid).await;
    if let Ok(s) = client.tell_status(&gid).await {
        emit_progress(event_tx, &s).await;
    }
}
EngineCmd::Resume(gid) => {
    tracing::info!(?gid, "resume");
    let _ = client.unpause(&gid).await;
    if let Ok(s) = client.tell_status(&gid).await {
        emit_progress(event_tx, &s).await;
    }
}
EngineCmd::PauseAll => {
    tracing::info!("pause all");
    let _ = client.pause_all().await;
    for s in fetch_active_and_waiting(client).await {
        emit_progress(event_tx, &s).await;
    }
}
EngineCmd::ResumeAll => {
    tracing::info!("resume all");
    let _ = client.unpause_all().await;
    for s in fetch_active_and_waiting(client).await {
        emit_progress(event_tx, &s).await;
    }
}
```
新增辅助函数（放在 `fetch_all_tasks` 附近，engine.rs:277）：
```rust
async fn fetch_active_and_waiting(client: &Client) -> Vec<aria2_ws::response::Status> {
    let mut all = Vec::new();
    if let Ok(list) = client.tell_active().await {
        all.extend(list);
    }
    if let Ok(list) = client.tell_waiting(-1, 1000).await {
        all.extend(list);
    }
    all
}
```
（轮询循环也可复用此函数；为减少改动可保持内联或替换为调用，二选一，保持一致即可。）

## 为什么这能修复

- **问题1**：改动1 阻止重启时 `Progress{0,0}` 覆写 DB 已知进度；DB 的 `downloaded/total` 被保留，UI 显示上次真实进度。改动2 保证任务被激活后（即便随后再暂停）其真实进度被持续上报，状态不丢失。
- **问题2**：改动3 在暂停/恢复后即时上报 `status`；改动2 兜底保证 paused 任务在 1s 内被上报。卡片按钮据 `t.status` 渲染（task_list.rs:210-220），状态更新后按钮立即翻转。

## 不改动 / 保持现状
- `Added` 处理器对已有任务的 no-op（上一轮已应用，app.rs:496-500）保持不变。
- `name_from_status`（engine.rs:249）保持不变。
- DB schema / `upsert_meta` / `flush` 不变。
- `Snapshot`/`Refresh` 路径不变（一次性全量，含 stopped，用于手动刷新）。
- 通知订阅（engine.rs:568-591）不变。

## 验证
1. `cargo fmt --check`
2. `cargo clippy --workspace`（无新增警告；已有的 `unused variable: page` in sidebar.rs:11 为既有，忽略）
3. `cargo build`
4. 流程测试（关键）：
   - 当前 DB 已被覆写为 0/0：先**恢复一次**——启动应用，对 paused 任务点恢复让其激活解析元数据，进度变为真实值（~12%），等待 1s 落库；再暂停。
   - 关闭应用，重启 → 确认任务仍显示真实进度（非 0%）、状态为 paused、按钮为“恢复”。
   - 进行中（active）点暂停 → 确认卡片按钮**立即**变为“恢复”图标；点恢复 → 立即变回“暂停”。
   - PauseAll / ResumeAll → 所有卡片按钮同步翻转。
   - 查 DB：`name/url/dir` 未被覆写，`downloaded/total` 保持非零真实值。

## 已知局限 / 不在范围
- 从未激活的 paused 任务在**首次**重启时若 DB 已是 0/0，仍显示 0%（aria2 自身不报元数据，无法在不激活的情况下获取）。需恢复一次以重新填充 DB；此后重启由改动1 保留。不做“启动时自动 unpause-repause 强制解析”等 hack。
- 不改 aria2 session 机制 / 不在 session 中补 `out=` 选项以提前定位控制文件（属另一议题）。
