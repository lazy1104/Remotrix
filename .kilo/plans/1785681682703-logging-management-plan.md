# Logging Management Plan (app + aria2-next engine)

## Goal

1. 完善日志：为应用（tracing）增加可配置日志级别，并为 aria2-next 引擎增加独立日志文件（`--log` / `--log-level`）。
2. 在 设置 → 高级 中新增「日志管理」区：日志位置、应用/引擎两个级别选择器放在一行（窄窗口自动换行）、下一行放「清除日志」按钮。
3. 检查系统在关键位置补充日志。

## Decisions (already confirmed)

- **引擎日志级别生效方式**：仅在下次启动或用户手动「重启引擎」后生效。`ApplySettings`/`ApplyAndLeaveSettings` **不**因日志级别变化自动重启引擎。设置页在引擎级别与已应用值不一致时显示提示文字。
- **默认级别**：应用 `info`，引擎 `warn`（保持现状的低噪音）。
- **日志文件**（位于 `config::log_dir()`，即 `<data_dir>/logs/`）：
  - 应用：沿用按天滚动命名 `remotrix.log.YYYY-MM-DD`（实现改为自定义按天滚动的 `io::Write`，每次写入以 append 方式重新打开文件 → 清除日志安全）。
  - 引擎：固定文件 `aria2.log`（aria2 以 append 模式打开日志文件，外部 truncate 安全）。
- **应用级别立即生效**：通过 `tracing_subscriber::reload::Layer<EnvFilter, Registry>` 动态重载；`Reset` 时还原。
- 引擎级别 `--log-level` 取值：`debug` / `info` / `notice` / `warn` / `error`（aria2 原生取值）。
- 应用级别取值：`trace` / `debug` / `info` / `warn` / `error`。
- 清除日志 = 将该目录下 `remotrix.log*` 与 `aria2.log*` 全部 **truncate 为 0 字节**（不删除文件，避免删除后正在写入的句柄指向已删除 inode）。

## Files to change

### 1. `src/logging.rs` (new module)

- 常量：`DEFAULT_APP_LEVEL="info"`、`DEFAULT_ENGINE_LEVEL="warn"`、`APP_LOG_FILENAME="remotrix.log"`、`ENGINE_LOG_FILENAME="aria2.log"`。
- `pub fn init() -> Option<WorkerGuard>`：替代原 `main.rs::init_tracing()`。
  - `let cfg = crate::config::load();`，取 `config::log_dir()`。
  - `log_dir()` 为 `None` 时退化为仅 stdout 的 subscriber（沿用现有逻辑）。
  - 有目录时：
    - 自定义 `DailyRollingWriter`（见下）→ `tracing_appender::non_blocking(...)` 获得 worker guard。
    - `let (filter_layer, filter_handle) = reload::Layer::new(EnvFilter::new(cfg.log.app_level))`；
      将 `reload::Handle<EnvFilter, Registry>` 存入 `static APP_FILTER: OnceLock<reload::Handle<EnvFilter, Registry>>`（`Registry = tracing_subscriber::registry::Registry`）。
    - `tracing_subscriber::registry().with(filter_layer).with(fmt::layer().with_target(false)).with(fmt::layer().with_ansi(false).with_target(true).with_writer(non_blocking)).try_init();`
      （注意 `reload::Layer` 必须放在最内层以过滤所有层，handle 的 `S` 参数即 `Registry`，类型可静态存储。）
    - `tracing::info!(level=..., "remotrix logging initialized")`。
  - 返回 guard 由 `main` 持有。
- `pub fn set_app_level(level: &str)`：`if let Some(h) = APP_FILTER.get() { let _ = h.reload(EnvFilter::new(level)); }`。
- `pub fn engine_log_path() -> Option<PathBuf>`：`log_dir().map(|d| d.join(ENGINE_LOG_FILENAME))`。
- `pub fn clear_logs() -> Result<usize, String>`：读取 `log_dir()` 下前缀 `remotrix.log` 和 `aria2.log` 的文件，逐个 `OpenOptions::new().write(true).truncate(true).open(...)` 清零，返回清除的文件数。
- `pub fn app_level_options() -> &'static [&'static str]` = `["trace","debug","info","warn","error"]`。
- `pub fn engine_level_options() -> &'static [&'static str]` = `["debug","info","notice","warn","error"]`。
- `pub fn normalize_engine_level(s:&str)->String`：不在列表中时回退 `DEFAULT_ENGINE_LEVEL`。
- `DailyRollingWriter`：
  - 字段：`dir: PathBuf`、`prefix: &'static str`、`current_date: Mutex<Option<String>>`。
  - 实现 `std::io::Write`：`write` 中计算 `chrono::Local::now().format("%Y-%m-%d")`，日期变化则更新缓存；以 `OpenOptions::new().create(true).append(true)` 打开 `dir/{prefix}.{date}`，`write_all(buf)`。
  - `flush` 返回 `Ok(())`。
  - 每次写入重新打开文件（append），因此 `clear_logs()` 截断后下一次写入会干净地从头开始。

