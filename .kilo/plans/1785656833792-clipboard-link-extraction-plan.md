# 剪切板链接提取与链接类型检测设置

## Goal
1. 剪切板检测从"整段内容必须是链接"改为**从任意文本中提取下载链接**。例：`这是链接 ftp://xxx 后面的文字 http://xxx` → 提取出两个链接，弹新建下载框并预填。
2. 只识别 6 类链接：`http/https`、`ftp`、`磁力 magnet`、`ed2k`、`迅雷 thunder`、`BT Info Hash`。
3. 高级设置页新增"剪切板"分组：主开关"自动检测剪切板中的下载链接"+（开启后显示）6 个链接类型 checkbox，**默认全勾选**，只识别勾选的类型。

## Confirmed decisions (已与用户确认)
1. 现有"剪切板"分组从**通用页整体移入高级设置页**，开关改名为"自动检测剪切板中的下载链接"（复用 `SettingKey::DetectClipboardOnStart` 与字段 `detect_clipboard_on_start`）。
2. 迅雷链接：`thunder://` 后的 base64 解码（去 `AA` 前缀、`ZZ` 后缀）得到真实 http/https/ftp URL 再填入下载框；解码失败回退原始 token。
3. BT Info Hash：裸 hash 自动转 `magnet:?xt=urn:btih:<hash>` 再填入。

## Detection rules (src/clipboard_watch.rs)
- `ClipboardLinkTypes` 结构（6 个 bool：`http`/`ftp`/`magnet`/`ed2k`/`thunder`/`bt_infohash`），serde 默认全 `true`，实现 `Default`。
- `parse_clipboard(text: &str, prefs: ClipboardLinkTypes) -> Option<ClipboardPayload>`：
  1. 先按现状判定整体文本是否为本地 `.torrent` 路径（`file://` 解码 + `.torrent` + `is_file`）→ 返回 `Torrent(PathBuf)`（保留现有能力）。
  2. 否则扫描文本提取链接（见下），非空 → `Urls(Vec<String>)`，否则 `None`。
- 提取算法（手写扫描，**不新增依赖**；`base64`/`hex` 已在依赖中）：
  - 依次查找 scheme 前缀出现位置，取 token 直到空白/引号/尖括号；再清除尾部标点（`.`,`，`,`。`,`;`,`；`,`:`,`：`,`!`,`！`,`?`,`？`,`、`,`)`,`）`,`]`,`】`,`>`,`"`,`'`,`,`）。
  - 各类型（仅在对应 prefs 开启时扫描）：
    - `http/https`：`http://`、`https://`
    - `ftp`：`ftp://`（可顺带 `ftps://`）
    - `magnet`：`magnet:?`
    - `ed2k`：`ed2k://`
    - `thunder`：`thunder://` + base64 解码去 `AA`/`ZZ`（用 `base64::engine::general_purpose::STANDARD`），失败保留原始 token
    - `bt_infohash`：词边界内 `[0-9a-fA-F]{40}`、`[2-7A-Za-z]{32}`，或 `btih:` + 上述；转为 `magnet:?xt=urn:btih:<hash>`；**排除被已提取 magnet token 覆盖的区间**（避免磁力内嵌 hash 重复）
  - 按出现位置排序、`HashSet` 去重、丢弃空串。

## Changes by file

### 1. `src/clipboard_watch.rs`
- 新增 `#[derive(Debug, Clone, Copy, Serialize, Deserialize)] pub struct ClipboardLinkTypes`（默认全 true）+ 提取逻辑 + 类型转换。
- `parse_clipboard` 增加 `prefs` 参数；`is_url` 逻辑被扫描器替代。
- 新增 `#[cfg(test)] mod tests`：中文混排双链接、magnet、ed2k、thunder 解码、infohash 转换、尾部标点清理、未勾选类型不提取、`.torrent` 路径仍识别。

### 2. `src/config.rs`
- `Settings` 新增 `#[serde(default)] pub clipboard_types: ClipboardLinkTypes`（type 定义在 clipboard_watch.rs，config import 它；clipboard_watch 不 import config，无循环）。
- `Settings::default()` 加 `clipboard_types: ClipboardLinkTypes::default()`。
- `apply_fields_equal` 增加 `detect_clipboard_on_start` 与 `clipboard_types` 比较（使 Apply/Reset/未保存确认对剪切板设置生效；`last_clipboard_hash` 不加）。

