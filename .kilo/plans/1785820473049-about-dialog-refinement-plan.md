# Plan: 关于页二次改版（应用版本置顶、等宽可复制行、iced 构建行、Homepage 链接、副标题）

## Goal
基于当前已实现的 `src/ui/about_dialog.rs`（logo + ReMotrix + 可复制版本行 + 许可证行）做布局与信息增强：

1. **应用版本行置顶**：`Remotrix 0.1.0` 排在第一个，`Engine: aria2-next ...` 排第二。
2. **可复制行等宽加宽**：两个 `copyable_text` 行都设 `.width(Length::Fill)`，全宽等宽。
3. **版本与许可证之间**加入：`Built with iced {version}` 说明行（普通次要文本，不可复制）+ **Homepage 按钮**（打开仓库 URL）+ 许可证行。
4. 名称行下方增加**副标题 tagline**（i18n）。
5. 新增 `Message::OpenLink(String)` 消息与 handler（复用现有 `open::that` 的 `Task::perform` 模式）。

## Decisions（已与用户确认）
- **Homepage URL**：使用占位常量 `const ABOUT_REPO_URL: &str = "https://github.com/yourname/remotrix";` 放在 `about_dialog.rs`，便于以后修改。
- **iced 行**：普通次要文本（不参与复制），仅版本两行可复制。
- **底部区块顺序**：版本行 → `Built with iced 0.14` 文本 → Homepage 按钮 → 许可证行。
- **副标题 tagline**：i18n 静态文本，置于名称行正下方。
- 关于页签名 `view<'a>(fluent, theme, aria2_version)` 不变，app.rs 调用点（`src/app.rs:2862`）不变。
- iced 版本无运行时常量（`iced_core 0.14.0` 未导出 VERSION），硬编码 `"0.14"` 作为占位参数（与原实现 `"GUI: iced 0.14"` 一致）。

## Tasks

### 1. 新增打开链接消息
- `src/message.rs`：`Message` 枚举中 `CopyText(String),` 之后、`Noop,` 之前加 `OpenLink(String),`。
- `src/app.rs`：在 `update()` 的 `Message::CopyText(s) => return iced::clipboard::write::<Message>(s),` 之后、`Message::Noop => {}` 之前加：
  ```rust
  Message::OpenLink(url) => {
      return Task::perform(
          async move {
              let _ = open::that(&url);
          },
          |_| Message::Noop,
      );
  }
  ```
  （模式与 app.rs:2587-2592、3294-3299 的 `open::that` 一致；`open` crate 已引入且是 async-safe 用法。）

### 2. i18n：副标题 / iced 构建行 / Homepage
- `src/i18n.rs`：
  - `Tr` 枚举在 `LicenseNotice,` 后加三个变体：`AboutTagline, AboutBuiltWith, AboutHomepage`。
  - `key()` 中对应加：
    - `Tr::AboutTagline => "about-tagline"`
    - `Tr::AboutBuiltWith => "about-built-with"`
    - `Tr::AboutHomepage => "about-homepage"`
- `i18n/locales/en/main.ftl`（`license-notice` 行附近）：
  - `about-tagline = A fast, simple download manager`
  - `about-built-with = Built with iced { $version }`
  - `about-homepage = Homepage`
- `i18n/locales/zh-CN/main.ftl`：
  - `about-tagline = 快速、简洁的下载管理器`
  - `about-built-with = 基于 iced { $version } 构建`
  - `about-homepage = 项目主页`
- 占位符语法与现有 `last-sync-time = Last sync: { $time }` 一致（注意 `{ $version }` 内空格）。

### 3. 重写 `src/ui/about_dialog.rs`
- 新增导入：`use crate::ui::icon;`（`icon::globe()`，见 `src/ui/icon.rs:123`）。
- 新增常量：`const ABOUT_REPO_URL: &str = "https://github.com/yourname/remotrix";`
- 文本计算（保持现有格式，仅调整顺序）：
  - `gui_text = format!("Remotrix {}", env!("CARGO_PKG_VERSION"))`（第一个）
  - `engine_text`：同现状（`Engine: aria2-next v{v}` / `(--)`）（第二个）
  - `iced_text = fluent.get_args(Tr::AboutBuiltWith, &{ version: "0.14" })`，`get_args` API 参考 `src/ui/settings_page.rs:755-762`（`HashMap<Cow<str>, FluentValue>`，`"0.14".into()`）。
- body 列（保持 `column![].spacing(SPACE_4XL).align_x(Alignment::Center).width(Length::Fill)`）顺序：
  1. logo（现有 `container(logo::view(...)).center_x(Length::Fill).width(Length::Fill)`）。
  2. 名称行（Re primary / Motrix，`FONT_TITLE`，现状不变）。
  3. 副标题：`text(fluent.get(Tr::AboutTagline)).size(FONT_SMALL).style(theme::style::text::secondary)`。
  4. `copyable_text(gui_text.clone(), Message::CopyText(gui_text)).width(Length::Fill)`。
  5. `copyable_text(engine_text.clone(), Message::CopyText(engine_text)).width(Length::Fill)`。
  6. `text(iced_text).size(FONT_SMALL).style(theme::style::text::secondary)`。
  7. Homepage 按钮：
     ```rust
     let home_btn = button(
         row![icon::globe().size(FONT_MEDIUM), text(fluent.get(Tr::AboutHomepage)).size(FONT_BODY)]
             .spacing(SPACE_MD)
             .align_y(Alignment::Center),
     )
     .on_press(Message::OpenLink(ABOUT_REPO_URL.to_string()))
     .padding(PADDING_BUTTON_MD)
     .style(theme::style::button::secondary());
     ```
  8. 许可证行（现状不变：`text(fluent.get(Tr::LicenseNotice)).size(FONT_SMALL).style(theme::style::text::secondary)`）。
- footer `close_btn`、`Dialog::new().width(380.0).with_close(...).body(body).footer(close_btn).build()`、`overlay(...)` 全部保持不变。
- 注意：`.width(Length::Fill)` 时 `CopyableText` 的 `From` impl 会用 `truncated_text` + `max_lines(1)` 渲染 label（`copyable_text.rs:38-46`），无需改组件。

## Validation
- `cargo build` — 无警告。
- `cargo clippy --workspace` — 无警告。
- `cargo fmt --check` — 干净。
- 手动确认：应用版本行在引擎行之前且两行同宽填满、tagline 显示、`Built with iced 0.14` 显示、Homepage 按钮点击后系统浏览器打开占位 URL、许可证行与关闭按钮正常。

## Risks / Notes
- `Message::OpenLink` 与 `CopyText` 一样是全局枚举新增，注意两处（message.rs + app.rs）都改。
- 不要改 `copyable_text` 组件内部；只新增使用点 `.width(Length::Fill)`。
- 若以后确定真实仓库 URL，仅需改 `ABOUT_REPO_URL` 一处。
- iced 版本目前硬编码 "0.14"；未来如需动态化可换 `env!` 方式，不在本次范围。