### 2. `src/main.rs`

- 顶部 `mod logging;`（按字母序加入现有 `mod` 列表）。
- 删除内联 `fn init_tracing()`。
- `main()` 改为 `let _log_guard = crate::logging::init();`
- init 后增加启动日志：版本、应用日志级别、日志目录（可用 `config::announce()` 已有输出 + 新增 `tracing::info!(version, level)`）。

### 3. `src/config.rs`

- 新增：
  ```rust
  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct LogPrefs {
      #[serde(default = "default_app_log_level")]
      pub app_level: String,
      #[serde(default = "default_engine_log_level")]
      pub engine_level: String,
  }
  ```
  `default_app_log_level()` 返回 `crate::logging::DEFAULT_APP_LEVEL`；`default_engine_log_level()` 返回 `crate::logging::DEFAULT_ENGINE_LOG_LEVEL`。
- `Settings` 增加字段 `#[serde(default)] pub log: LogPrefs`。
- `Settings::default()` 增加 `log: LogPrefs::default()`。
- `apply_fields_equal` 增加 `&& self.log == other.log`（使级别变更进入 dirty/Apply 判定）。
- `announce()` 中补充 `tracing::info!(?p, "engine log path")`（用 `crate::logging::engine_log_path()`）。

### 4. `src/engine.rs`

- `Sidecar::spawn`（约 line 287 已 `let settings = crate::config::load();`）中，在 ed2k 参数之后追加：
  ```rust
  if let Some(log_file) = crate::logging::engine_log_path() {
      let level = crate::logging::normalize_engine_level(&settings.log.engine_level);
      cmd.arg("--log").arg(log_file);
      cmd.arg("--log-level").arg(level);
      tracing::info!(?log_file, level, "aria2-next log file");
  }
  ```
- 保持现有 `pipe_lines`（stdout/stderr → tracing debug）不变。

### 5. `src/message.rs`

- `Message` 增加 `ClearLogs`。
- `SettingKey` 增加 `AppLogLevel`、`EngineLogLevel`。

### 6. `src/app.rs`

- `SettingChanged` 新增两个分支：
  - `AppLogLevel`：`state.settings.log.app_level = value; crate::logging::set_app_level(&value); tracing::info!(level=%value, "app log level changed");`
  - `EngineLogLevel`：`state.settings.log.engine_level = value;`（只保存，不重启）
- `Message::ClearLogs` 分支：调用 `crate::logging::clear_logs()`；成功 → `tracing::info!(count, "ui: cleared log files")` + toast（`ToastKind::Success`，文案 `Tr::LogsCleared`）；失败 → warn + toast（`ToastKind::Error`，`Tr::LogsClearFailed`）。复用现有 toast 推送方式（同 `Message::ShowToast` 处理逻辑：分配 `next_toast_id` 后 `push_toast`）。
- `revert_apply_settings`（约 line 211）：追加 `state.settings.log = state.applied_settings.log.clone(); crate::logging::set_app_level(&state.settings.log.app_level);`
- `ApplySettings` / `ApplyAndLeaveSettings`：不因日志级别变化重启引擎；在 Apply 日志中记录 `app_log_level` / `engine_log_level`。

### 7. `src/ui/settings_page.rs`

- `advanced_view` 中，在 Performance 区之后、Engine 区之前（或 Engine 区之后）新增「Logging」分组（`group_title(fluent, Tr::Logging, accent)`）：
  1. 日志位置：`labeled_readonly(fluent, theme, fluent.get(Tr::LogLocation), log_dir.to_string_lossy())`（`crate::config::log_dir()`，无则跳过或显示空）。
  2. 日志级别行（一行、自动换行）：
     ```rust
     let controls = row![
         text(fluent.get(Tr::LogLevelApp)),
         pick_list(app_opts, sel_app, |o| Message::SettingChanged(SettingKey::AppLogLevel, o.value))...,
         text(fluent.get(Tr::LogLevelEngine)),
         pick_list(engine_opts, sel_engine, |o| Message::SettingChanged(SettingKey::EngineLogLevel, o.value))...,
     ].spacing(SPACE_XL).wrap().vertical_spacing(SPACE_LG).align_y(Alignment::Center);
     setting_row_auto(fluent.get(Tr::LogLevel), controls.into())
     ```
     其中 `Labeled<String>` 选项由 `fluent.get(Tr::LevelXxx)` 构造（参照 `labeled_pick` 模式，label = 本地化级别名，value = 原始字符串）。
  3. 引擎级别与已应用值不一致（`settings.log.engine_level != applied_settings.log.engine_level`）时，追加小字提示 `fluent.get(Tr::LogLevelEngineHint)`（`text_secondary` 样式）。
  4. 清除日志按钮（换行、独立一行）：
     ```rust
     row![].push(button(text(fluent.get(Tr::ClearLogs)).size(FONT_BODY))
         .on_press(Message::ClearLogs)
         .padding(PADDING_BUTTON_SM)
         .style(theme::style::button::secondary()))
     ```
