# 修复 Torrent 上传区已选文件状态的样式与交互

## 背景 / 决策

- **拖放问题根因（已确认，用户决定不修）**：会话为 Wayland（`XDG_SESSION_TYPE=wayland`）。winit 0.30 的 Wayland 后端不实现 DnD（无 `HoveredFile`/`DroppedFile` 事件），因此 iced 收不到任何文件拖放事件。`app.rs:399-428` 的事件订阅与处理代码本身正确，在 X11/Windows/macOS 下可正常工作。
- **用户选择**：保持现状（不改后端），仅修复已选文件状态下的样式与交互。Wayland 下拖放仍不可用（与现状一致）。
- 范围：仅改 `src/ui/components/torrent_upload.rs` 一个文件，不动 `app.rs`/`message.rs`/i18n。

## 任务

### 1. 已选文件状态（`view()` 的 `else` 分支，约 213-263 行）重排

重写该分支为：

- **文件信息区（左，可点击重新选择）**：
  - 保留现有 `column![text(name), text(path)]`，`width(Length::Fill)`。
  - 用 `mouse_area(info).on_press(map(TorrentUploadEvent::Browse)).interaction(mouse::Interaction::Pointer)` 包裹，点击即触发 `Browse`（走现有 `app.rs` → `pick_path(PathPickerId::Torrent)` 流程）。`mouse::Interaction` 已 import。
  - **删除** 原来的 folder-open `replace_btn`（其功能被点击文件信息替代）。
- **移除按钮（右）**：保留现有 x 图标按钮（`TorrentUploadEvent::Clear`，tooltip `Tr::Remove`）。
- **垂直居中**：`row![reselect, remove_btn].spacing(SPACE_MD).align_y(Alignment::Center).padding(PADDING_CARD)`，并在 `container(...)` 上加 `.align_y(Alignment::Center)`（当前缺少该设置，是内容只占上部的直接原因；空状态分支 200-201 行已有相同用法）。
- **拖拽覆盖反馈**：已选状态容器样式从 `drop_zone(false)` 改为 `drop_zone(self.dragging)`，与空状态一致，拖入新文件时整个区域高亮。
- 保留外层 `mouse_area(stack![zone, dashed]...).on_enter(...).on_exit(...)`（hovered 边框变色）。

### 2. 不修改部分

- 空状态分支（187-212 行）保持不动。
- `is_torrent_file` / `is_valid_torrent_file` / `DashedBorder` / `update()` 等保持不动。
- 拖放覆盖逻辑（`app.rs` 的 `FileDropped` → `set_torrent_path`）无需改动。

## 验证

1. `cargo build`
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. 手动：`cargo run --` → 添加下载 → Torrent 页签
   - 通过浏览选择一个 `.torrent`：内容垂直居中，左侧为文件名+路径，右侧为 x 按钮。
   - 点击左侧文件信息 → 重新弹出文件选择。
   - 点击 x → 文件被移除，回到空状态提示。
   - （若在 X11 会话下测试）从文件管理器拖入新的 `.torrent` 覆盖 → 文件名更新，区域高亮。

## 备注（不实现）

- Wayland 下拖放不可用为 winit 限制；如需修复需强制 X11/XWayland 后端（用户已决定不采用）。
