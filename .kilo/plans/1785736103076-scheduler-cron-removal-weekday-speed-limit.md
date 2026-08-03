# 去掉 cron，星期多选限速 + 缺失文件定时清理迁移

## 目标

1. 彻底移除 cron 机制（`croner` 依赖、`ScheduledTask`/`ScheduledAction`/`parse_cron`、`Settings.schedules`、孤儿消息 `Message::CheckMissingFiles`）。
2. 给现有限速时间窗配置（`SpeedLimitSchedule`）增加"周一到周日"多选下拉，星期过滤叠加在时间窗之上。
3. 将缺失文件清理从（死掉的）cron 迁移到引擎 tokio 调度循环：`remove_task_if_files_missing` 开启时每 10 分钟做一次**定向扫描**，保留现有引擎启动/重启时的一次性检查。**不引入 notify**（见设计决策）。
4. **关键约束**：所有定时逻辑（限速时间窗切换 + 缺失文件周期检查）必须留在引擎 tokio 任务 `run_scheduler` 中，不得放到 iced `Subscription`/`time::every`。因为 `run_scheduler` 是 `tokio::spawn` 的独立任务，窗口最小化/挂后台时 tokio runtime 依然运行，iced 的 subscription 不具备这个保证。

## 设计决策

- **weekdays 数据模型**：`SpeedLimitSchedule` 新增 `weekdays: Vec<u8>`（ISO 周一=1 … 周日=7），serde default 为 `[1,2,3,4,5,6,7]`（旧配置缺字段自动回退为全选，行为不变）。
- **空选择语义**（已与用户确认）：`weekdays.is_empty()` 视为每天生效（不做星期过滤），防御性避免限速静默失效。
- **生效条件**：`enabled && in_speed_window(start,end,now) && weekday_active(weekdays,now)`。
- **缺失文件清理机制**（已与用户确认：10 分钟可接受）：引擎调度循环内固定每 600s 触发一次定向扫描；保留 app.rs:1375 引擎启动/重启路径。不加新 UI/设置项。
- **不用 notify**（已评估，明确排除）：检测对象是"已完成任务全部文件消失"的**状态**而非事件；notify 收事件后仍需 RPC 核对状态，无法替代"app 关闭期间被删"的启动补检查；且 app 自身删文件（DeleteTask/DeleteAll/cleanup 选项）会产生大量假阳性事件，需要动态 watch 管理 + 去抖 + 跨平台适配，成本与收益不成比例。轮询方案简单、统一、免疫自删噪音。
- **定向扫描**：`check_missing_files` 改为只取 `tell_stopped`（丢弃 active/waiting，它们本就被状态过滤掉），并先用 `tell_stopped(0, 1)` 做廉价跳过——没有任何已停止任务时零扫描直接返回。
- **多选下拉**：iced 无内置多选，复用现有 `ui/components/drop_down.rs` 的 `DropDown`，新增一个 `weekday_select` 组件（underlay=摘要按钮，overlay=7 个 checkbox 行）。
- **打开/关闭状态**：`SettingsUiState` 新增 `schedule_days_menu_open: bool`；underlay 按钮和 dismiss 都发 `Message::ToggleScheduleDaysMenu`。

## 实施任务

### 1. Cargo.toml
- 删除 `croner = "3"`。

### 2. src/scheduler.rs
- 删除：`default_true`、`ScheduledTask`、`ScheduledAction`、`parse_cron`，以及 `parse_cron` 的测试。
- 保留：`parse_hhmm`、`in_speed_window`。
- 新增（`use chrono::Datelike`）：
  ```rust
  pub fn weekday_active(weekdays: &[u8], now: &chrono::DateTime<chrono::Local>) -> bool {
      weekdays.is_empty()
          || weekdays.contains(&(now.weekday().number_from_monday() as u8))
  }
  ```
- 为 `weekday_active` 补测试（空=全选、命中、未命中）。

### 3. src/config.rs
- `use crate::scheduler::{in_speed_window, ScheduledTask}` → `use crate::scheduler::{in_speed_window, weekday_active}`。
- `SpeedLimitSchedule` 增加字段：
  ```rust
  #[serde(default = "default_schedule_weekdays")]
  pub weekdays: Vec<u8>,
  ```
  并加 `fn default_schedule_weekdays() -> Vec<u8 { vec![1,2,3,4,5,6,7] }`。默认值 `Self` 也填 `vec![1,2,3,4,5,6,7]`。
