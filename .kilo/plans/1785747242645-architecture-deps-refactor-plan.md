# Remotrix 结构与依赖优化计划（全面重构）

## 背景与目标

用户要求检查系统结构与依赖并优化。已确认范围：**全面重构**，包含 **Message 完整子枚举重构**。
现状基线：`cargo clippy --workspace` 无警告、42 个测试通过、工作区干净。

主要问题（分析结论）：
- 依赖：`anyhow` 完全未使用；iced `image` feature 未使用却拉入 image 0.25 重型编解码栈（rav1e/AVIF/exr/tiff/gif/webp），与直接依赖 `image 0.24` 重复编译两份。
- 结构：`app.rs` 3160 行（63 字段 god-state + update() 约 2000 行 + EngineEvent 嵌套 match 约 523 行）；`engine.rs` boot 序列 4 处重复；`Message` 枚举 113 变体且 `SettingChanged(SettingKey, String)` 字符串化。
- 隐患：`#![allow(dead_code)]` 掩盖约 11 处死代码；`settings_page.rs:1581` 每帧 `Box::leak` 16 字节（真实内存泄漏）；`view()` 每帧克隆全部任务；db.rs 静默吞错。

## 执行顺序

按 Phase 顺序执行，每 Phase 结束必须 `cargo build` + `cargo clippy --workspace` 通过。Phase 3 之后追加 `cargo test` 与 `cargo fmt --check`。

---

### Phase 1 — 依赖清理（零行为风险）

1. `Cargo.toml:18` 删除 `anyhow = "1"`（src/ 中 0 处使用，`grep -rn "anyhow" src/` 验证无命中）。
2. `Cargo.toml:14` iced features 删除 `image`：
   - 已确认 `src/` 中无任何 `iced::widget::image` / `iced::image` 使用（logo.rs 用 `svg`、piece_map.rs 用 `canvas`、number_stepper/dialog/drop_down 用 `advanced`，均保留）。
   - `iced` features 变为 `["tokio", "advanced", "canvas", "svg"]`。
3. `Cargo.toml:24` `image` 改为 `image = { version = "0.24", default-features = false, features = ["png"] }`（仅 `main.rs:56` 图标解码使用）。
4. 验证：`cargo tree` 确认 `image v0.25` 及其 rav1e/ravif/exr/tiff 依赖栈消失、`anyhow` 消失；`cargo build` 成功且窗口图标正常（`main.rs:54-59` 逻辑不变）。

### Phase 2 — 死代码清理 + 移除 `#![allow(dead_code)]`

1. `main.rs:1` 删除 `#![allow(dead_code)]`。
2. 迭代修复 clippy 暴露的未使用项，确认删除（均有验证依据）：
   - `task.rs:53-62` `TaskStatus::label()` — 无调用（状态文案走 `Tr::*`）。
   - `db.rs:152-167` `Db::upsert_progress` — 无调用（`flush` 已覆盖）。
   - `updater.rs:12-23` `CheckOutcome` — 无引用。
   - `config.rs:663-669` `UpdatePrefs::skip_version` — 无调用。
   - `message.rs:211` `CloseDialogChoice::MinimizeToTray` — 仅 `close_dialog.rs:29-41` tray 按钮引用，按钮无 `.on_press`（disabled "coming soon"），删除变体并清理按钮或显式禁用文案。
   - `message.rs:188` `Message::ShowToast` — 从未构造（toast 均走 `push_toast`/`spawn_toast`），删除。
   - `toast.rs:99-102` `Toast::position()` builder — 从未调用。**注意保留** `ToastPosition` 枚举 + `ALL`（`toast.rs:122` 渲染循环使用）及除 Top 外变体。
   - `updater.rs:5-7` `ReleaseInfo.notes` / `asset_name` — 从未读取。
   - `app.rs:3074-3093` `spawn_toast` 返回的 `Task` — 恒为 `Task::none()`，签名改为 `()`（或移除返回值）。
   - `engine.rs:1101` `_ => {}` catch-all — 不可达（所有 `EngineCmd` 变体已覆盖或被 supervisor 拦截），改为穷举消除。
