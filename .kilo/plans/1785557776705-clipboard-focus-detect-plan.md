# 聚焦时检测剪切板（替代仅启动时读取一次）

## Goal
将现有"仅在启动/首次打开窗口时读取一次剪切板"改为"**每次应用窗口获得焦点时**检测剪切板"。内容判定、对话框预填、Toast、哈希去重逻辑沿用现有实现。已与用户确认：剪贴板内容与上次相同（哈希一致）时不重复弹窗。

## Design decisions（已与用户确认）
1. 触发点：窗口聚焦事件（`iced::event::listen_with` 过滤 `window::Event::Focused`），同时保留首次 `WindowOpened` 触发作为兜底（部分平台初始聚焦事件不送达）。
2. 移除一次性 `clipboard_checked` 字段。
3. 去重：保留 `settings.json` 中的 `last_clipboard_hash`（sha256+hex）。内容不变 → 聚焦不弹窗；内容变化 → 弹窗。
4. 顺带修复哈希写入顺序 bug：对话框已打开导致跳过时**不写哈希**（避免新链接被永久标记为已处理）；非链接内容仍写入哈希（避免每次聚焦重复解析）。
5. 设置开关关闭时不发起剪切板读取（在触发点门控）。

## 技术验证（iced 0.14）
- `iced::event::listen_with(|event: Event, status, window: Id| -> Option<Message>)` 存在于 iced 0.14（`iced_futures::event`，经 `iced::event` 重导出）。
- `iced_core::window::Event::Focused` 为**无负载单元变体**；窗口 id 由 `listen_with` 闭包的第三个参数 `window: Id` 提供（注意：不是 `Focused(id)`）。
- `Subscription::filter_map` 亦存在，但用 `listen_with` 更简洁（过滤所有状态的事件）。

## Changes by file

### 1. `src/message.rs`
`Message` 新增变体（放在 `WindowOpened` 之后）：
```rust
WindowFocused(iced::window::Id),
```

### 2. `src/app.rs`

**a. 移除字段**：删除 `Remotrix.clipboard_checked: bool` 及其 init（`false`）。

**b. 新增私有辅助函数**（放在 `update` 附近）：
```rust
fn read_clipboard(state: &Remotrix) -> Task<Message> {
    if !state.settings.detect_clipboard_on_start {
        return Task::none();
    }
    iced::clipboard::read().map(Message::ClipboardRead)
}
```

**c. `Message::WindowOpened(id)`**（约 1049 行）：仅首次窗口保留兜底读取：
```rust
Message::WindowOpened(id) => {
    if state.window_id.is_none() {
        state.window_id = Some(id);
        return read_clipboard(state);
    }
    Task::none()
}
```

**d. 新增 `Message::WindowFocused(id)`**（放在 WindowOpened 之后）：
```rust
Message::WindowFocused(id) => {
    if state.window_id.is_none() || state.window_id == Some(id) {
        return read_clipboard(state);
    }
    Task::none()
}
```

**e. 调整 `Message::ClipboardRead(content)`**（约 1058-1103 行）处理顺序，**关键改动是哈希写入时机**：
```rust
Message::ClipboardRead(content) => {
    let Some(text) = content else {
        return Task::none();
    };
    let trimmed = text.trim();
    let hash = hex::encode(Sha256::digest(trimmed.as_bytes()));
    let Some(payload) = crate::clipboard_watch::parse_clipboard(trimmed) else {
        if hash != state.settings.last_clipboard_hash {
            state.settings.last_clipboard_hash = hash;
            config::save(&state.settings);
        }
        return Task::none();
    };
    if state.add_dialog.is_visible() {
        return Task::none();
    }
    if hash == state.settings.last_clipboard_hash {
        return Task::none();
    }
    state.settings.last_clipboard_hash = hash;
    config::save(&state.settings);
    match payload {
        ClipboardPayload::Urls(urls) => {
            state
                .add_dialog
                .open(state.settings.download_dir.clone(), state.settings.split);
            state.add_dialog.set_urls(urls);
        }
        ClipboardPayload::Torrent(path) => {
            state
                .add_dialog
                .open(state.settings.download_dir.clone(), state.settings.split);
            state
                .add_dialog
                .torrent_picker
                .set_value(path.to_string_lossy());
        }
    }
    let (_, task) = spawn_toast(
        state,
        ToastKind::Normal,
        state.fluent.get(Tr::ClipboardDetected),
        Some(Duration::from_secs(3)),
        false,
    );
    task
}
```
> 注意：原处理开头有 `if !state.settings.detect_clipboard_on_start { return Task::none(); }`，该门控上移到 `read_clipboard` 后此处不再需要（可删除，避免死逻辑）。`WindowOpened`/`WindowFocused` 返回的都是 `read_clipboard` 的结果。

**f. `subscription()`**（约 1647 行）新增聚焦订阅并加入 batch：
```rust
let focus = iced::event::listen_with(|event, _status, window| match event {
    iced::event::Event::Window(iced::window::Event::Focused) => {
        Some(Message::WindowFocused(window))
    }
    _ => None,
});
```
在 `Subscription::batch(vec![engine, open, close, ...])` 中追加 `focus`。闭包无捕获，满足 `listen_with` 的 `fn` 参数要求。

### 3. 无需改动
`config.rs`、`clipboard_watch.rs`、`add_dialog.rs`、`settings_page.rs`、`i18n.rs`、FTL 文件均无需改动（`detect_clipboard_on_start`、`last_clipboard_hash`、`parse_clipboard`、`set_urls`、Toast、翻译已就绪）。

## Edge cases / failure modes
- 启动时序：`WindowOpened` 兜底读取 + 首个聚焦事件可能同时派发两次 read；`update()` 顺序处理，哈希去重保证只弹一次。
- Wayland 等平台上初始剪切板读取返回 `None`（窗口尚未聚焦）→ 静默跳过、不写哈希；聚焦事件到达后重试成功。
- 反复聚焦同一内容 → 哈希一致，不弹窗（已确认）。
- 对话框打开时聚焦且剪贴板为**新**链接 → 跳过且**不写哈希**；关闭对话框后再聚焦仍能检测到该链接。
- 非链接内容 → 写入哈希、不弹窗（避免每次聚焦重复解析）。
- 快速连续聚焦 → 多次 read 派发，顺序处理 + 哈希去重，安全。
- 设置关闭 → `read_clipboard` 返回 `Task::none()`，完全不读取剪贴板。

## Validation
1. `cargo build`
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. 手动验证（`cargo run --`）：
   - 复制 `https://…` 链接 → 启动 → 弹新建下载并预填 + Toast。
   - 取消弹窗 → 复制另一链接 → 切换到其他窗口再切回（聚焦）→ 弹新链接。
   - 不复制新内容，反复切换窗口聚焦 → 不重复弹窗。
   - 关闭"启动时检测剪切板中的下载链接"开关 → 聚焦不弹窗。
   - 复制真实存在的 `.torrent` 路径 → 聚焦 → 种子框预填。