- `Settings` 结构体删除 `pub schedules: Vec<ScheduledTask>` 字段及 `Default` 里的 `schedules: Vec::new()`。
- `effective_task_options`（约 488-506 行）把窗口判断改为星期感知：
  ```rust
  let now = chrono::Local::now();
  if self.speed_limit_schedule.enabled
      && !(in_speed_window(&self.speed_limit_schedule.start, &self.speed_limit_schedule.end, &now)
          && weekday_active(&self.speed_limit_schedule.weekdays, &now))
  { /* 置 0 限速 */ }
  ```
  否则新任务在排除日/窗口外会带错限速。
- `apply_fields_equal` 无需改（`speed_limit_schedule == other.speed_limit_schedule` 已覆盖新字段）。

### 4. src/engine.rs — run_scheduler 重写
- 删除 cron 加载（1162-1177）与 cron 触发（1202-1219）两段。
- 保留 1s ticker 与限速窗口状态机，窗口判断改为星期感知：
  ```rust
  let cur = in_speed_window(&schedule.start, &schedule.end, &now)
      && weekday_active(&schedule.weekdays, &now);
  if cur && !inside { inside = true; apply_speed_limits(&client, &settings, true).await; }
  else if !cur && inside { inside = false; apply_speed_limits(&client, &settings, false).await; }
  ```
  初始 `inside` 同样加 `weekday_active` 判断（启动补状态，跨天/跨星期开机即正确）。
- 新增周期缺失文件检查（复用现有 `trigger_missing_files_check`，它自带 `MISSING_CHECK_IN_FLIGHT` 防重入）：
  ```rust
  let missing_enabled = settings.remove_task_if_files_missing;
  let mut last_missing_check = tokio::time::Instant::now();
  const MISSING_CHECK_INTERVAL: Duration = Duration::from_secs(600); // 10 分钟
  // 循环内：
  if missing_enabled && last_missing_check.elapsed() >= MISSING_CHECK_INTERVAL {
      last_missing_check = tokio::time::Instant::now();
      trigger_missing_files_check(client.clone(), event_tx.clone());
  }
  ```
- **定向扫描优化 `check_missing_files`（约 505 行）**：改为只扫描 `tell_stopped`（原 `fetch_all_tasks` 拉的 active/waiting 本就会被 `status != Complete` 过滤掉，纯浪费）：
  ```rust
  async fn check_missing_files(client: &Client) -> Vec<String> {
      let probe = client.tell_stopped(0, 1).await.unwrap_or_default();
      if probe.is_empty() {
          return vec![]; // 没有任何已停止任务 → 无已完成任务可查，零成本跳过
      }
      let stopped = client.tell_stopped(-1, 1000).await.unwrap_or_default();
      // 仅对 status == Complete 的任务做文件路径存在性检查（其余逻辑不变）
  }
  ```
  行为等价，但把周期检查的常规成本压到 1 个小 RPC。
- 删除对 `crate::scheduler::parse_cron` / `ScheduledAction` 的引用。
- `EngineCmd::ReloadSchedules`、`EngineCmd::CheckMissingFiles`（app.rs:1375 仍在用）保留。

### 5. src/message.rs
- 删除孤儿消息 `CheckMissingFiles`（138 行）。
- 新增：
  ```rust
  ScheduleDayToggled { day: u8, enabled: bool },
  ToggleScheduleDaysMenu,
  ```
- `SettingKey` 新增 `ScheduleDays`。

### 6. src/app.rs
- 删除 `Message::CheckMissingFiles` handler（2119-2128）。
- `SettingChanged` 增加 `SettingKey::ScheduleDays` 分支：解析 day 后按 `enabled` 增删 `state.settings.speed_limit_schedule.weekdays`（`ScheduleDayToggled` 消息里处理即可，见下）。
- 新增 `Message::ScheduleDayToggled { day, enabled }` handler：在 `weekdays` 中 push/remove。
- 新增 `Message::ToggleScheduleDaysMenu` handler：翻转 `state.settings_ui.schedule_days_menu_open`。
- Reset 路径（245 行）已整体 clone `speed_limit_schedule`，无需额外处理。

