# BT 追踪器同步：进行中常驻 toast + 30s 超时看门狗 + 按钮恢复

## Goal
修复点击"同步追踪器"后按钮长时间禁用、无任何反馈的问题：
1. 点击同步后立即弹出**不自动关闭**的"同步中"toast 提醒。
2. fetch 在 30s 内完成 → 关闭常驻 toast，按结果弹出 成功 / 部分成功 / 失败 的自动关闭 toast。
3. 30s 仍未完成（看门狗触发）→ 强制关闭常驻 toast、弹出"同步超时"失败 toast、恢复按钮可点击。
4. 同步结束/超时后按钮恢复可再次点击（`syncing_trackers` 复位，按钮不再卡死）。

采用**应用层看门狗**方案（已与用户确认）：即便 reqwest 底层 30s 超时不生效，应用层看门狗也能强制恢复并给出失败反馈。

## Context
- 同步流程：`Message::SyncTrackers`（app.rs:1953）→ `start_tracker_fetch`（app.rs:2991）置 `syncing_trackers=true` 并发起 `Task::perform` 调 `trackers::fetch_sources`；完成回 `Message::TrackersSynced`（app.rs:1972）置 `syncing_trackers=false` 并弹 toast。
- `fetch_sources`（trackers.rs:80）对每个源用 reqwest `timeout(30s)`（trackers.rs:85），`join_all` 等所有源；挂起源需等满 30s。
- 按钮在同步时禁用：`on_press_maybe(if syncing_trackers { None } else { Some(Message::SyncTrackers) })`（settings_page.rs:767）。
- toast 机制（`ui/components/toast.rs`）：`Toast::close_after(None)` → `remaining=None`，永不自动关闭（常驻）；`push_toast`（app.rs:2937）只按 pos+group 清掉 `close_after.is_some()` 的旧 toast，**不会**清常驻 toast，故完成/超时时需显式 `dismiss_toast(id)`。
- `spawn_toast(state, group, kind, msg, close_after, show_close) -> (id, Task)`（app.rs:2963）可创建常驻 toast 并拿到 id。
- 现有 tracker toast key：`bt-tracker-sync-succeed / -partial / -failed / -select-source`（i18n.rs:155-158、434-441）。
- `Duration`/`Instant`/`Task` 已在 app.rs 导入（app.rs:5,11）。

## Changes

### 1. `src/message.rs`
在 `TrackersSynced`（163-166）后新增变体：
```rust
TrackerSyncTimedOut,
```

### 2. `src/i18n.rs` + `i18n/locales/{zh-CN,en}/main.ftl`
新增 2 个 key：
- `Tr::BtTrackerSyncing` → `"bt-tracker-syncing"`
- `Tr::BtTrackerSyncTimeout` → `"bt-tracker-sync-timeout"`

在 Tr 枚举（158 后）、key 映射（441 后）各加两项。ftl 文案：
- zh-CN：`bt-tracker-syncing = 正在同步追踪器…` / `bt-tracker-sync-timeout = 同步超时，请重试`
- en：`bt-tracker-syncing = Syncing trackers…` / `bt-tracker-sync-timeout = Tracker sync timed out, please retry`

### 3. `src/app.rs` — 状态字段
`Remotrix` 结构体新增：
```rust
tracker_sync_toast_id: Option<u64>,
```
`init`（app.rs:169 附近）初始化为 `None`。

### 4. `src/app.rs` — `start_tracker_fetch`（2991）
置 `syncing_trackers=true`；用 `spawn_toast` 弹常驻"同步中"toast（`ToastGroup::Tracker`, `ToastKind::Normal`, `close_after=None`, `show_close=true`），记录其 id 到 `tracker_sync_toast_id`；返回 `Task::batch([fetch_task, timeout_task])`：
```rust
const SYNC_TIMEOUT: Duration = Duration::from_secs(30);
let fetch = Task::perform(
    async move { crate::trackers::fetch_sources(&urls).await },
    |(fetched, failures)| Message::TrackersSynced { fetched, failures },
);
let timeout = iced::time::sleep(SYNC_TIMEOUT).map(|_| Message::TrackerSyncTimedOut);
Task::batch([fetch, timeout])
```
> 若 `iced::time::sleep(...)` 返回类型与 `Task` 不直接兼容，改用 `Task::perform(iced::time::sleep(SYNC_TIMEOUT), |_| Message::TrackerSyncTimedOut)`，以 `cargo build` 为准。