3. 若 `RemoveAllRecords` / `PurgeResults` 或类似变体被暴露为死代码，依据同样原则处理（先确认无 UI 入口）。
4. 验证：`cargo clippy --workspace` 零警告。

> 注意：ShowToast/MinimizeToTray 的删除也可与 Phase 5 合并，但删除动作本身独立，先做。

### Phase 3 — 内存泄漏修复 + 设置字段列表去重

1. `number_stepper.rs:220` `number_stepper` 与 `number_stepper_read_only` 签名由 `value: &'a T` 改为按值 `value: T`（`T: Copy` 已满足），widget 结构体存 owned 值。更新 4 处调用点（`settings_page.rs:439, 1423, 1587`、`add_dialog.rs:319`）。
2. `settings_page.rs:1571-1581` `speed_labeled_input`：删除 `Box::leak`，直接传 `display_val`。**修复每帧 16 字节泄漏**。
3. 有界泄漏保留并加注释说明（不属本计划修复范围）：
   - `theme.rs:44` `Box::leak` 为 `Font::with_name(&'static str)` 所需，按字体族缓存有界。
   - `settings_page.rs:375` `Box::leak` 按 locale 缓存有界。
4. 设置 diff 机制统一：
   - `config.rs` `Settings` 及其子结构派生 `PartialEq`。
   - 用 `settings != applied_settings` diff 替换 3 处手写字段列表：
     - `config.rs:570-587` `apply_fields_equal`
     - `app.rs:231-265` `revert_apply_settings`（15 字段手抄）
     - `app.rs:1218-1259` `ApplySettings` 内 diff+apply+restart 逻辑
   - 合并为单一 `fn apply_settings(state: &mut Remotrix) -> bool`（返回是否需要重启），消除 `app.rs:1218-1259` 与 `app.rs:2463-2494` 的重复。
5. 验证：clippy 零警告 + `cargo test` + `cargo fmt --check`。

### Phase 4 — engine.rs 去重

1. 统一 boot 安装序列：4 处 "abort handles → boot → on_sidecar_ready → start_scheduler → emit Applied"（`engine.rs:1498-1512` 初始、`1588-1607` RetryAria2Fetch、`1608-1648` RestartEngine、`1662-1699` 崩溃重启）提取为单一 helper（例如 `async fn install_sidecar(...) -> Result<Sidecar, String>`），各错误事件差异（Aria2FetchFailed vs EngineDegraded）保留为调用侧参数。
2. RestartEngine 与 Shutdown 共用的优雅停机序列（`engine.rs:1522-1546` 与 `1609-1629`：pause_all → 等待 active 清空 → emit_progress → save_session → shutdown）提取 `async fn graceful_stop(client: &Client)`。
3. `ResumeAll`（`engine.rs:774-815`）与 `ResumeGids`（`engine.rs:816-861`）合并为 `resume_staggered(client, gids, tx)`（按 host 分组 + 间隔）。
4. TaskOptions 构造样板（dir+split+max_connection_per_server，`engine.rs:633-638, 691-696, 990-998, 1036-1044`）提取 `fn base_task_options(dir: &Path, split: u16) -> TaskOptions`。
5. 验证：clippy 零警告 + `cargo test`。

### Phase 5 — Message 子枚举重构（改动面最大）

1. `message.rs` 重组为分组枚举（具体归组可微调，保持变体名不变以减少 churn）：

