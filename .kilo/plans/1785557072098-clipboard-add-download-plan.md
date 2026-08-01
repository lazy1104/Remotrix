# 启动时从剪切板识别链接/种子并自动打开新建下载

## Goal
应用启动时读取一次系统剪切板：
- 若内容与上次启动时读取的内容哈希不同（判定为"新"），且内容是**下载链接**或**本地种子文件（.torrent）**，则自动打开"新建下载"对话框并预填。
- 功能受设置开关控制（默认开启）。
- 打开对话框时显示一条 Toast 提示来源。

## Design decisions (已与用户确认)
1. 添加设置开关 `detect_clipboard_on_start`（默认 `true`），位于"设置-常规"，新增 "Clipboard" 分组。
2. "新内容"判定：`settings.json` 中持久化 `last_clipboard_hash`（sha256+hex），本次与上次相同则不触发。
3. 多行内容：所有非空行均为链接则全部填入 URL 编辑框；否则仅取第一行作候选。
4. 自动打开时显示 Toast（zh/en 两条新翻译）。
5. 触发时机：`Message::WindowOpened` 首次回调（确保窗口已就绪）后执行 `iced::clipboard::read()`；用 `clipboard_checked: bool` 保证只执行一次。

## Detection rules (`src/clipboard_watch.rs` 新模块)
- 解析入口 `parse_clipboard(text: &str) -> Option<ClipboardPayload>`，返回：
  - `Urls(Vec<String>)` — 链接列表（每行一个，填入 URL 编辑框）
  - `Torrent(PathBuf)` — 本地 .torrent 文件路径（填入种子选择框）
- 链接判定（trim 后）：以 `http://`、`https://`、`ftp://`、`magnet:?`、`ed2k://`、`thunder://`、`flashget://`、`qqdl://` 开头，或包含 `://`。
- 种子判定（单行）：去掉 `file://localhost` / `file://` 前缀并做简单 `%XX` 百分号解码后，路径以 `.torrent` 结尾（大小写不敏感）且 `Path::is_file()` 存在；纯路径形式同样适用。
- 逻辑顺序：
  1. 去空白，拆非空行。
  2. 单行：先判种子，再判链接，否则 `None`。
  3. 多行：全部为链接 → `Urls(all)`；否则第一行是链接 → `Urls([first])`；否则 `None`。

## Changes by file

### 1. `Cargo.toml`
无新依赖（复用已有 `sha2`、`hex`；剪贴板用 iced 内置 `iced::clipboard::read`）。

### 2. `src/clipboard_watch.rs`（新文件）
- `pub enum ClipboardPayload { Urls(Vec<String>), Torrent(PathBuf) }`
- `pub fn parse_clipboard(text: &str) -> Option<ClipboardPayload>` 及私有 helper：
  - `is_url(line: &str) -> bool`
  - `torrent_path_from_line(line: &str) -> Option<PathBuf>`（file:// 解码 + `.torrent` + is_file）
  - 迷你百分号解码函数
- 无注释（遵循项目约定）；如需注释仅限非显然设计点。

### 3. `src/main.rs`
`mod clipboard_watch;` 加入模块声明。

### 4. `src/config.rs`
`Settings` 新增字段（均 `#[serde(default)]`，兼容旧配置）：
- `pub detect_clipboard_on_start: bool`，default 用现有 `default_true()`。
- `pub last_clipboard_hash: String`，`#[serde(default)]`。

### 5. `src/message.rs`
- `Message` 新增 `ClipboardRead(Option<String>)`。
- `SettingKey` 新增 `DetectClipboardOnStart`。

### 6. `src/app.rs`
- `Remotrix` 新增字段 `clipboard_checked: bool`（init 为 `false`）。
- `Message::WindowOpened(id)`：`window_id` 首次设置时，若 `!clipboard_checked` 则置 true 并
  `return iced::clipboard::read().map(Message::ClipboardRead);`