- 移除 Engine 组中重复的 `Tr::EngineLogFile` 行（位置改由 Logging 组展示）；`Tr::EngineLogFile` key 可保留不用。
- `advanced_view` 已接收 `applied_settings`，可直接比较引擎级别显示提示。

### 8. `src/i18n.rs`

- `Tr` 新增：`Logging, LogLocation, LogLevel, LogLevelApp, LogLevelEngine, ClearLogs, LogsCleared, LogsClearFailed, LogLevelEngineHint, LevelTrace, LevelDebug, LevelInfo, LevelNotice, LevelWarn, LevelError`。
- `key()` 对应新增：
  `logging, log-location, log-level, log-level-app, log-level-engine, clear-logs, logs-cleared, logs-clear-failed, log-level-engine-hint, level-trace, level-debug, level-info, level-notice, level-warn, level-error`。

### 9. `i18n/locales/en/main.ftl` 与 `zh-CN/main.ftl`

- en：
  ```
  logging = Logging
  log-location = Log location
  log-level = Log level
  log-level-app = App
  log-level-engine = Engine
  clear-logs = Clear logs
  logs-cleared = Log files cleared
  logs-clear-failed = Failed to clear log files
  log-level-engine-hint = Takes effect after the engine restarts
  level-trace = Trace
  level-debug = Debug
  level-info = Info
  level-notice = Notice
  level-warn = Warn
  level-error = Error
  ```
- zh：
  ```
  logging = 日志管理
  log-location = 日志位置
  log-level = 日志级别
  log-level-app = 应用
  log-level-engine = 引擎
  clear-logs = 清除日志
  logs-cleared = 日志文件已清除
  logs-clear-failed = 清除日志文件失败
  log-level-engine-hint = 重启引擎后生效
  level-trace = Trace
  level-debug = Debug
  level-info = Info
  level-notice = Notice
  level-warn = Warn
  level-error = Error
  ```

### 10. 补充关键日志（检查系统后按需添加）

- `main.rs`：启动时记录 `app_log_level`、日志目录。
- `logging.rs`：init 成功/退化、`set_app_level` 重载、`clear_logs` 结果。
- `app.rs`：日志级别变更（`SettingChanged` 两分支）、`ClearLogs`、`ApplySettings` 记录日志级别。
- `engine.rs`：spawn 时记录 aria2 日志文件路径与级别；`pipe_lines` 保持 debug。
- 审阅现有 `tracing` 覆盖（engine.rs 已较完整），对明显缺口补充：如 app.rs 中任务状态 → completed/error 的 UI 状态记录（若缺失）、db.rs 写操作成功/失败（若缺失）。以 info/warn 为主，避免噪音。

## Validation

1. `cargo build`（离线构建应成功）。
2. `cargo clippy --workspace`（零警告）。
3. `cargo fmt --check`。
4. 手动：
   - 启动后 `logs/remotrix.log.YYYY-MM-DD` 生成，`logs/aria2.log` 生成（引擎启动后）。
   - 高级 → 日志管理：改应用级别为 debug → 应用 → 应用日志立即变详细；Reset 还原。
   - 改引擎级别 → 应用 → 出现「重启引擎后生效」提示；点「重启引擎」后 `aria2.log` 级别变化。
   - 点「清除日志」→ toast 提示成功，两个日志文件变为 0 字节且后续写入正常（无 NUL 空洞）。

## Risks / Notes

- `reload::Handle<EnvFilter, Registry>` 的类型必须与 `registry().with(filter_layer)...` 的构建一致（`reload::Layer` 放最内层）；若因版本差异遇到类型问题，回退方案：应用级别仅在下次启动生效（去掉 reload，保留保存）。
- `DailyRollingWriter` 需 `Send + 'static` 以传入 `non_blocking`（内部用 `Mutex<Option<String>>`）。
- aria2 日志文件为 append 模式（已验证 aria2 `Logger::openFile` 使用 `BufferedFile::APPEND`），外部 truncate 安全；aria2-next fork 假定保留该行为。
- `apply_fields_equal` 增加 log 字段后，更改级别会在离开设置页时触发未应用确认——符合预期。
- 引擎日志级别不自动重启引擎（用户已确认）。
