# 系统字体支持：设置中切换字体

## Goal
在“设置 → 外观”中列出系统字体，允许用户选择 UI 默认字体。选择保存到 `settings.json`，**下次启动生效**（用户已确认此方案）。字体修改后在选择处出现“保存并立即重启应用”按钮，点击后自动重启，免去手动重启（用户补充需求）。

## 重启机制可行性（已核实源码）
- 引擎 aria2-next 以 `--stop-with-process <父进程pid>` 启动（`engine.rs:283-284`），应用进程退出时旧引擎自动终止。
- 引擎 RPC 端口用 `find_free_port()` 动态分配（`engine.rs:242`），重启后新实例占用新端口，**不存在端口冲突**。
- 任务通过 `--save-session`（session.txt）+ SQLite 持久化，重启后自动恢复。
- 因此“保存 → 生成新的自身进程 → 退出旧进程”是安全的。

## 背景与关键约束（已核实源码）
- iced 0.14 的 `default_font` 在启动时固化进渲染器（`main.rs:31`），运行期无法热切换，因此选择“下次启动生效”。
- iced 底层 `cosmic_text::FontSystem::new_with_fonts` 会调用 `fontdb.load_system_fonts()`，即**所有系统字体已加载**。`Font::with_name("<系统字体名>")` 在渲染时可直接命中系统字体，无需复制字体文件。
- `Font::with_name` 需要 `&'static str`：配置里的名字在运行时拿到，需 `Box::leak`（每种家族名最多泄漏一次，有界）。
- `fontdb 0.23` 已是 cosmic-text 的传递依赖（默认特性含 `fs`/`memmap`/`fontconfig`，跨平台可枚举系统字体）。直接添加同版本依赖不会产生新下载，**离线构建不受影响**。
- lucide 图标字体使用显式 `Font::with_name("lucide")`，不受 default_font 影响。
- 所选字体缺字（如选了不含 CJK 的字体）时，cosmic-text 按字形回退到其它系统字体，不会崩溃。

## 改动清单（有序）

### 1. `Cargo.toml`
- `[dependencies]` 增加 `fontdb = "0.23"`（复用 lockfile 中已有 0.23.0）。

### 2. `src/ui/theme.rs` — 新增字体工具
- `pub const BUNDLED_FONT_NAME: &str = "HarmonyOS Sans SC";`
- `static FONT_CACHE: Mutex<Option<(String, Font)>>` + `pub fn font_from_family(family: &str) -> Font`：
  - `family.trim()` 为空 → 返回 `Font::DEFAULT`（系统默认，`Family::SansSerif`）。
  - 否则按缓存的 distinct 家族名 `Box::leak` 出 `&'static str` → `Font::with_name(leaked)`。
  - 缓存命中直接返回 `Font`（`Font: Copy`），避免每帧泄漏。
- `static FONT_FAMILIES: OnceLock<Vec<String>>` + `pub fn system_font_families() -> Vec<String>`：
  - `fontdb::Database::new()` + `db.load_system_fonts()`；
  - 收集 `db.faces().filter_map(|f| f.families.first().map(|(n, _)| n.clone()))`（fontdb 保证第一个家族是英文名）；
  - 大小写不敏感排序 + `dedup`。
- 需要 `use std::sync::{Mutex, OnceLock}; use iced::Font;`

### 3. `src/config.rs` — Settings 字段
- `Settings` 增加：`#[serde(default = "default_font_family")] pub font_family: String,`
- `fn default_font_family() -> String { crate::ui::theme::BUNDLED_FONT_NAME.into() }`
- `Default` impl 增加 `font_family: default_font_family()`。
- **不要**加入 `apply_fields_equal`：字体属于即时生效项（与 theme/locale 一致），不参与 Apply/Reset 脏检查。

### 4. `src/main.rs`
- `.default_font(crate::ui::theme::font_from_family(&cfg.font_family))` 替换硬编码 `Font::with_name("HarmonyOS Sans SC")`。
- 保留 `.font(include_bytes!("../fonts/HarmonyOS_Sans_SC_Regular.ttf"))`（作为内置 CJK 兜底字体，始终注册进字体系统）。

### 5. `src/message.rs`
- `Message` 增加 `FontFamilyChanged(String)` 与 `RestartApp`。

### 6. `src/app.rs` — 状态 + 消息处理 + 重启
- `Remotrix` 增加两个字段：
  - `applied_font_family: String`：启动时从 config 读取（`init` 中 `settings.font_family.clone()`），**会话内永不更新**。用于判断“字体是否已修改”。
  - `restart_pending: bool`：初始 `false`。
- 处理 `Message::FontFamilyChanged(family)`：`state.settings.font_family = family;` + `config::save(&state.settings);`。此路径不需要 `rebuild_theme`（字体只能下次启动生效）。
- 处理 `Message::RestartApp`：`state.restart_pending = true; return begin_close(state);` —— 复用现有关闭流程（发送 `EngineCmd::Shutdown`、隐藏窗口、5s 超时兜底）。
- `finalize_close(state)`（app.rs:424）在 `config::save(&state.settings)` 之后、`iced::window::close` 之前，插入：
  - `if state.restart_pending { spawn_detached_self(); state.restart_pending = false; }`
  - `spawn_detached_self()`：`std::env::current_exe()` + 透传 `std::env::args().skip(1)`，`std::process::Command::new(exe).args(args).stdin/out/err(null()).spawn()`，忽略结果。新实例在当前进程退出（含旧引擎经 `--stop-with-process` 终止）后接管。
  - 说明：`finalize_close` 同时覆盖 `EngineStopped`（优雅）与 `ShutdownTimeout`（兜底）两条退出路径；若在兜底路径执行，旧引擎可能尚未退出，但新实例用新 RPC 端口、旧引擎随进程退出终止，故无冲突。