```rust
pub enum Message {
    Nav(NavMsg),
    Add(AddMsg),
    Task(TaskMsg),
    Settings(SettingsMsg),
    Engine(EngineMsg),
    Window(WindowMsg),
    Sort(SortMsg),
    Dialog(DialogMsg),
    Toast(ToastMsg),
    Noop,
}
```

   - `NavMsg`：NavigatePage, SetTaskFilter, SetSettingsCategory, SelectDetailsTab
   - `AddMsg`：OpenAddDialog, CancelAdd, AddDownload, SplitChanged, AddFieldChanged, ToggleAdvanced, PathPicker, PathPicked, SelectAddTab, TorrentUpload, TorrentTreeExpand/Toggle, TorrentFilesSelectAll/None, TorrentFilesScroll, TorrentFilesTogglePanel, UrlEditor, FileHovered, FileDropped, FilesHoveredLeft
   - `TaskMsg`：PauseTask, ResumeTask, RedownloadTask, RemoveTask, DeleteTask, StartAll, PauseAll, DeleteAll, RemoveAllRecords, ClearCompleted, Refresh, OpenTaskDetails, CloseTaskDetails, RefreshTaskDetails, OpenTaskFolder, OpenTaskFile, CopyTaskLink, DetailsTreeExpand/Toggle, DetailsFilesSelectAll/None, DetailsFilesScroll, DetailsFilesFlush, CopyPath
   - `SettingsMsg`：SettingChanged(SettingKey, SettingValue), ApplySettings, ResetSettings, ApplyAndLeaveSettings, DiscardAndLeaveSettings, ThemeModeChanged, ThemeColorChanged, LocaleChanged, FontFamilyChanged, RestartApp, UaEditor, BtTrackerEditor, SyncTrackers, TrackersSynced, TrackerSyncTimedOut, TrackerSourceToggled, TrackerCustomInputChanged, TrackerCustomAdd, TrackerCustomRemove, CheckTrackerAutoSync, SpeedUnitChanged, ToggleScheduleStartPicker, ToggleScheduleEndPicker, ToggleScheduleDaysMenu, ScheduleDayToggled, SetAutoCheck, ClearLogs
   - `EngineMsg`：`Event(EngineEvent)` + CheckAria2Update, RetryAria2Fetch, RestartEngine, ConfirmRestartEngine, EngineRestartCooldownFinished, EngineRestartSafetyTimeout
   - `WindowMsg`：WindowOpened, WindowFocused, WindowResized, WindowMaximized, ClipboardRead, ClipboardParsed, DroppedFileParsed, DragWindow, ResizeWindow, WindowAction, CloseRequested, CloseDialog, ShutdownRequested, ShutdownTimeout, PersistWindowGeometry, FlushDirty
   - `SortMsg`：SortSelected, ToggleSortMenu, CloseSortMenu, ToggleSortOrder, SearchChanged
   - `DialogMsg`：RequestConfirm, ConfirmCancel, OpenAbout, CloseAbout
   - `ToastMsg`：DismissToast, ToastHovered, ToastUnhovered, ToastTick（ShowToast 已在 Phase 2 删除）

2. 新增类型化设置值枚举（消除字符串往返解析）：

```rust
pub enum SettingValue { Num(u64), Bool(bool), Text(String) }
```

   `SettingChanged` 改为此签名；删除 `app.rs:1011/1017/1021/1053` 等处的 `parse()`/`=="true"` 逻辑；`settings_page.rs` 各调用点构造类型化值（调用点本来已持有类型化数据，机械替换）。
3. 更新所有消息构造点：`src/ui/**`（约 20+ 文件）、`app.rs`、`clipboard_watch.rs`、`engine.rs`/订阅流（`app.rs:2797, 2849` 产生 `Message::Engine(EngineMsg::Event(ev))`）。建议按 UI 页面分批改、每批编译一次。
4. `TaskStatus::from_engine`（`message.rs:330-341`）移到 `task.rs`（消除 message.rs 对 task 的宿主 impl）。
5. 验证：clippy 零警告 + `cargo test` + 手动冒烟（详见 Phase 7）。

### Phase 6 — Remotrix 状态子结构 + UI 参数收拢

