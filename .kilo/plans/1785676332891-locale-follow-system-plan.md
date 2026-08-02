# 语言设置：跟随系统 + 双语标题 + 母语选项标签

## Goal
调整语言设置界面：
1. 设置页"语言"分组标题显示双语：中文界面显示 `语言·Language`，英文界面保持 `Language`。
2. 下拉框选项用各语言自己的文字书写：`简体中文`、`English`（以及未来的 日本語、Français 等）。
3. 下拉框新增"跟随系统语言"选项，默认值为跟随系统语言。
4. 所选语言缺少对应翻译时回退到英语（fluent-templates `fallback_language: "en"` 已提供，需验证）。

## Font / 渲染分析（决定保留母语选项标签）
用户担心不同字体下母语文字（如 日本語、한국어、العربية）会显示为豆腐块。已核对本应用依赖的真实渲染链路：

- 渲染器：iced 0.14 + `iced_wgpu` → `cosmic-text 0.15`。应用在 `main.rs:30-32` 注册了 `lucide.ttf` + 内嵌的 `HarmonyOS_Sans_SC_Regular.ttf`，并把默认字体设为 `HarmonyOS Sans SC`。
- cosmic-text 0.15 的 `FontSystem::new_with_fonts`（`cosmic-text/src/font/system.rs:149-163,396-404`）会调用 `db.load_system_fonts()`——系统字体始终被加载进 fontdb。
- 逐字形回退：iced 的 `Shaping::default()` 为 `Auto`（`iced_core-0.14.0/src/text.rs`），`to_shaping` 对非 ASCII 文本（即 CJK 等选项标签）映射为 `Advanced` → cosmic-text 走 `FontFallbackIter`（`cosmic-text/src/shape.rs:302-359`），按 script 查询系统字体补齐缺失字形（如 Han→"Noto Sans CJK SC"、Hiragana/Katakana→"Noto Sans CJK JP"、Hangul→"Noto Sans CJK KR"、Arabic→"Noto Sans Arabic"，见 `cosmic-text/src/font/fallback/unix.rs`）。
- 结论：主字体（HarmonyOS Sans SC）缺的字形会自动回退到系统字体。当前两个语言（简体中文 + English）的字形完全被内嵌的 HarmonyOS Sans SC 覆盖，无任何渲染风险；未来新增语言只要有对应系统字体即可正常显示。
- 备选（如验证中发现某目标系统确实渲染失败）：把该语言选项标签改用"当前界面语言"书写（如简中界面显示 `简体中文 / 英语 / …`），仅需改 FTL 文案，不改代码结构。

## Current State
- `Locale` enum（`src/i18n.rs:15-21`）：仅 `ZhCN`("zh-CN")、`EnUS`("en")；`Default` 返回 `detect_locale()`（首启即固化）。
- `detect_locale()`（`src/i18n.rs:45-56`）：`LANG → LC_ALL → LC_MESSAGES` 优先级（与 POSIX 相反），`zh*` → ZhCN，其余 → EnUS。
- `Fluent::get`（`src/i18n.rs:524-526`）走 `LOCALES.lookup(self.locale.langid(), ...)`。
- 下拉框（`src/ui/settings_page.rs:196-211`）直接发 `Message::LocaleChanged`，即时生效并保存。
- 标题文案来自 FTL `locale` 键：zh `语言` / en `Language`；选项文案来自 `locale-zh`（`中文`）/ `locale-en`（`English`），两语言文件内容相同。
- `app.rs:874-879` 另有字符串路径 `SettingChanged(SettingKey::Locale, ...)` 处理 `"zh-CN"`/默认→EnUS。
- `Locale::label()`（`src/i18n.rs:30-35`）为死代码（`main.rs` 有 `#![allow(dead_code)]`），但新增枚举变体会导致其 match 非穷尽而编译失败，必须处理。

## Changes

