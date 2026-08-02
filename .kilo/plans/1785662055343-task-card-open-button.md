# 任务卡片"打开"按钮 + 双击任务名

## Goal
在任务列表的任务卡片工具栏首位（`pause/resume` 之前）新增一个 **打开** 图标按钮；双击任务名称也触发同一动作。行为：
- 普通任务（HTTP/FTP）：用系统默认应用打开已下载文件（`save_dir.join(name)`）。文件不存在 → 弹出警告 toast（已确认）。
- BT/磁链任务（`info_hash` 非空，或 URL 是 magnet / `.torrent`）：弹出 **新建下载** 对话框并预填磁力链接（已确认）。url 为空但有 `info_hash` 时构造 `magnet:?xt=urn:btih:<hash>`。

## Files & Changes

### 1. `src/message.rs`
- 在 `OpenTaskFolder(String)`（line 142）旁新增：
  ```rust
  OpenTaskFile(String),
  ```

### 2. `src/i18n.rs`
- `Tr` 枚举新增 `Open`、`FileMissing`。
- `key()` 新增：`Tr::Open => "open"`、`Tr::FileMissing => "file-missing"`。

### 3. `i18n/locales/en/main.ftl` + `i18n/locales/zh-CN/main.ftl`
新增（两文件同 key，分别中英文）：
- `open = Open` / `打开`
- `file-missing = The downloaded file is missing` / `下载文件不存在`

### 4. `fonts/icons.toml`
- 新增 `external_link = "external-link"`（构建时 `iced_lucide::build` 会自动重新生成 `src/ui/icon.rs` 与 `fonts/lucide.ttf`；不要手改生成文件）。
- 生成后 `src/ui/icon.rs` 会出现 `pub fn external_link<'a>() -> Text<'a>`。

### 5. `src/ui/task_list.rs` — 工具栏按钮 + 双击
- 引入 `use iced::widget::mouse_area;`、`use iced::mouse;`。
- 在 `task_card` 中新增 `open_btn`（放在 `pause_resume_btn` **之前**）：
  ```rust
  let open_btn: Element<'a, Message> = {
      let glyph = icon::external_link().size(FONT_ICON).color(text_secondary);
      tip::standard(
          button(glyph)
              .on_press(Message::OpenTaskFile(t.gid.clone()))
              .padding(PADDING_ICON_BTN)
              .style(theme::style::button::toolbar_icon(false)),
          text(fluent.get(Tr::Open)).size(FONT_SMALL),
          tooltip::Position::Bottom,
      )
  };
  ```
- 工具栏 `row![]`（line 333）第一个 `.push(open_btn)`，再接现有 `pause_resume_btn` 等。
- 任务名称双击：把 `name`（line 235-242 的 `tip::standard(truncated_text(..), text(..), Bottom)`）用 `mouse_area` 包裹：
  ```rust
  let name = mouse_area(
      tip::standard(
          truncated_text(t.name.clone())
              .size(FONT_ICON)
              .max_lines(2)
              .wrapping(text::Wrapping::Glyph),
          text(t.name.clone()).size(FONT_SMALL),
          tooltip::Position::Bottom,
      ),
  )
  .on_double_click(Message::OpenTaskFile(t.gid.clone()))
  .interaction(mouse::Interaction::Pointer);
  ```
  注意：`name` 在 `row![name, toolbar]` 中保持 `Length::Fill` 展开（truncated_text 默认宽度 Fill），双击区域即任务名文本区域。

### 6. `src/app.rs` — 消息处理
在 `Message::OpenTaskFolder`（line 1721）之前新增分支：
```rust
Message::OpenTaskFile(gid) => {
    let Some(t) = state.tasks.get(&gid).cloned() else {
        return Task::none();
    };
    let is_bt = t.info_hash.is_some()
        || crate::engine::is_magnet_url(&t.url)
        || crate::engine::is_torrent_url(&t.url);
    if is_bt {
        state.add_dialog.save_picker.close_history();
        state
            .add_dialog
            .open(state.settings.download_dir.clone(), state.settings.split);
        let link = if !t.url.is_empty() {
            t.url.clone()
        } else if let Some(hash) = t.info_hash.as_deref() {
            format!("magnet:?xt=urn:btih:{hash}")
        } else {
            String::new()
        };
        if !link.is_empty() {
            state.add_dialog.set_urls(vec![link]);
        }
        return Task::none();
    }
    let path = t.save_dir.join(&t.name);
    if path.exists() {
        return Task::perform(
            async move { let _ = open::that(&path); },
            |_| Message::Noop,
        );
    }
    let (_, task) = spawn_toast(
        state,
        ToastKind::Warning,
        state.fluent.get(Tr::FileMissing),
        Some(Duration::from_secs(4)),
        false,
    );
    return task;
}
```
- 复用已有的 `crate::engine::is_magnet_url` / `is_torrent_url`（engine.rs:369-377）与 `open::that`（`OpenTaskFolder` 同款用法）。
- `spawn_toast` 返回 `(u64, Task)`，丢弃 id 用 task 即可。

## Behavior / Edge Cases
- **任务已移除 / 不存在**：`state.tasks.get` 失败 → 静默 `Task::none()`。
- **普通任务文件缺失**（未完成/出错/删除）：警告 toast `file-missing`，不打开。
- **magnet 任务**：`info_hash` 解析前 url 即为 magnet 链接 → 预填原链接；`bt-metadata-only` 场景下 url 为空但有 `info_hash` → 构造 magnet。
- **`.torrent` URL 任务**：按 BT 处理，弹新建下载框预填 URL（可重新添加）。
- **双击与单击不冲突**：任务名当前无单击动作，`on_double_click` 仅双击触发。

## Validation
1. `cargo build`（触发 icon 重新生成，`src/ui/icon.rs` 含 `external_link`）
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. 手动验证：
   - 已完成 HTTP 任务点"打开" → 系统应用打开文件；双击任务名同样触发。
   - 未完成/出错任务点"打开" → 警告 toast。
   - magnet/BT 任务点"打开" → 新建下载对话框预填 magnet 链接。
   - 工具栏顺序：打开、暂停/继续、文件夹、复制链接、详情、删除。
