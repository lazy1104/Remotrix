# 修复：重启后任务无法继续下载（合并逻辑简化）

## 根因

`sync_existing_tasks`（engine.rs:284）对 aria2 中的**每个**任务都发送 `EngineEvent::Added`，但 `Added` 语义上是"新建任务"。`app.rs` 的 `Added` 处理器被迫同时承担"新建"和"合并已有任务"两个职责，导致对已存在任务（来自 DB）仍然覆写 `name`/`url`/`save_dir`。

正确的设计原则（用户原话："通过 aria2 读取未完成任务，和数据库中的合并一下就行，启动暂停直接操作就行了"）：

| 数据归属 | 来源 |
|---|---|
| `name`, `url`, `save_dir`, `added_at` | **DB**（元数据权威，sync 不应触碰） |
| `downloaded`, `total`, `speed`, `status`, `connections` | **aria2**（运行时权威，由 `Progress` 事件更新） |
| pause / resume / remove | 直接 RPC 操作 aria2（用 gid，已是如此） |

`Progress` 事件**已经**正确地只更新运行时字段、不碰元数据（app.rs:555-561）。问题仅在于 `Added` 事件对已有任务多余地覆写元数据。

上一轮修复用了"空值守卫"（`if url.is_empty() { 保留 } else { 覆写 }`），但只要 aria2 返回非空但与 DB 不同的值（例如重定向后的 URI、或 aria2 全局 dir），仍会覆写。守卫式判断既脆弱又偏离"DB 是元数据权威"的原则。

## 修复方案

**核心：对已有任务，`Added` 处理器不做任何元数据覆写，仅标记 dirty。** 运行时状态由紧随其后的 `Progress` 事件负责。

### 1. `src/app.rs` — `Added` 处理器已有任务分支改为 no-op

当前（上一轮的守卫式修复，app.rs:496-510）：
```rust
if let Some(existing) = state.tasks.get_mut(&gid) {
    if !name.is_empty() && name != gid {
        existing.name = name;
    }
    existing.url = if url.is_empty() { existing.url.clone() } else { url };
    existing.save_dir = if dir.is_empty() { existing.save_dir.clone() } else { PathBuf::from(dir) };
    state.dirty.insert(gid.clone());
}
```

改为：
```rust
if let Some(existing) = state.tasks.get_mut(&gid) {
    // DB 是元数据权威：sync 不触碰 name/url/save_dir/added_at。
    // 运行时状态由紧随其后的 Progress 事件更新。
    let _ = existing;
    state.dirty.insert(gid.clone());
}
```

新建任务分支（else 分支，app.rs:511-535）保持不变——它用于 aria2 中存在但 DB 中没有的孤立任务，需要用 aria2 数据创建并持久化。

### 2. `src/engine.rs` — 保留 `name_from_status` 修复

上一轮对 `name_from_status`（engine.rs:249）的修复（空 path 守卫 + URI basename 回退）**保留**。它仅影响新建/孤立任务的名称解析，不触碰已有任务。

### 3. 不需要改动

- `Progress` 事件 / 处理器：已正确（只更新运行时字段）。
- `Pause`/`Resume`/`Remove` 命令：已直接 RPC 操作 aria2，用 gid，无需改动。
- DB schema / `upsert_meta` / `flush`：无需改动。
- `Snapshot`/`Refresh` 路径：已正确（只发 `Progress`）。

## 为什么这能修复"无法继续下载"

上一轮守卫式修复在 aria2 返回非空 `url`/`dir` 时仍会覆写 DB 值。如果 aria2 返回的 `dir` 与用户原始 `save_dir` 不同（例如 aria2-next 在 session 恢复时返回全局 `--dir` 而非任务级 dir），UI 中的 `save_dir` 被覆写为错误值。虽然 pause/resume 用 gid 不受影响，但元数据不一致会导致后续行为不可预测。

改为 no-op 后：已有任务的元数据完全由 DB 决定，aria2 只提供运行时状态。合并语义清晰、无覆写风险。

## 验证

1. `cargo fmt --check`
2. `cargo clippy --workspace`（无新增警告）
3. `cargo build`
4. 流程测试：
   - 添加 HTTP 任务，下载中关闭应用
   - 重启，确认任务列表显示真实文件名（非 gid）
   - 确认任务能继续下载（进度增长）或可暂停/恢复
   - 对 waiting/paused 状态任务重复上述步骤
   - 确认 DB 中 `name`/`url`/`dir` 未被覆写（查 `tasks` 表）

## 不在范围

- 不改 DB schema 或持久化策略
- 不改 aria2 sidecar 启动参数 / session 机制
- 不新增 `Synced` 事件（`Added` no-op + `Progress` 已足够简单）
- 不处理"DB 有任务但 aria2 session 丢失"的自动重加（session 持久化由 aria2 `--save-session` 负责，属另一问题）
