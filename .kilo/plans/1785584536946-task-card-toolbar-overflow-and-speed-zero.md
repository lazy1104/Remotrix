# 修复任务卡片长文件名导致工具栏被挤出 + 下载中任务速度为 0 时显示 0

## Goal
1. **工具栏被挤出屏幕**：任务名很长（典型为种子下载任务，如
   `[DMG&SumiSora][Magical_Girl_Lyrical_Nanoha_EXCEEDS_Gun_Blaze_Vengeance][04][1080P][BIG5].mp4`）
   时，卡片第一行 `row![name, Space(Fill), toolbar]` 中 `name` 按自然宽度撑满整行，
   把右侧 5 个操作按钮（暂停/开始、文件夹、复制链接、详情、删除）推到卡片/滚动区可视边界之外被裁掉，
   表现为"缺少工具栏"（已与用户确认根因）。
2. **下载中任务速度为 0 不显示**：卡片速度文本当前为 `if t.speed > 0 { format_speed } else { "—" }`
   （`src/ui/task_list.rs:340-344`）。下载中（Active/Waiting）任务瞬时速度为 0（刚启动、连不上 peer、
   短暂卡顿）时显示 `—`，用户要求显示 `0 B/s`（与速度 HUD 行为一致：`format_speed(0)` = `"0 B/s"`）。

## 根因
- iced `Text` 默认 `Length::Shrink` + `Wrapping::Word`：在 Row 中按内容自然宽度布局，超宽不换行，
  整行被外层 `slim_scrollable` 视口裁切，工具栏整体消失。
- 种子文件名通常无空格，`Wrapping::Word` 无法换行，必须用 `Wrapping::WordOrGlyph`（无空格时按字形换行）。
- `name` 若改为 `Length::Fill`，必须删除行内的 `Space(Fill)`，否则两个 Fill 子项均分剩余宽度，
  会在短文件名时把名称与工具栏之间拉开一条大空隙。

## Changes by file

### 1. `src/ui/task_list.rs`

**Issue 1 — 名称换行，保住工具栏**
- `task_card` 中（约 L222）：
  ```rust
  let name = text(t.name.clone())
      .size(15)
      .width(Length::Fill)
      .wrapping(text::Wrapping::WordOrGlyph);
  ```
- 卡片第一行（约 L376-382）删除 `iced::widget::Space::new().width(Length::Fill)`：
  ```rust
  row![name, toolbar].align_y(Alignment::Center)
  ```
  `name` 作为唯一 Fill 子项占据工具栏左侧全部空间；文本左对齐（`text::Alignment::Default` 对 LTR 即左对齐），
  短文件名视觉不变，长文件名在框内按字形换行、多行撑高卡片，工具栏始终固定在右侧可见。
- 不需要新增 import：`text` 模块已导入，`text::Wrapping::WordOrGlyph` 可直接引用。

**Issue 2 — 下载中速度为 0 显示 "0 B/s"**
- 约 L340-344，将
  ```rust
  let speed_text = if t.speed > 0 { format_speed(t.speed) } else { "—".to_string() };
  ```
  改为
  ```rust
  let speed_text = if t.is_download_active() || t.speed > 0 {
      format_speed(t.speed)
  } else {
      "—".to_string()
  };
  ```
  效果：
  - Active / Waiting 且 speed==0 → `format_speed(0)` = `"0 B/s"`；
  - 任意状态 speed>0 → `format_speed(speed)`（保留现状）；
  - Paused / Completed / Error / Removed 且 speed==0 → `—`（不再下载中，不显示速度）。
  `is_download_active()`（Active|Waiting）与同函数 `conn_text`（L349）口径一致；
  "下载中"分类（`TaskFilter::Downloading`）也包含 Waiting。

### 2. `src/ui/details_dialog.rs`（一致性与健壮性，次要）
- **Activity 页速度**（约 L234-238）：同样改为
  ```rust
  let speed_str = if task.is_download_active() || task.speed > 0 {
      format_speed(task.speed)
  } else {
      "—".to_string()
  };
  ```
- **Summary 页 `key_value_row`**（约 L136-149）：value 文本加
  `.width(Length::Fill).wrapping(text::Wrapping::WordOrGlyph)`，避免长文件名在 640px 详情对话框里横向溢出。
  （`text` 已在文件头导入。）

## Edge cases / failure modes
- 短文件名：Fill 框左对齐单行，视觉与现状一致；无回归。
- 无空格长文件名（种子）：`WordOrGlyph` 按字形换行，工具栏可见。
- 极窄窗口：名称更多行换行，工具栏始终可见。
- 多行名称使卡片增高：任务列表本来纵向可滚动，可接受。
- 已在 aria2 中任务（名称由 `name_from_status` 提供）同样受益；修复不依赖任务来源。
- 不触碰：页面顶部工具栏、速度 HUD（`speed_hud.rs` 已在前一提交支持 active 时显示 `0 B/s`）、
  详情对话框 Files/Activity 其余内容。
- 已知但本次不处理：日志中 `Unrecognized URI or unsupported protocol: <本地 .torrent 路径>`（
  会话恢复时把本地种子路径当 URI）与 magnet 重加任务名显示 `magnet:` 属种子流程其它问题，超出本次范围。

## Validation
1. `cargo build`
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. 手动：
   - 文件选择器添加一个长文件名种子 → 卡片右上角 5 个按钮完整可见；短文件名 URL 任务卡片无视觉变化。
   - 新建下载任务并暂停到速度为 0 的瞬时（或直接观察刚启动的任务）→ 速度显示 `0 B/s` 而非 `—`；
     暂停/完成后恢复 `—`。
   - 打开种子任务详情 → 概览页长文件名换行不溢出，活动页速度口径与卡片一致。
