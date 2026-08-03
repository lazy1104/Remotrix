# 同步追踪器下方描述对齐 + 自定义源卡片化 + 测试自定义源

## Goal
1. 放弃之前 BT 追踪器编辑器的只读化方案，不改动编辑器。
2. 调整 BitTorrent 设置中"同步追踪器"按钮下方的计数/上次同步描述，使其与按钮左对齐（同一列），前置 label 留空。
3. 对"已添加的自定义源"行做同样处理：前置 label 留空、与按钮同列，并加上圆角边框卡片样式，删除图标也包在边框内。
4. 交付一个测试用的自定义追踪器源 URL。

## Context
- 同步按钮行：`src/ui/settings_page.rs:743-761`，用 `setting_row_auto(Tr::BtTrackerSync, button...)` 渲染（label 固定 200px，按钮位于其后）。
- 按钮下方描述：`src/ui/settings_page.rs:762-767`，当前是直接 `tracker_rows.push(text(format!("{count_str} · {last_sync_str}")))`，从最左侧开始，未与按钮对齐。
- 已添加的自定义源列表：`src/ui/settings_page.rs:733-742`，当前用 `setting_row_auto(url.clone(), button(icon::x())...)` 渲染（URL 当 label，删除按钮当 control）。
- `setting_row_auto(label, control)`（`settings_page.rs:1371`）把 label 放进 200px 容器再排 control；传空字符串 label 即实现"留空 label + 与按钮同列"。先例：`logging_view` 里 `setting_row(String::new(), ...)`（`settings_page.rs:1337`）。
- 圆角卡片样式：`theme::style::card`（`src/ui/theme.rs:296`），`RADIUS_CARD=8`，含边框色 1px 边框 + weak 背景，`container(...).style(theme::style::card)` 即可用。
- 自定义源输入行（`settings_page.rs:715-732`）保持原样，不在本次改动范围。

## Changes（`src/ui/settings_page.rs`）
1. 同步描述（`762-767`）：包进 `setting_row_auto(String::new(), text(...))`，文本内容与样式保持不变（`format!("{count_str} · {last_sync_str}")`，`FONT_SMALL`，`theme::style::text::secondary`），使其与按钮左对齐、前置 label 空。
2. 已添加自定义源（`733-742`）：每项改为
   - `setting_row_auto(String::new(), <卡片>)`，与按钮同列、label 留空；
   - 卡片 = `container(row![ URL文本, Space(Fill), button(icon::x()) ])`，`.width(Length::Fill)`、合适的小内边距（如 4–6px，左右可略大），`.style(theme::style::card)`；
   - URL 文本：`text(url)`（`FONT_SMALL` 或 `FONT_MEDIUM`，可换行），删除按钮 `.on_press(Message::TrackerCustomRemove(url.clone()))` 保留在卡片内、右侧对齐。
3. 无其他源改动；BT 编辑器、预设源勾选框、自定义源逻辑、同步逻辑均保持不变。

## Deliverable: 测试自定义源
- 主推：`https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_best.txt`
  （ngosang 的 GitHub raw 镜像，区别于内置的 `ngosang.github.io` 与 `cf.trackerslist.com`，便于确认自定义源独立生效）
- 备选（jsDelivr CDN）：`https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection@master/best.txt`

## Validation
- `cargo build`
- `cargo clippy --workspace`
- `cargo fmt --check`
- 手动：设置 → BitTorrent → 确认同步描述与按钮左对齐、label 空；添加自定义源后每项以圆角卡片显示、删除图标在卡片内，且与按钮同列；粘贴上面的测试 URL 添加并同步，确认追踪器数量变化。

## Notes / Risks
- 卡片文本较长时应可换行，避免撑破布局；建议文本 `width(Length::Fill)` + `wrap()`，删除按钮固定右侧。
- 若删除按钮因 `theme::style::button::text()` 内边距过大影响卡片高度，可在实现时微调卡片 padding。
