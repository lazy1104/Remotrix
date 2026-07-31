# 设置页右侧标题跟随中间栏选中的分类

## Goal
设置页（`src/ui/settings_page.rs`）右侧的大标题目前固定显示 `settings-title`（"设置"/"Settings"）。改为显示中间栏（`src/ui/category_bar.rs`）当前选中的分类名，例如选中"下载"时标题显示"下载"。与 Motrix 等常见设置页一致。

## Context
- 布局：`src/app.rs:1285-1359`，左栏 sidebar（图标）、中栏 `category_bar`（设置页显示"偏好设置"标题 + 6 个分类项）、右栏 `settings_page`。
- `settings_page::view` 已接收 `category: SettingsCategory` 参数（`src/ui/settings_page.rs:73`），无需改动调用方。
- 中栏分类标签使用的 Tr key（`src/ui/category_bar.rs:121-135`）：
  - General → `Tr::General`
  - Download → `Tr::DownloadCategory`
  - BitTorrent → `Tr::BitTorrent`
  - Ed2k → `Tr::Ed2k`
  - Network → `Tr::Network`
  - Advanced → `Tr::Advanced`

## Changes

### 1. `src/ui/settings_page.rs`
- 新增一个私有辅助函数 `settings_title(fluent: &Fluent, category: SettingsCategory) -> String`，将 `SettingsCategory` 映射到上述对应 `Tr` key 并 `fluent.get(key)`。
- 将 `src/ui/settings_page.rs:129` 的 `.push(text(fluent.get(Tr::SettingsTitle)).size(22))` 改为 `.push(text(settings_title(fluent, category)).size(22))`。
- 所需类型已导入（`Fluent`、`Tr`、`SettingsCategory`、`Message`），无需改 imports。

### 2. 清理不再使用的 `Tr::SettingsTitle`
改完后 `Tr::SettingsTitle` 变体不再被引用，会触发 `dead_code` 警告（AGENTS.md 要求 clippy 零警告）：
- `src/i18n.rs`：删除 `Tr::SettingsTitle` 变体（第 93 行）及其 key 映射 `Tr::SettingsTitle => "settings-title"`（第 263 行）。
- `i18n/locales/en/main.ftl`：删除第 34 行 `settings-title = Settings`。
- `i18n/locales/zh-CN/main.ftl`：删除第 34 行 `settings-title = 设置`。

## Verification
```bash
cargo build
cargo clippy --workspace   # 零警告
cargo fmt --check
```
手动验证：切到设置页，点击中栏不同分类，右侧标题应随选中分类变化（如"下载"、"BitTorrent"、"高级"）。

## Risks
- 无行为风险：改动仅影响设置页标题文案。
- `Tr::Preferences`（中栏固定标题）不受影响，继续使用。
