# 修复：重新下载时绕过"自动下载磁力与种子内容"开关

## Goal
两个 bug 均由"点击重新下载"触发（用户已确认）：
1. 关闭 `bt_auto_download`（自动下载磁力与种子内容）后，对种子/磁力任务点击重新下载，内容仍被自动下载。
2. 重新下载时，即使新任务显示未完成（等待/获取元数据中），内容仍开始下载，怀疑复用了已完成的种子/元数据。

根因一致：`EngineCmd::Redownload` 与 `EngineCmd::ReaddTask`（重启后 ghost 重挂）在重新添加任务时**没有应用**新增路径 `AddDownload` 已有的 BT 开关逻辑：
- 磁力链接 → 未带 `bt-metadata-only=true`（开关关闭时应只取元数据）；
- `.torrent` 链接 → 未带 `follow-torrent=false`（开关关闭时应只下载种子文件）。

由于 `bt_auto_download` 默认关闭（config.rs:154），这正是默认行为。元数据/种子文件已在本地时，aria2 无需慢速抓取即进入内容下载阶段，表现为"使用了已完成的种子"。

## 已确认决策
- 只改重新下载/重挂路径；本地 `.torrent` 文件上传（`AddTorrent`）保持现状（原功能计划明确排除：显式选文件 = 显式意图）。
- 复用 `AddDownload` 已有的开关语义：开关关闭 → 磁力仅元数据、`.torrent` URL 仅下文件；开关开启 → 完整下载。

## Files & Changes

### 1. `src/engine.rs`
- **`EngineCmd::ReaddTask`**（约 line 116-122）与 **`EngineCmd::Redownload`**（约 line 123-128）：各加字段 `bt_metadata_only: bool`。
- **新增辅助函数**（放在 `is_magnet_url` 旁，约 line 381-383）：
  ```rust
  fn apply_bt_url_options(opts: &mut TaskOptions, url: &str, bt_metadata_only: bool) {
      if is_torrent_url(url) {
          opts.extra_options
              .insert("follow-torrent".to_string(), "false".into());
      }
      if bt_metadata_only && is_magnet_url(url) {
          opts.extra_options
              .insert("bt-metadata-only".to_string(), "true".into());
      }
  }
  ```
- **`AddDownload` URL 循环**（约 line 673-686）：用 `apply_bt_url_options(&mut opts, &url, bt_metadata_only)` 替换现有两段内联 insert（行为不变，去重）。
- **`Redownload` 分支**（约 line 976-1007）：解构新增 `bt_metadata_only`；构建 `options` 后调用 `apply_bt_url_options(&mut options, &url, bt_metadata_only)`（放在 `..Default::default()` 构造之后、`add_uri` 之前）。
- **`ReaddTask` 分支**（约 line 932-975）：同样解构 `bt_metadata_only` 并调用该辅助函数。

### 2. `src/app.rs`
- **`Message::RedownloadTask`**（约 line 694-732）：发送前计算 `let bt_metadata_only = !state.settings.aria2.bt_auto_download;` 并传入 `EngineCmd::Redownload { .. }`；同时 `state.torrent_followed.remove(&gid);`（保证开关开启时，`.torrent` URL 任务重下完成后能再次自动跟随创建内容任务，修复二次完成后不跟随的隐患）。
- **`SyncComplete` ghost 重挂**（约 line 1116-1161）：构造 `ReaddTask` 前计算同一 `bt_metadata_only`，加入 `ghost` 元组并传入 `EngineCmd::ReaddTask`。

### 3. `AGENTS.md`
- 更新通道协议片段：`ReaddTask { gid, url, save_dir, split, paused, bt_metadata_only }` 与 `Redownload { gid, url, save_dir, split, bt_metadata_only }`。

## 行为 / 边界
- 磁力任务重新下载 + 开关关闭 → 再次仅取元数据，不下载内容；完成后仍产出 `[METADATA]<hash>.torrent`。
- `.torrent` URL 任务重新下载 + 开关关闭 → 只重下种子文件，不创建内容任务（`follow-torrent=false`）。
- 磁力/`.torrent` URL 重新下载 + 开关开启 → 完整内容下载；`.torrent` URL 完成后自动跟随（`torrent_followed` 已清除）。
- 重启后中断任务 ghost 重挂 → 按当前开关语义处理（关闭时磁力保持 metadata-only）。
- 本地 `.torrent` 上传（`AddTorrent`）与 `FollowTorrent` 不变。

## Validation
1. `cargo build`
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. 手动验证：
   - 关闭开关 → 添加磁力（仅元数据完成）→ 点重新下载 → 任务仍仅取元数据，目标目录无内容文件。
   - 关闭开关 → 添加 `.torrent` URL（仅下文件完成）→ 点重新下载 → 只重下种子文件，无内容任务出现。
   - 开启开关 → `.torrent` URL 下载完成自动跟随 → 点重新下载 → 种子文件重下完成后再次自动跟随创建内容任务。
   - 关闭开关时重启（存在中断中的磁力任务）→ ghost 重挂后仍为 metadata-only，不下载内容。