- 新增 `Message::ClipboardRead(content)` 处理：
  1. `!state.settings.detect_clipboard_on_start` → no-op。
  2. `content` 为 `None` → no-op（不更新哈希）。
  3. `hash = hex::encode(sha2::Sha256::digest(text.trim().as_bytes()))`；等于 `last_clipboard_hash` → no-op。
  4. 写回 `state.settings.last_clipboard_hash = hash` 并 `config::save`。
  5. `clipboard_watch::parse_clipboard(&text)` 为 `None` → no-op。
  6. 若 `state.add_dialog.is_visible()` → 跳过自动打开（避免覆盖用户正在编辑的对话框）。
  7. 打开并预填：
     - `Urls(urls)`：`add_dialog.open(download_dir, split)` 后 `add_dialog.set_urls(urls)`。
     - `Torrent(path)`：`add_dialog.open(download_dir, split)` 后 `torrent_picker.set_value(path.to_string_lossy())`。
  8. 用现有 `spawn_toast(ToastKind::Normal, fluent.get(Tr::ClipboardDetected), Some(3s), false)` 提示并返回其 task。
- `SettingKey::DetectClipboardOnStart` 分支：`state.settings.detect_clipboard_on_start = value == "true";`（仿照 `NavToTasksAfterAdd`）。

### 7. `src/ui/add_dialog.rs`
`AddDialogState` 新增方法：
```rust
pub fn set_urls(&mut self, urls: Vec<String>) {
    self.url_editor = text_editor::Content::with_text(&urls.join("\n"));
}
```

### 8. `src/ui/settings_page.rs`
常规页（General view，约 419-424 行 "Confirm" 分组之后）新增：
```rust
.push(iced::widget::Space::new().height(Length::Fixed(16.0)))
.push(group_title(fluent, Tr::Clipboard, accent))
.push(labeled_toggle(
    fluent.get(Tr::DetectClipboardOnStart),
    settings.detect_clipboard_on_start,
    SettingKey::DetectClipboardOnStart,
))
```

### 9. `src/i18n.rs` + FTL
- `Tr` 新增三个变体并映射 key：
  - `Tr::Clipboard` → `"clipboard"`
  - `Tr::DetectClipboardOnStart` → `"detect-clipboard-on-start"`
  - `Tr::ClipboardDetected` → `"clipboard-detected"`
- `i18n/locales/en/main.ftl`：
  - `clipboard = Clipboard`
  - `detect-clipboard-on-start = Detect download link from clipboard on start`
  - `clipboard-detected = Link detected from clipboard`
- `i18n/locales/zh-CN/main.ftl`：
  - `clipboard = 剪切板`
  - `detect-clipboard-on-start = 启动时检测剪切板中的下载链接`
  - `clipboard-detected = 已从剪切板识别到下载链接`

## Failure modes / edge cases
- 剪贴板读取返回 `None`（Wayland 权限、无剪贴板提供者）→ 静默跳过，不更新哈希，下次启动仍会重试。
- 关闭开关后完全不读取剪贴板。
- 内容不是链接也不是种子 → 仅更新哈希，不弹窗。
- 与上次相同的内容 → 哈希一致，不重复弹窗。
- 多行非全链接 → 只取第一行。
- `.torrent` URL（`http://…/x.torrent`）→ 按链接填入 URL 框（aria2 处理）。
- magnet / ed2k / thunder 等 → 按链接填入 URL 框。

## Validation
1. `cargo build`
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. 手动验证（`cargo run --`）：
   - 复制一个 `https://…` 链接后启动 → 自动弹新建下载，URL 框已填入，出现 Toast；再次启动（内容未变）→ 不弹。
   - 复制 `/path/to/x.torrent`（真实存在的文件）后启动 → 种子框已填入路径。
   - 关闭"启动时检测剪切板中的下载链接"开关后重启 → 不弹窗。
   - 剪切板为空文本 → 无任何弹窗。
