# 剪切板下载：文件判断 + 文本大小门槛

## 目标

增强剪切板自动检测（`src/clipboard_watch.rs` + `app.rs` 的 `Message::ClipboardRead`）：

1. **种子文件**（保持现有路径识别）→ 进入种子任务标签页。
2. **剪切板是单行、指向本地非种子文件**，且文件 ≤ 64KB → 读取文件内容（UTF-8），从中提取下载链接 → 打开 URL 标签页。
3. **文本大小门槛**：剪切板文本 > 64KB → 直接忽略，不弹窗。
4. 非种子本地文件若：> 64KB、是目录、非 UTF-8、或内容无链接 → 忽略。

## 已确认决策

- 种子识别仅限文件路径（`file://`/本地路径），**不**读取剪切板二进制内容（无新依赖，不引入 arboard/clipboard-rs）。
- 阈值固定 64KB 常量，不做设置项（无 config / i18n / 设置页改动）。
- 超过阈值的文本或文件一律忽略，静默不弹窗。

## 改动

### 1. `src/clipboard_watch.rs`

- 新增常量：`pub const MAX_CLIPBOARD_CONTENT: u64 = 64 * 1024;`
- 新增 `pub fn payload_hash(payload: &Option<ClipboardPayload>) -> String`：
  - `Urls(urls)` → `hex(Sha256("urls|<join \n>"))`
  - `Torrent(p)` → `hex(Sha256("torrent|<path>"))`
  - `None` → 空字符串
  - 新增 `use sha2::{Digest, Sha256};`（`hex` 已作为依赖用于 `percent_decode`）。
- `parse_clipboard(text, prefs)` 改造：
  - 入口门控：`if text.len() as u64 > MAX_CLIPBOARD_CONTENT { return None; }`
  - 将 `torrent_path_from_line` 重构为 `file_path_from_line(line) -> Option<PathBuf>`：解析 `file://localhost` / `file://` / 裸路径 + `percent_decode`，`path.is_file()` 才返回 Some。
  - 单行分支：
    - `file_path_from_line` 返回 `Some(path)`：
      - `is_torrent_path(&path)`（扩展名忽略大小写 == "torrent"，沿用现有逻辑）→ `Some(Torrent(path))`（不变）
      - 否则 → `file_content_links(&path, prefs)` 的结果直接返回（不回落解析路径文本）
    - 否则 → 与现在一致，对整段文本 `extract_links(text, prefs)`。
- 新增私有 helper：
  - `fn is_torrent_path(path: &Path) -> bool`
  - `fn file_content_links(path: &Path, prefs: ClipboardLinkTypes) -> Option<ClipboardPayload>`：
    - `fs::metadata` 失败或 `len == 0 || len > MAX_CLIPBOARD_CONTENT` → None
    - `fs::read` 失败 → None
    - `String::from_utf8` 失败（二进制）→ None
    - `extract_links(&content, prefs)` 为空 → None，否则 `Urls(urls)`
  - 需要 `use std::path::{Path, PathBuf};`

### 2. `src/app.rs` — `Message::ClipboardRead`（约 1627–1652 行）

- 删除现用的"解析前基于 `trimmed` + prefs 计算 hash"的代码。
- `Task::perform` 闭包改为：
  ```rust
  let payload = crate::clipboard_watch::parse_clipboard(&trimmed, prefs);
  let hash = crate::clipboard_watch::payload_hash(&payload);
  (payload, hash)
  ```
- 后续 `ClipboardParsed`（去重、写 `last_clipboard_hash`、`config::save`、`open_with`、Toast）不变。

### 3. 单元测试（`src/clipboard_watch.rs` 的 `#[cfg(test)]`）

在临时目录写入文件并断言：
- 大文本：>64KB 且含链接 → `None`
- 小 `.txt` 文件（裸路径）内容含链接 → `Urls`
- 小 `.txt` 文件（`file://` URI）内容含链接 → `Urls`
- 大文件（>64KB 且含链接）→ `None`
- 小二进制文件（非 UTF-8）→ `None`
- 现有 `torrent_path_still_recognized` 等测试保持不变且通过

## 行为说明 / 边界情况

- 去重 hash 改为基于**解析结果**（payload），不再基于原文+prefs；相同提取结果在再次聚焦时不会重复弹窗（符合预期）。链接类型开关的变动若不影响提取结果，将不再使去重失效（可接受的行为变化）。
- 文件内容读取（≤64KB）发生在 `Task::perform` 内（主线程之外），无卡顿。
- 单行裸路径若指向不存在的文件 → 回落为文本链接提取（原行为不变）。
- 单行是目录路径 → 回落为文本链接提取（通常无链接 → None）。
- 无需修改 i18n、config、设置页、README（README 未提及剪切板功能）。

## 验证

- `cargo test`（clipboard_watch 单元测试）
- `cargo build`（离线，无新依赖）
- `cargo clippy --workspace`（无警告）
- `cargo fmt --check`
- 手动：复制含 URL 的 `.txt` 文件 → URL 标签页弹框；复制大文件 → 无反应；复制 `.torrent` 文件 → 种子标签页；复制超长文本 → 无反应。
