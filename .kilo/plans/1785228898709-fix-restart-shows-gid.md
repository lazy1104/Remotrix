# 修复：重启后任务列表显示 gid 而非文件名

## 根因

`src/app.rs::init()` 启动时从 DB 加载任务（`db.load_all()`），此时 `name` 字段是正确的文件名。

随后 `src/engine.rs::sync_existing_tasks()` 对 aria2 会话中的每个任务发送 `EngineEvent::Added`，其中 `name` 由 `name_from_status(s)` 计算：

```rust
fn name_from_status(s: &Status) -> String {
    if let Some(file) = s.files.first() {
        let path = Path::new(&file.path);
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            return name.to_string();
        }
    }
    s.gid.clone()   // ← 回退到 gid
}
```

当任务处于 waiting/paused、或 aria2 刚从 session 加载尚未建立连接时，`file.path` 为空字符串，`Path::new("").file_name()` 返回 `None`，于是 `name_from_status` 回退到 `s.gid`。

`app.rs` 的 `EngineEvent::Added` 处理逻辑对已存在任务**无条件覆盖**：

```rust
if let Some(existing) = state.tasks.get_mut(&gid) {
    existing.name = name;   // ← 用 gid 覆盖了 DB 中的正确文件名
    ...
}
```

结果：DB 中保存的正确文件名被 `sync_existing_tasks` 推来的 gid 覆盖，重启后界面显示 gid。

## 修复方案

### 1. `src/engine.rs` — `name_from_status` 增加 URL basename 回退

在回退到 `s.gid` 之前，先尝试从 `s.files[0].uris[0].uri` 提取 basename（复用已有的 `basename()` 函数）。这样即便 aria2 未给出 `file.path`，也能返回真实文件名。

```rust
fn name_from_status(s: &aria2_ws::response::Status) -> String {
    if let Some(file) = s.files.first() {
        let path = std::path::Path::new(&file.path);
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if !name.is_empty() {
                return name.to_string();
            }
        }
        if let Some(u) = file.uris.first() {
            if let Some(b) = basename(&u.uri) {
                return b;
            }
        }
    }
    s.gid.clone()
}
```

### 2. `src/app.rs` — `Added` 不用退化名覆盖已存在任务

对已存在任务（来自 DB），仅在推来的 `name` 是"真实名"（非空且不等于 `gid`）时才覆盖 `existing.name`，避免 gid 覆盖 DB 中的正确名。

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

`url`/`dir` 同理空值不覆盖（"sync 推来空值"是常见情况，不应破坏 DB 既有正确数据）。

## 不在范围

- 不改 DB schema 或持久化策略。
- 不改 aria2 sidecar 启动参数。
- 不动 `upsert_meta`（它只在新增任务时调用，DB 名仍正确）。

## 验证

1. `cargo fmt --check`
2. `cargo clippy --workspace`（无警告）
3. `cargo build`
4. 运行流程测试：
   - 添加一个 HTTP/BT 任务，下载中关闭应用
   - 重新启动，确认任务列表显示真实文件名而非 gid
   - 对 waiting/paused 状态任务重复上述步骤
   - 确认 DB 中 `name` 字段未被 gid 覆盖（可查 `tasks` 表）