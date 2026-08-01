# 修复 change_global_option 客户端反序列化失败（全局设置未确认生效）+ .torrent follow 无速度诊断结论

## 诊断结论（本次调查）

用户报告：.torrent URL follow 出的内容任务（gid `8f765194b30dcf21`）一致没有速度（aria2-stdout 一直 `CN:0 SD:0 DL:0B`）。

### 结论 1：follow 流程本身无回归
- 任务① (.torrent URL, `18cb87ad75994aaf`) 正常下载完成；FollowTorrent 正确创建任务②（`8f765194b30dcf21`），dir、info_hash 均正确（DB 与 aria2 实际 `infoHash` 一致）。
- 任务② 的 `.torrent` 文件**独立运行 aria2-next 60 秒（DHT 开启）同样 `CN:0 SD:0`** —— 该种子（2016 年 Doraemon 番剧）是**死 swarm**，tracker 全部超时不可达，无 peer/seeder。**非本应用 bug，无需改代码**。

### 结论 2：真实 bug —— `change_global_option` 每次启动都报错
- 每次 boot 日志：`boot apply global options: aria2: json error: invalid type: string "OK", expected unit`（7-31 起已存在，非本次改动引入）。
- 根因：aria2-ws 0.5.1 `change_global_option`（method.rs:206）用 `call_and_wait::<()>` 反序列化响应，但 aria2-next 对 `changeGlobalOption` 返回字符串 `"OK"`。
- **实测**：RPC 服务端实际已生效（`max-overall-download-limit=1024`、`seed-ratio=2.5` 写入成功），仅客户端解析报错 → 应用**无法确认**全局设置（限速/DHT/bt-tracker/max-concurrent-downloads/user-agent）是否生效，且每次启动/改设置打误导性 WARN。
- 附带确认：`Client` Deref 到 `InnerClient`，`call_and_wait` 为 `pub`，可在 engine 内直接以 `String` 类型调用绕过该 bug；`pause_all`/`shutdown`/`save_session`/`remove_download_result` 均已用 `call_and_wait::<String>` 正确处理 `"OK"`，只有 `change_global_option` 漏了。

## Goal
修复全局选项客户端反序列化，使 `ApplyAria2Options`（运行时改限速/设置）与 boot 应用全局选项能够正确确认成功/失败，消除误导性 WARN，并为将来依赖该结果的功能打基础。不改 aria2-ws 依赖（保持离线构建），在 engine.rs 内用底层调用替代。

## Changes by file

### `src/engine.rs`
- 新增 helper：
  ```rust
  async fn apply_global_options(client: &Client, options: TaskOptions) -> Result<(), String> {
      let params = serde_json::to_value(options).map_err(|e| format!("serialize options: {e}"))?;
      client
          .call_and_wait::<String>("changeGlobalOption", vec![params])
          .await
          .map(|_| ())
          .map_err(|e| format!("change_global_option: {e}"))
  }
  ```
  （`TaskOptions` 已 `Serialize`；`call_and_wait` 经 Deref 访问，需确认 `InnerClient::call_and_wait` 可见 —— 已核实 pub。）
- `EngineCmd::ApplyAria2Options` 分支（engine.rs ~843）：`client.change_global_option(options.clone()).await` → `apply_global_options(client, options).await`，错误仍 `tracing::warn`（真实失败才报）。
- `on_sidecar_ready` 的 boot 应用（engine.rs ~974）：`boot_client.change_global_option(opts).await` → `apply_global_options(&boot_client, opts).await`，错误 `tracing::warn` 保留。
- 可选：`serde_json` 已在依赖中；无需新依赖。

## 不改动
- aria2-ws 依赖、Cargo.toml。
- `add_torrent_and_emit` / FollowTorrent / `follow-torrent=false` / app.rs 的 follow 触发 —— 经诊断确认工作正常。
- 死 swarm 场景不做代码处理（种子无 seeders 属网络/资源问题；用户可手动用选择器添加有源种子验证）。

## Validation
1. `cargo build`、`cargo clippy --workspace`（无警告）、`cargo fmt --check`。
2. 手动：启动应用 → 日志不再出现 `boot apply global options ... json error`；改限速设置后无 WARN。
3. 手动确认全局设置已生效：RPC `aria2.getGlobalOption` 读回与设置一致（如 `max-overall-download-limit`）。
4. 回归：添加 `.torrent` URL → 任务① 完成 → 任务② 正常创建（dir/info_hash 正确）。
