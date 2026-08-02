# Logging Settings UI Adjustment (layout, ordering, defaults)

## Goal

Adjust the Logging group in 设置 → 高级 to match user + Motrix conventions:

1. 应用日志级别与引擎日志级别各自单独一行（不再合并到一行换行）。
2. 「清除日志」按钮放到一行的组件栏，左侧 label 栏留空。
3. 应用/引擎默认级别均为 `warn`（最少量的低噪音档，与 Motrix 默认一致）。
4. 两个下拉框选项排序均「从少到多」（最少量在前）：error 在顶部，debug/trace 在底部。

## Decisions (confirmed)

- **默认级别**：`DEFAULT_APP_LEVEL = "warn"`、`DEFAULT_ENGINE_LEVEL = "warn"`（用户已确认，参考 Motrix `'log-level': 'warn'`）。
- **排序（从少到多）**：
  - 应用：`["error", "warn", "info", "debug", "trace"]`
  - 引擎：`["error", "warn", "notice", "info", "debug"]`（debug 排到 info 后面，修复当前 `debug, info, notice, warn, error` 的倒序）
  - 语义参照 Motrix `LOG_LEVELS = ['error','warn','info','verbose','debug','silly']`（error 在最前 = 最少日志）。
- **布局**（`logging_view` 内，顺序）：
  1. 日志位置 readonly 行（`labeled_readonly`，不变）
  2. 应用级别行：`setting_row(Tr::LogLevelApp, app_pick_list)`
  3. 引擎级别行：`setting_row(Tr::LogLevelEngine, engine_pick_list)`
  4. 引擎重启提示（`engine_restart_pending || engine_level != applied`，不变）
  5. 清除日志按钮行：`setting_row(String::new(), clear_button)`（label 栏留空）
- **行标签文案**：`log-level-app` / `log-level-engine` 改为完整短语（"App log level"/"Engine log level"、"应用日志级别"/"引擎日志级别"）；合并的 `log-level` key 不再使用，删除。

## Files / Tasks

### 1. `src/logging.rs`
- `DEFAULT_APP_LEVEL: &str = "info"` → `"warn"`；`DEFAULT_ENGINE_LEVEL` 保持 `"warn"`。
- `app_level_options()` 返回 `&["error", "warn", "info", "debug", "trace"]`。
- `engine_level_options()` 返回 `&["error", "warn", "notice", "info", "debug"]`。
- `normalize_app_level` / `normalize_engine_level` 用 `contains` 判断，顺序无关，无需改动。

### 2. `src/ui/settings_page.rs` — `logging_view`（约 1046 行起）
- 删除合并的 `controls` 包裹行（`row![...].wrap().vertical_spacing(...)`）。
- 两个 picker 分别用 `setting_row` 单行放置：
  - `col.push(setting_row(fluent.get(Tr::LogLevelApp), app_pick.into()));`
  - `col.push(setting_row(fluent.get(Tr::LogLevelEngine), engine_pick.into()));`
  - pick_list 配置（placeholder/width 140/style/menu_style）保持现状。
- 保留日志位置行、引擎重启提示块。
- 清除日志按钮改为：`col.push(setting_row(String::new(), button(...).into()));`
- 若 `setting_row_auto` 不再被本函数使用，仍保留（`theme_color_swatches` 在用）。

### 3. `src/i18n.rs`
- 删除 `Tr::LogLevel` 枚举变体与 `key()` 中的 `Tr::LogLevel => "log-level"`。
- 保留 `LogLevelApp`、`LogLevelEngine`、`LogLevelEngineHint`。

### 4. `i18n/locales/en/main.ftl` 与 `zh-CN/main.ftl`
- 删除 `log-level = ...` 两行。
- 更新：
  - en: `log-level-app = App log level`、`log-level-engine = Engine log level`
  - zh: `log-level-app = 应用日志级别`、`log-level-engine = 引擎日志级别`

## Notes / Caveats

- 默认值改动只影响新建/缺失的 `log` 配置；已保存的 `settings.json` 中的 `log.app_level` 值不会被迁移（保持用户已设值）。
- 语义澄清（供实现确认）：日志级别是「最低阈值」，error 只放行 error（最少），debug/trace 放行更多（最多）；「从少到多」= error 在前。与 Motrix `LOG_LEVELS` 顺序一致。

## Validation

1. `cargo build`
2. `cargo clippy --workspace`（零警告）
3. `cargo fmt --check`
4. 手动：设置 → 高级 → 日志管理：应用/引擎各占一行；下拉框首项为 error、末项为 debug（引擎）/trace（应用）；新配置默认选中 warn；清除日志按钮在组件栏、左侧 label 空；改引擎级别 → Apply → 出现「重启引擎后生效」提示并保持到引擎重启。