### 7. i18n
- `src/i18n.rs` Tr 枚举 + `key()` 增加：
  - `ScheduleDays` → `"schedule-days"`
  - `EveryDay` → `"every-day"`
  - `WeekdayMon/Tue/Wed/Thu/Fri/Sat/Sun` → `"weekday-mon"`…`"weekday-sun"`
- `i18n/locales/en/main.ftl`：
  ```
  schedule-days = Days
  every-day = Every day
  weekday-mon = Mon … weekday-sun = Sun
  ```
- `i18n/locales/zh-CN/main.ftl`：
  ```
  schedule-days = 生效日期
  every-day = 每天
  weekday-mon = 周一 … weekday-sun = 周日
  ```

### 8. 新组件 src/ui/components/weekday_select.rs
- 复用 `drop_down::DropDown`，签名参照 `time_picker.rs` 风格（返回 `Element<'a, M, iced::Theme, iced::Renderer>`）：
  ```rust
  pub fn weekday_select<'a, M>(
      summary: String,
      selected: &'a [u8],
      day_labels: &'a [String; 7],   // index 0 = 周一
      open: bool,
      on_toggle: impl Fn(u8, bool) -> M + 'a,   // (day, enabled)
      on_dismiss: M,
  ) -> Element<'a, M, iced::Theme, iced::Renderer>
  ```
- underlay：仿 `time_picker::picker_button` 样式的按钮，显示 `summary` + 图标；on_press 由调用方传 `Message::ToggleScheduleDaysMenu`。
- overlay：7 行 `row![text(day_label), checkbox(selected)]`，勾选发 `on_toggle(day, !checked)`。
- `DropDown::on_dismiss(on_dismiss.clone()).alignment(Alignment::Bottom).width(...)`。
- 在 `src/ui/components/mod.rs` 注册 `pub mod weekday_select;`。

### 9. src/ui/settings_page.rs
- `SettingsUiState` 新增 `schedule_days_menu_open: bool`，`new()` 初始 `false`。
- `download_view` 的限速时段块（570-600 行内、ScheduleEndTime 行之后）新增：
  ```rust
  setting_row(
      fluent.get(Tr::ScheduleDays),
      weekday_select(
          summary,
          &settings.speed_limit_schedule.weekdays,
          &day_labels,
          settings_ui.schedule_days_menu_open,
          move |day, enabled| Message::ScheduleDayToggled { day, enabled },
          Message::ToggleScheduleDaysMenu,
      ),
  )
  ```
- `day_labels` 从 `fluent.get(Tr::WeekdayMon..Sun)` 构建 `[String; 7]`。
- `summary`：`weekdays.is_empty() || weekdays.len() == 7` 时显示 `fluent.get(Tr::EveryDay)`，否则按序 join 选中天标签。
- import `crate::ui::components::weekday_select::weekday_select` 与 `Tr::ScheduleDays/EveryDay/WeekdayMon…WeekdaySun`。

### 10. 清理与验证
- 全局 grep 确认无残留：`croner`、`ScheduledTask`、`ScheduledAction`、`parse_cron`、`settings.schedules`、`Message::CheckMissingFiles`、`Message` 中 `CheckMissingFiles`。
- 构建检查：
  ```bash
  cargo build
  cargo clippy --workspace      # 不允许 warning
  cargo fmt --check
  cargo test                    # 重点是 scheduler.rs 与 trackers.rs 的测试
  ```

## 风险与注意

- **时间窗位置**：`run_scheduler` 保持为 tokio 任务，不要改到 iced subscription。iced 挂后台时 tokio 任务照常运行。
- **启动补状态**：`inside` 初始化必须含 `weekday_active`，否则"限速日"重开 app 时开机瞬间限速状态错误。
- **effective_task_options** 同步改星期感知，避免新任务限速不一致。
- 旧配置文件含 `schedules` 字段：serde 默认忽略未知字段，无迁移负担；缺 `weekdays` 回退全选，行为与现状一致。
- 定向扫描的 `tell_stopped(0, 1)` 探测在引擎退化/无 sidecar 时不执行（`run_scheduler` 仅在 sidecar 就绪后启动）。
- `ScheduleHint` 文案可顺带改为提及生效日期（可选，非必须）。
