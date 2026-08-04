# 限制 max_concurrent 上限 + 排队中任务图标

## 目标
1. 给 `max_concurrent`（同时下载数）设置上限 32。
2. 任务列表中，Waiting（排队中）任务用 hourglass 图标 + "排队中" tooltip 标识，与 Active 区分。

## 背景事实（已核实）
- 设置页 `src/ui/settings_page.rs:444` 当前范围为 `1..=u32::MAX`。
- `number_stepper`（`src/ui/components/number_stepper.rs:168/185/426`）会把 +/- 按钮和输入提交的值都钳制到给定范围后才发出 `SettingChanged`，因此**只改 UI 范围即可完全约束用户输入**，`src/app.rs:1211` 的 `n.max(1)` 无需改动。
- 旧 `settings.json` 中若存有 >32 的值（如 200），不会被 UI 捕获，需在下发 aria2 前钳制。
- 图标由 `fonts/icons.toml` 在 build 时经 `iced_lucide::build` 生成 `src/ui/icon.rs` + 子集字体 `fonts/lucide.ttf`（`build.rs` 已监听 icons.toml 变化）。`hourglass` 在 lucide 图标集内存在。
- `Tr::Waiting` 已存在（`src/i18n.rs:68/353`），中英 fluent 均有 `waiting` key（en: "Waiting"、zh-CN: "等待中"）。

## 改动清单

### A. max_concurrent 上限 32

1. **`src/config.rs`**：新增常量
   ```rust
   pub const MAX_CONCURRENT_DOWNLOADS: u32 = 32;
   ```
   （放在 `Settings` 定义附近，跟随现有 `SCREAMING_SNAKE` 命名约定）

2. **`src/config.rs` `to_aria2_task_options`**（约 355-358 行）：钳制下发值，避免旧配置超限
   ```rust
   extra.insert(
       "max-concurrent-downloads".into(),
       Value::String(self.max_concurrent.min(MAX_CONCURRENT_DOWNLOADS).to_string()),
   );
   ```

3. **`src/ui/settings_page.rs:444`**：范围改为
   ```rust
   1..=crate::config::MAX_CONCURRENT_DOWNLOADS,
   ```

### B. 排队中任务图标

1. **`fonts/icons.toml`**：`[icons]` 下新增
   ```toml
   hourglass = "hourglass"
   ```
   重新 `cargo build` 会自动重新生成 `src/ui/icon.rs`（新增 `hourglass()` 函数与 `ALL_ICONS` 条目）和 `fonts/lucide.ttf`（子集加入该字形）。**生成文件一并提交**。

2. **`src/ui/task_list.rs` `task_card` 中 `name_marker`**（当前约 490-502 行，仅 Completed 有 `circle_check` 标记）：增加 Waiting 分支，在文件名前加 hourglass 图标 + "排队中" tooltip。参考现有 Completed 分支与 `tip::standard` 用法：
   ```rust
   let name_marker: Element<'a, Message> = match t.status {
       TaskStatus::Completed => row![
           icon::circle_check().size(FONT_ICON).color(theme::success(theme)),
           name,
       ]
       .spacing(SPACE_SM)
       .align_y(Alignment::Center)
       .into(),
       TaskStatus::Waiting => row![
           tip::standard(
               icon::hourglass().size(FONT_ICON).color(text_secondary),
               text(fluent.get(Tr::Waiting)).size(FONT_SMALL),
               tooltip::Position::Bottom,
           ),
           name,
       ]
       .spacing(SPACE_SM)
       .align_y(Alignment::Center)
       .into(),
       _ => name.into(),
   };
   ```
   所需 imports 已具备：`icon`、`tip`、`text`、`Tr`、`theme::text_secondary`/`theme::success`、`TaskStatus`。

## 验证
1. `cargo build`（会重新生成 icon.rs / lucide.ttf，确认无网络依赖、构建成功）
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. 手动验证：
   - 设置页"最大同时下载数"输入 `999` 并提交 → 被钳制为 32；stepper +/- 到 32 后不可再加。
   - 已有 settings.json 设 `max_concurrent: 200` → 启动后 aria2 收到的 `max-concurrent-downloads` 为 32。
   - 添加 40 个任务 → 仅 32 个 Active；其余 Waiting 卡片文件名旁显示沙漏图标，悬停显示"排队中/等待中"。

## 风险
- 生成文件 `src/ui/icon.rs` 与 `fonts/lucide.ttf` 会被 build 改写，需一并提交（当前仓库即采用此方式）。
- 不修改 `max_concurrent` 的存储类型（仍为 u32）与持久化格式，无需迁移。