### 5. `src/app.rs` — `Message::TrackersSynced` handler（1972）
在函数开头（原 1973 前）加守卫并关常驻 toast：
```rust
Message::TrackersSynced { fetched, failures } => {
    if !state.syncing_trackers {
        // 看门狗已先超时恢复，忽略迟到结果，避免重复 toast
        if let Some(id) = state.tracker_sync_toast_id.take() {
            dismiss_toast(state, id);
        }
        return Task::none();
    }
    state.syncing_trackers = false;
    if let Some(id) = state.tracker_sync_toast_id.take() {
        dismiss_toast(state, id);
    }
    ... 保留原逻辑（lines/失败/成功/部分 toast 等），删除原来第 1973 行的 `state.syncing_trackers = false;`（已上移） ...
}
```

### 6. `src/app.rs` — 新增 `Message::TrackerSyncTimedOut` handler（放到 `TrackersSynced` 之后）
```rust
Message::TrackerSyncTimedOut => {
    if !state.syncing_trackers {
        return Task::none(); // fetch 已提前完成
    }
    state.syncing_trackers = false;
    if let Some(id) = state.tracker_sync_toast_id.take() {
        dismiss_toast(state, id);
    }
    let mut toast = Toast::new(ToastKind::Error, state.fluent.get(Tr::BtTrackerSyncTimeout))
        .group(ToastGroup::Tracker)
        .close_after(Some(Duration::from_secs(5)));
    toast.id = state.next_toast_id;
    state.next_toast_id += 1;
    push_toast(state, toast);
    Task::none()
}
```

### 7. `settings_page.rs` — 按钮
`syncing_trackers` 复位后按钮自动恢复可点；保持同步期间禁用（防重复触发）。无需改 UI，除非希望同步期间也不禁用——按默认（保持禁用）执行。若后续要支持同步中重试，再放开 `on_press_maybe`，本期不做。

## 不修改
- `fetch_sources` 的超时（保持 30s）、`trackers.rs` 其余逻辑、预设源/自定义源增删逻辑、自动同步 `CheckTrackerAutoSync`（其同样走 `start_tracker_fetch`，自动获得常驻 toast + 看门狗，行为一致）。

## Validation
- `cargo build`
- `cargo clippy --workspace`
- `cargo fmt --check`
- 手动：设置 → BitTorrent → 添加一个会挂起的源（如 `https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_best.txt`）→ 点击同步：
  - 立即出现"正在同步追踪器…"常驻 toast；
  - 30s 内若其他源成功 → 关闭常驻 toast，弹 部分/成功 toast，按钮恢复；
  - 若 30s 无结果 → 看门狗弹"同步超时，请重试"失败 toast，按钮恢复，可再次点击。
- 用 `https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection@master/best.txt`（实测 200 OK）验证成功路径。

## Risks / Notes
- 竞态：看门狗(30s)与 fetch 完成(~30s)先后不定。用 `syncing_trackers` 作为守卫保证只弹一次结果 toast：
  - 若 fetch 先回 → 正常弹结果 toast、复位；随后看门狗看到 `syncing_trackers==false` 直接返回。
  - 若看门狗先触发 → 弹失败 toast、复位；随后 `TrackersSynced` 因 `!syncing_trackers` 走守卫直接返回，不再重复 toast。
- 常驻 toast 需在完成/超时两处都显式 `dismiss_toast`，因为 `push_toast` 不会自动清常驻 toast。
- 新增 i18n key 需三处同步：Tr 枚举、key 映射、两个 ftl 文件，缺一则会编译期/运行期缺 key。