### 3. `src/message.rs`
- `SettingKey` 新增：`ClipboardHttp`、`ClipboardFtp`、`ClipboardMagnet`、`ClipboardEd2k`、`ClipboardThunder`、`ClipboardBtInfohash`。

### 4. `src/app.rs`
- `Message::ClipboardRead`：计算 hash 时并入类型开关签名（`sha256(format!("{trimmed}|{http}{ftp}{magnet}{ed2k}{thunder}{bt}"))`），并把 `state.settings.clipboard_types` 复制进 async 块传入 `parse_clipboard`。
- `Message::SettingChanged` 新增 6 个分支：`state.settings.clipboard_types.xxx = value == "true";`。
- 其余流程（`ClipboardParsed`、去重、Toast、弹框跳过）不变。

### 5. `src/i18n.rs` + FTL
- `Tr` 新增：`LinkTypeHttp`、`LinkTypeFtp`、`LinkTypeMagnet`、`LinkTypeEd2k`、`LinkTypeThunder`、`LinkTypeBtInfohash`，映射 key：`link-type-http`、`link-type-ftp`、`link-type-magnet`、`link-type-ed2k`、`link-type-thunder`、`link-type-bt-infohash`。
- `en/main.ftl`：
  - `detect-clipboard-on-start = Auto-detect download links from clipboard`
  - `link-type-http = HTTP/HTTPS links` / `link-type-ftp = FTP links` / `link-type-magnet = Magnet links` / `link-type-ed2k = ED2K links` / `link-type-thunder = Thunder links` / `link-type-bt-infohash = BT Info Hash`
- `zh-CN/main.ftl`：
  - `detect-clipboard-on-start = 自动检测剪切板中的下载链接`
  - `link-type-http = HTTP/HTTPS 链接` / `link-type-ftp = FTP 链接` / `link-type-magnet = 磁力链接` / `link-type-ed2k = ED2K 链接` / `link-type-thunder = 迅雷链接` / `link-type-bt-infohash = BT Info Hash`

### 6. `src/ui/settings_page.rs`
- `general_view`：删除 Clipboard 分组（原 208-213 行 `group_title(Tr::Clipboard)` + `labeled_toggle(DetectClipboardOnStart)`）。
- `advanced_view`：在 `update_toggle` 之后、`Performance` 分组之前插入：
  ```rust
  .push(group_title(fluent, Tr::Clipboard, accent))
  .push(labeled_toggle(fluent.get(Tr::DetectClipboardOnStart),
      settings.detect_clipboard_on_start, SettingKey::DetectClipboardOnStart))
  ```
  并在 `settings.detect_clipboard_on_start` 为 `true` 时追加 6 个 checkbox 行，每行：
  ```rust
  fn labeled_checkbox(label, value, key) // 仿 labeled_toggle，用 iced::widget::checkbox(value).on_toggle(|v| SettingChanged(key, v.to_string()))
  ```
  用到的 key：`ClipboardHttp/Ftp/Magnet/Ed2k/Thunder/BtInfohash`，标签取自 `Tr::LinkType*`。`checkbox` 加入 settings_page.rs 顶部 import（或全限定 `iced::widget::checkbox`）。

## Edge cases / failure modes
- 磁力链接内含 hash → infohash 扫描排除被 magnet token 覆盖区间，不重复。
- 40 位十六进制随机串被误判为 InfoHash（点击筛选固有风险，接受）。
- thunder base64 解码失败/非 UTF-8 → 保留原始 `thunder://...` token。
- 升级后 `last_clipboard_hash` 格式变化 → 首次聚焦多触发一次检测，可接受。
- 全部类型未勾选 → 扫描结果为空 → 不弹窗、不写 hash（与现状一致）。
- `.torrent` 路径检测保留，不与链接提取冲突（整体路径优先判定）。

## Validation
1. `cargo build`
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. `cargo test`（clipboard_watch 单元测试）
5. 手动（`cargo run --`）：
   - 复制 `这是链接 ftp://xxx 后面的文字 http://xxx` → 聚焦 → 弹框预填两个链接。
   - 复制 `magnet:?xt=urn:btih:<40hex>` → 预填磁力。
   - 复制 `thunder://QUFodHRwOi8vZXhhbXBsZS5jb20vZi56aXBaWg==` → 预填解码后的真实 URL。
   - 复制裸 `40hex` → 预填 `magnet:?xt=urn:btih:<hash>`。
   - 高级设置：关掉某类型 checkbox 后复制该类型链接 → 不弹；开关关闭 → 不读取。
   - 重启后设置保持。
