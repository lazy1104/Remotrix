# Plan: 关于页改版（Logo + 名称 + 可复制版本行 + GPL 许可证）

## Goal
重做 `src/ui/about_dialog.rs` 布局，并让之前暂存的 `copyable_text` 组件获得真实使用点（从而移除其 `#[allow(dead_code)]`）：
1. 顶部居中 logo；下方应用名 **ReMotrix**（"Re" 用 primary 色，"Motrix" 用默认色）。
2. 引擎版本与 GUI 版本分别用 `copyable_text` 组件包裹（可点击复制）。
3. 关于页底部显示一行许可证说明；仓库根目录新增 `LICENSE` 文件（GPL-2.0-or-later）。
4. 保留对话框关闭按钮，去掉单独的 "About Remotrix" 标题。

## Decisions
- 许可证：**GPL-2.0-or-later**（维持现状，与 aria2-next 引擎许可一致，避免与 MIT 冲突）。关于页底部显示 + 新增根目录 `LICENSE` 文件。
- 头部：`Dialog::new().with_close(...)` 保留关闭钮，不调用 `.title(...)`；logo 通过 `logo::view` 渲染。
- 复制动作：由于 `copyable_text` 需要 `on_copy: Message`，需重新加入 `Message::CopyText(String)` 变体及其 app.rs 处理器（此前因无使用点被移除，现组件有了真实使用点）。
- `copyable_text` 与 `theme::style::button::copyable()` 现在被使用 → 移除它们的 `#[allow(dead_code)]`。
- 许可证文本本身不作为可复制项（仅版本行可复制）。

## Tasks

### 1. 重新加入复制消息
- `src/message.rs`：在 `Message` 枚举中 `Toast(ToastMsg)` 之后、`Noop` 之前加入 `CopyText(String),`。
- `src/app.rs`：在 `update()` 的 match 中，`Message::Noop => {}` 之前加入：
  `Message::CopyText(s) => return iced::clipboard::write::<Message>(s),`

### 2. 移除 dead_code 标注
- `src/ui/components/copyable_text.rs`：删除文件顶部 `#![allow(dead_code)]`。
- `src/ui/theme.rs`：删除 `pub fn copyable()` 上的 `#[allow(dead_code)]`。

### 3. 重写 `src/ui/about_dialog.rs`
- 签名：`view<'a>(fluent: &'a Fluent, theme: &'a iced::Theme, aria2_version: Option<&'a str>)`（`_theme` 改为 `theme`，因为要取 primary 色）。
- 计算文本：
  - `engine_text`：`Some(v) => format!("Engine: aria2-next v{v}")`，`None => "Engine: aria2-next (--)"`。
  - `gui_text`：`format!("Remotrix {}", env!("CARGO_PKG_VERSION"))`。
- body（居中 `column![].align_x(Alignment::Center).width(Length::Fill)`，`spacing(SPACE_4XL)`）：
  - `logo::view(theme, SIDEBAR_LOGO_W, SIDEBAR_LOGO_H)`（包在 `container(...).center_x(Length::Fill).width(Length::Fill)` 中居中）。
  - 名称行：`row![ text("Re").color(theme::primary(theme)).size(FONT_TITLE), text("Motrix").size(FONT_TITLE) ]`（`spacing(0)`，`align_y(Alignment::Center)`）。
  - `copyable_text(engine_text.clone(), Message::CopyText(engine_text)).into()`。
  - `copyable_text(gui_text.clone(), Message::CopyText(gui_text)).into()`。
  - 底部许可证行：`text(fluent.get(Tr::LicenseNotice)).size(FONT_SMALL).style(theme::style::text::secondary)`。
- 保留 `close_btn`（`button(text(fluent.get(Tr::CloseAbout)).size(FONT_BODY))` + `PADDING_BUTTON_LG` + `theme::style::button::secondary()`）。
- `Dialog::new().width(380.0).with_close(Message::Dialog(DialogMsg::CloseAbout)).body(body).footer(close_btn).build()`，经 `overlay(...)` 包裹。
- 注意：`copyable_text` 返回的 `CopyableText<Message>` 通过 `column.push(...)` 需要 `Into<Element>`，`From` impl 已提供（要求 `Message: Clone + 'a`，`Message` 满足）。

### 4. i18n 许可证字符串
- `src/i18n.rs`：`Tr` 枚举加入 `LicenseNotice`；`key()` 中 `Tr::LicenseNotice => "license-notice"`。
- `i18n/locales/en/main.ftl`：加入 `license-notice = Licensed under GPL-2.0-or-later`。
- `i18n/locales/zh-CN/main.ftl`：加入 `license-notice = 基于 GPL-2.0-or-later 协议开源`。

### 5. 新增 LICENSE 文件
- 仓库根目录新建 `LICENSE`，内容为完整 GPL-2.0-or-later 许可证文本（标准 GPLv2-or-later 全文，可从 gnu.org 的 GPL-2.0.txt 获取，注明 "later version" 措辞）。

### 6. README 引用（可选小改）
- `README.md` License 章节（第 191-193 行）可补充指向 `LICENSE` 文件。若改，保持一行说明即可。

## Validation
- `cargo build` — 无警告（确认 `copyable_text`、`copyable()` 已无 dead_code 告警）。
- `cargo clippy --workspace` — 无警告。
- `cargo fmt --check` — 干净。
- 手动确认：关于页顶部 logo、ReMotrix 名称（Re 为 primary 色）、引擎/GUI 版本可复制、底部许可证行、关闭按钮可用。

## Risks / Notes
- 不要改动 `copyable_text` 组件内部结构（仅移除 allow 属性）。
- 不要新增其他使用点；本计划只把关于页作为唯一使用点。
- `Message::CopyText` 只用于关于页版本复制，与任务详情对话框无关。