### 1. `src/i18n.rs`
- `Locale` 新增变体 `System`，`#[serde(rename = "system")]`。
- `impl Default for Locale` 改为返回 `Locale::System`。
- 新增方法 `pub fn resolved(self) -> Locale`：`System => detect_locale()`，其余返回自身。
- `langid(&self)` 改为基于 `self.resolved()` 返回 `&ZH_CN` / `&EN`（`System` 分支已由 `resolved` 消解）。
- `detect_locale()`：修正环境变量优先级为 POSIX 顺序 `LC_ALL > LC_MESSAGES > LANG`；逻辑保持 `zh*` → `ZhCN`，否则 → `EnUS`。
- 删除 `label()` 死代码（或为 `System` 补一个分支，二选一，删除更干净）。
- `Tr` 枚举（`src/i18n.rs:152-153` 附近）新增 `LocaleSystem`，并在 `key()`（`src/i18n.rs:379-381` 附近）映射为 `"locale-system"`。

### 2. `i18n/locales/zh-CN/main.ftl`
```
locale = 语言·Language
locale-zh = 简体中文
locale-en = English
locale-system = 跟随系统语言
```

### 3. `i18n/locales/en/main.ftl`
```
locale = Language
locale-zh = 简体中文
locale-en = English
locale-system = Follow system
```

### 4. `src/ui/settings_page.rs`（`general_view` 第 195-211 行）
- 选项列表最前面新增：`Labeled { value: Locale::System, label: fluent.get(Tr::LocaleSystem) }`。
- 其余选项不变（`fluent.get(Tr::LocaleZh)` / `Tr::LocaleEn` 已从 FTL 取文案，自动变为 `简体中文` / `English`）。
- `Some(settings.locale)` 作为选中值：`System` 已在选项内，pick_list 正常显示。

### 5. `src/app.rs`
- `SettingChanged(SettingKey::Locale, ...)`（第 874-877 行）新增 `"system" => Locale::System` 分支（`_` 仍回退 EnUS）。
- `LocaleChanged`（第 1687-1690 行）与 `Fluent::new`（第 102 行）无需改动：`langid()` 已解析 `System`。

### 6. 无需改动
- `src/message.rs`、`src/config.rs`（`#[serde(default)]` + `Default` 即满足新装默认 System；旧配置 `"zh-CN"/"en"` 仍可反序列化，用户既有选择保留）。

## Behavior Notes / Decisions
- 旧安装：settings.json 已有 `"locale"`，保持用户既有选择，不迁移到 System（"默认"仅对新配置生效）。
- `System` 每次查询时解析 `detect_locale()`（环境变量读取开销可忽略），系统 locale 运行中变更也生效。
- 系统语言非 `zh*`（如 fr）时 `detect_locale()` 返回 EnUS → 界面为英语，即"缺少对应翻译时使用英语"。
- 未来新增语言时，只需加 `locale-xx` 母语文案（如 `日本語`），并让 `detect_locale()` 增加映射；英文标题各语言文件按 `本地词·Language` 模式填写。

## Validation
```bash
cargo build
cargo clippy --workspace   # 无警告
cargo fmt --check
```
手动验证（`ARIA2_BIN=/bin/true cargo run` 或直接运行）：
1. 全新配置（删除 settings.json）：下拉框默认选中"跟随系统语言"；`LANG=zh_CN.UTF-8` 启动为中文界面，`LANG=en_US.UTF-8` 启动为英文界面。
2. 中文界面下标题显示"语言·Language"，下拉选项为 `跟随系统语言 / 简体中文 / English`；肉眼确认 `简体中文` 与 `English` 均正常渲染、无豆腐块。
3. 切换到"简体中文"后重启，设置保持（settings.json 写入 `"locale":"zh-CN"`）。
4. 切换到"跟随系统"后重启，跟随系统生效（写入 `"locale":"system"`）。
5. 旧 settings.json（`"locale":"en"`）加载不报错，仍为英文。
6. 回退验证（可选）：临时删除 zh-CN 中某键，确认显示 en 译文而非 "Unknown localization key"。
7. （可选）临时把某选项标签改为 `日本語`，确认在装有 Noto CJK 的系统上回退渲染正常；若渲染失败则启用上方"备选"方案。
