# 修复：同步追踪器在源列表有未应用改动时应禁用（+ 一行持久化修正）

## Goal
设置页的"同步追踪器"在源列表存在**未应用**改动时应被禁用，只有"应用"或"重置"后才恢复。同时补一行持久化修正，避免同步流程把其他未应用草稿改动误写入磁盘。

用户已确认方案：**源变动才禁用按钮 + 补一行持久化修正**（不做全局 dirty 禁用、不改持久化语义）。

## Context
- 设置模型（`src/app.rs`）：
  - `state.settings` = 设置页正在编辑的**草稿**（源增删/开关、`BtTrackerEditor` 等直接就地修改）。
  - `state.applied_settings` = 上次"应用"提交的**已生效**设置（磁盘真实值）。
  - 设置页视图 `settings_page.rs` 同时拿到 `settings`（草稿）与 `applied_settings`（已应用）两个引用（fn 参数 90/93 行）。
  - 同步按钮：`settings_page.rs:767-771`，当前仅 `if syncing_trackers` 时禁用。
  - 同步流程：`SyncTrackers`/`CheckTrackerAutoSync` → `start_tracker_fetch`（读草稿 `state.settings.tracker.sources`）→ `Message::TrackersSynced`（app.rs:1974）写结果。
- 触发 bug 的提交点（app.rs:2012-2015，`TrackersSynced` 内）：
  ```rust
  let now_ms = chrono::Local::now().timestamp_millis();
  state.settings.tracker.last_sync_time = Some(now_ms);
  state.applied_settings.tracker = state.settings.tracker.clone();   // 提交整个 TrackerPrefs 草稿（含已删除源）
  config::save(&state.settings);                                     // 持久化整个草稿（含所有未应用改动）
  ```
- 源列表的每次改动都会改 `tracker.sources`：**两个默认预设 checkbox**（settings_page.rs:701-713，`checked = settings.tracker.sources.contains(url)`，经 `TrackerSourceToggled` app.rs:1913-1921 增删该 URL）与自定义源增删（同时维护 `custom_urls`/`sources`）皆如此。故仅比较 `settings.tracker.sources != applied_settings.tracker.sources` 即可同时覆盖**预设 checkbox 与自定义源**，无需单独枚举 `TRACKER_SOURCE_OPTIONS`。

## Changes

### 1. `src/ui/settings_page.rs` — 同步按钮（767-771）
源列表有未应用改动时禁用（与同步中禁用叠加）：
```rust
.on_press_maybe(if syncing_trackers || settings.tracker.sources != applied_settings.tracker.sources {
    None
} else {
    Some(Message::SyncTrackers)
})
```
- 源列表与已应用一致时按钮正常可用。
- 删除/新增/切换源（含**两个默认预设 checkbox 的勾选/取消**）后（未应用）→ 按钮禁用；"应用"（提交删除）或"重置"（还原）后恢复。
- 沿用现有 `syncing_trackers` 禁用，防止同步中重复触发。

### 2. `src/app.rs` — `Message::TrackersSynced` 持久化修正（2012-2015）
只持久化同步自身输出（`aria2.bt_tracker` + `tracker.last_sync_time`），不再整体提交草稿：
```rust
let now_ms = chrono::Local::now().timestamp_millis();
state.settings.tracker.last_sync_time = Some(now_ms);
state.applied_settings.tracker.last_sync_time = Some(now_ms);
config::save(&state.applied_settings);
```
- 删除 `state.applied_settings.tracker = state.settings.tracker.clone();`（不再提交源列表草稿）。
- 第 2011 行已设 `state.applied_settings.aria2.bt_tracker = ...`，故 `applied_settings` 已含同步结果。
- 保存 `applied_settings`（而非整个 `state.settings`）：即使同步中用户改了其他设置/源，也不会被同步误持久化。

## 不修改
- 源列表抓取来源：保持读草稿 `state.settings.tracker.sources`（与 UI 所见一致）。
- `apply_fields_equal`/`dirty`（Apply 按钮逻辑）、`config::save` 本身、其余设置行为。
- `bt_tracker` 编辑器有未应用手工改动时**不**禁用同步（本期只针对源列表；同步会用抓取结果覆盖，符合"同步"语义）。若需要再放开。

## Validation
- `cargo build`
- `cargo clippy --workspace`
- `cargo fmt --check`
- 手动：
  1. 设置 → BitTorrent → 删除一个自定义源（不点应用）→ 同步按钮应**禁用**；点"应用"后按钮恢复，且被删源已提交。
  2. 不点应用，点"重置" → 源恢复，按钮恢复可用。
  3. 取消勾选/勾选**两个默认预设 checkbox**（不点应用）→ 同步按钮应**禁用**；"应用"/"重置"后恢复。
  4. 源列表无改动时点同步 → 正常执行；完成后按钮仍可用（`last_sync_time` 在草稿与已应用中一致，不产生脏状态）。
  5. 回归：同步结果 tracker 列表更新、toast/看门狗正常。

## Risks / Notes
- `state.applied_settings.tracker.last_sync_time = Some(now_ms)` 使同步后草稿与已应用一致，避免误触发"脏"导致按钮禁用。
- 第 2 项持久化修正与按钮禁用互相独立但都必要：按钮禁用挡住"源改动后直接同步"；持久化修正挡住"同步中/其他未应用改动被误存"。
- 改动点仅 2 处、各 1-3 行，均与现有 `settings`/`applied_settings` 双引用模型一致。