1. 从 `app.rs:32-95` `Remotrix` 提取内聚子结构（每个带自己的方法，`app.rs` 不再直接触碰内部字段）：
   - `ToastManager`：`toasts, next_toast_id, hovered_toast_id` + `push`/`dismiss`/`spawn` 方法；合并 7 处手动 `toast.id = ...; next_toast_id += 1; push_toast(...)` 样板（`app.rs:1267, 1276, 1996, 2025, 2062, 2100, 2123, 2957`）。
   - `EngineUiState`：`aria2_version, aria2_check_msg, update_pending, aria2_status, aria2_fetch_error, downloading_toast_id, startup_error_toast_id, startup_starting_toast_shown`；顺带合并 `aria2_check_msg` / `aria2_fetch_error` 两个重叠通道。
   - `WindowState`：`maximized, show_close_dialog, window_id, window_size, last_resize, geometry_dirty, pending_close, closing`。
   - `EngineRestartState`：`engine_restart_pending, engine_restart_in_progress, restart_resume_gids`。
   - `TaskTracking`：`paused_gids, synced_gids, removed_gids, sync_done, active_count, dirty, completion_toasted, torrent_files, torrent_followed`。
   - `details_pending_select` + `details_select_gen` 折入 `DetailsDialogState`；`syncing_trackers` + `tracker_sync_toast_id` 折入 `SettingsUiState`。
2. `settings_page::view` 18 参数（`app.rs:2609-2633`）收拢为 `SettingsPageContext` struct（或传 `&Remotrix` + 需要的引用）。
3. `view()` 任务克隆优化（`app.rs:2579-2598`）：filter/sort 改为引用传递（`Vec<&DownloadTask>` 或先对引用排序），避免每帧克隆 `DownloadTask`（5 个 String/struct）后再排序。
4. 验证：clippy 零警告 + `cargo test`。

### Phase 7 — 持久化健壮性 + 收尾验证

1. `db.rs`：错误不再静默吞掉（`let _ = conn.execute` → `tracing::error!` 记录，`db.rs:142, 163, 171, 183, 191`）；`load_all` 失败时记录错误并返回明确结果而非静默空 `Vec`（`db.rs:78-80`）；`Option<Db>` 打开失败时在 UI 顶部弹启动 toast 提示（复用现有 toast 机制）。
2. 解析 `engine ↔ aria2_fetcher` 编译期循环（`engine.rs:14` ↔ `aria2_fetcher.rs:6`）：将 `aria2_fetcher` 收为 `engine` 子模块（`src/engine/aria2_fetcher.rs`），或改由 `updater.rs` 提供中立的 event 类型。低优先级，若改动面大可仅记录并保持现状。
3. 最终验证：
   - `cargo clippy --workspace` 零警告（依赖 `#![allow(dead_code)]` 已移除成为硬性门禁）
   - `cargo fmt --check`
   - `cargo test`（现有 42 个测试全绿）
   - `cargo build --release` 成功
   - 手动冒烟：添加 URL/BT 任务、暂停/恢复/删除、设置修改与应用/放弃、限速窗口、主题与 locale 切换、引擎重启、应用重启恢复任务、关闭对话框、日志清理、追踪器同步。

## 风险与缓解

- **Phase 5 全文件改动**：按 UI 页面分批迁移 + 每批编译；子枚举仅机械包裹、变体名不改，降低回归。
- **engine 重启序列重构**：行为敏感（停机顺序、`generation`、retry_count、事件类型差异），机械提取保持原逻辑与事件差异，不动语义。
- **number_stepper 按值**：需手动验证输入框编辑中 Tree 状态（聚焦/文本）行为不回归；重点测速度限制输入与 AddDialog split。
- **有界 Box::leak 保留**：不追求清零，避免过度优化引入不必要改动。

## 范围外（另行决策）

- 移除 SQLite 持久化（aria2 `--save-session` 已提供持久化）——行为与数据安全风险高，需单独评估。
- 系统托盘实现（iced 0.14 无内置支持）。
- 大列表虚拟化（现有 scrollable + 可见项上限策略已满足）。