### 7. i18n — `src/i18n.rs` + 两个 locale 文件
- `Tr` 增加四个变体并映射 key：
  - `Tr::FontFamily` → `font-family`
  - `Tr::SystemDefault` → `system-default`
  - `Tr::FontRestartHint` → `font-restart-hint`
  - `Tr::SaveAndRestartApp` → `save-and-restart-app`
- `i18n/locales/en/main.ftl`：
  - `font-family = Font`
  - `system-default = System Default`
  - `font-restart-hint = Font changes take effect after restart`
  - `save-and-restart-app = Save & Restart Now`
- `i18n/locales/zh-CN/main.ftl`：
  - `font-family = 字体`
  - `system-default = 系统默认`
  - `font-restart-hint = 字体更改将在重启应用后生效`
  - `save-and-restart-app = 保存并立即重启`

### 8. `src/ui/settings_page.rs` — general_view 外观分组
- 给 `view` 与 `general_view` 增加参数 `font_restart_required: bool`（由 app.rs `view` 传入 `state.settings.font_family != state.applied_font_family`）。
- 在“外观(Appearance)”分组（`color-mode` 选择之后）追加字体选择：
  - `labeled_pick`，`T = String`，选项：
    1. `Labeled { value: String::new(), label: fluent.get(Tr::SystemDefault) }`
    2. `Labeled { value: BUNDLED_FONT_NAME.into(), label: BUNDLED_FONT_NAME.into() }` —— **应用内置字体恒在列表中显示**（“HarmonyOS Sans SC”）。它从 `include_bytes!` 注册进字体系统（`main.rs` 的 `.font(...)`），不依赖系统安装，`font_from_family("HarmonyOS Sans SC")` 总是可解析，因此永远可选。若系统字体枚举中恰好也出现该名字（用户系统级安装过），则跳过第 3 步中同名项，避免下拉框出现重复项。
    3. `theme::system_font_families()` 逐项 `Labeled { value: f.clone(), label: f }`（按第 2 步去重后的列表）
  - `selected: Some(settings.font_family.clone())`
  - `on_select: |o| Message::FontFamilyChanged(o.value)`
- 下方加一行预览：`text("AaBb 你好 0123 字体预览").size(FONT_BODY).font(theme::font_from_family(&settings.font_family))`，实时展示所选字体效果。
- 再下方加提示行：`text(fluent.get(Tr::FontRestartHint)).size(FONT_SMALL).style(theme::style::text::secondary)`。
- **重启按钮（新增，按用户要求）**：提示行下方、换行显示，**仅当 `font_restart_required == true` 时渲染**：
  - `button(text(fluent.get(Tr::SaveAndRestartApp)).size(FONT_SMALL)).on_press(Message::RestartApp).padding(PADDING_BUTTON_SM).style(theme::style::button::primary())`
- 细节：`labeled_pick` 默认下拉宽度 180px，字体全名可能较长，可放宽到约 240px（给该控件单独指定宽度或调整 `labeled_pick` 的宽度参数）。

## 边界与风险
- 所选字体之后被卸载 → 渲染时 cosmic-text 回退到系统默认，不崩溃。
- 空值/空白 = “系统默认” → `Font::DEFAULT`。
- `Box::leak` 内存：每个不同家族名最多泄漏一个 `&'static str`，有界、可接受。
- 枚举开销：fontdb 扫描每个进程只做一次（`OnceLock`），设置页可安全每帧调用。
- 离线构建保持：fontdb 已在 registry 缓存与 lockfile 中。
- 中文渲染：内置 HarmonyOS 仍注册在字体系统，选无 CJK 字体时按字形回退。
- 重启安全性：引擎带 `--stop-with-process` + 动态 RPC 端口，新实例启动无端口冲突；任务经 session.txt + SQLite 持久化。重启前 `finalize_close` 已 `config::save` 全量设置（含当前未 Apply 的编辑），避免重启丢失。
- 重启时机：仅在 `finalize_close`（引擎已停或超时兜底）后生成新进程，避免双实例 SQLite 并发写冲突。
- 双实例窗口瞬间叠加：新实例启动到旧窗口消失有极短重叠，可接受（正常快速重启同此）。

## 验证
- `cargo build`（离线可成功）
- `cargo clippy --workspace`（零警告）
- `cargo fmt --check`
- 手动验证：
  1. 启动 → 设置 → 外观 → 修改字体 → 出现“保存并立即重启”按钮；未修改字体时按钮不显示。
  2. 点击按钮 → 应用自动退出并重新启动 → 界面文本使用新字体；中英文正常显示；下载任务仍在。
  3. 预览行实时显示所选字体效果；切回内置 HarmonyOS → 按钮消失。
  4. 选择“系统默认” → 重启后使用系统原生字体。
  5. 字体下拉框中能直接看到并选回内置字体“HarmonyOS Sans SC”（当前默认值在列表中高亮/选中）。

## 明确不做（Out of scope）
- 会话内即时生效（需改造约 143 处 `text()` 及 input/editor/pick_list 默认字体，工作量大）。
- 字体切换后自动重启（仅提供按钮，不自动触发）。
- 自定义字体文件导入（仅系统字体枚举）。
- 字体大小/行高调节。
