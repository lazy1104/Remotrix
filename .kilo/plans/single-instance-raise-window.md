# 单实例 + 启动拉起已运行实例窗口（含 CLI 参数转发）

## 目标
- 若已有 remotrix 实例在运行，第二次启动时：把已运行实例的窗口拉到前台（取消最小化 + 聚焦），并把命令行参数（链接 / torrent 文件路径）转发给已运行实例，自身立即退出，不再启动新的 aria2 sidecar。
- 已运行实例收到转发参数后：把它们预填进 AddDialog（URL 进多行编辑器、torrent 文件进 torrent 路径），并拉起窗口，等用户确认后再开始下载。
- 第二次启动若没有带任何参数：仅拉起窗口，不预填 AddDialog。

## 范围与非目标
- 平台：Linux（开发/运行环境为 Linux）。使用 Unix 域套接字。
- 非 Unix 平台（Windows）暂以 `cfg(unix)` 桩实现：`acquire()` 返回 Primary 但不强制单实例，可正常编译并后续补 named pipe。
- 不引入 clap/任何 CLI 解析框架；`std::env::args_os()` 原样转发，已运行实例自行分类。
- 不实现系统托盘；窗口“拉起”指 `iced::window::unminimize` + `iced::window::gain_focus`（窗口当前不会被隐藏，仅可能被最小化或置于后台）。

## 决策
1. **IPC 通道**：Unix 域套接字，路径 = `<data_dir>/remotrix-ipc.sock`（`config` 已有 `directories::ProjectDirs`），与 `db_path()` 同一 `data_dir`。
2. **时序**：在 `main()` 调用 `iced::application(...).run()` **之前** 调用 `single_instance::acquire()`，确保第二实例在启动 engine / aria2 / DB / 日志写线程之前就退出。
3. **不依赖 tokio 运行时**做探测/上报：监听端用 `std::thread` + `std::os::unix::net::UnixListener`（阻塞），二实例端用 `std::os::unix::net::UnixStream`（阻塞）。这样 `acquire()` 可在 `run()` 之前同步完成。
4. **向 GUI 注入**：复用现有 `event_rx_slot: Arc<Mutex<Option<EventRx>>>` + `Subscription::run_with` 模式。新增 `restore_rx_slot: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Vec<String>>>>`，由监听线程持有 `UnboundedSender`（`UnboundedSender::send` 不需运行时上下文，可在 std 线程调用）。
5. **跨 `init()` 传递 receiver**：`init()` 签名不可改。用模块级 `static OVERLAY: std::sync::OnceLock<Option<UnboundedReceiver<Vec<String>>>>`，`main()` 在 Primary 分支把 receiver `set` 进去，`init()` `take` 出来。
6.转发协议：二实例向连接写入 `serde_json` 数组 `["arg1","arg2"]` 后 `shutdown` 写端；监听线程读全部字节反序列化为 `Vec<String>`。空 `[]` 表示仅拉起窗口。
7. 窗口拉起来自订阅流产出的 `Message::ForwardedArgs(Vec<String>)`，在 `update` 中处理，无需新增 `WindowCmd`。

## 改动清单

### 1. 新建 `src/single_instance.rs`
- `pub enum Outcome { Primary { restore_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<String>> }, Secondary }`
- `pub fn acquire() -> Outcome`：
  - 解析 socket 路径：`crate::config::data_dir()`（新增公共封装，复用 `directories::ProjectDirs::from("dev","remotrix","Remotrix").data_dir()`，参考现有 `db_path()`）；路径缺失时回退：返回 Primary 但不监听（receiver 永不产出，单实例弱化为“不禁用”）。
  - 路径存在 → 先 `UnixStream::connect`；成功则需读取对端确认（写 `[]`、读一行 ack 后判定）—— 简化：connect 成功即判定 Secondary，写入 JSON 数组后 `process::exit(0)`。
  - connect 失败（连接被拒 / 路径不可用）→ 视为 stale：`fs::remove_file` 清理后 `UnixListener::bind(path)`。
  - 绑定成功 → Primary：`tokio::sync::mpsc::unbounded_channel()`；`std::thread::spawn` 运行 `run_listener(listener, sender)`，返回 `Outcome::Primary { restore_rx }`。
- `fn run_listener(listener: UnixListener, tx: UnboundedSender<Vec<String>>)`：
  - `loop { match listener.accept() }`，对每个 stream：读至 EOF（`read_to_end` 上限例如 1 MiB 防滥用），反序列化 `Vec<String>`；`tx.send(args).ok()` 并向 socket 写一字节 ack 再关闭。反序列化失败时按 `vec![]` 发送（仅拉起）。
  - 监听线程 panic 不影响主流程：用 `tracing::error!` 记录后线程结束，单实例功能降级（后续启动可能重复），可接受。
- 非 unix：`pub enum Outcome` + `pub fn acquire()` 直接返回无意义的 Primary（带一个永不产出的 receiver），声明单实例未启用。

### 2. `src/config.rs`
- 抽出 `pub fn data_dir() -> Option<PathBuf>`（当前 `db_path`/`aria2`/`log_dir` 各自重复这一段），后续函数复用它；`single_instance` 用同一函数。保持原有函数行为不变。

### 3. `src/main.rs`
- `fn main()` 开头（在 `init_tracing()` **之前**，避免二实例创建日志文件）：
  ```rust
  match crate::single_instance::acquire() {
      crate::single_instance::Outcome::Primary { restore_rx } => {
          let _ = RESTORE_RX.set(restore_rx);
      }
      crate::single_instance::Outcome::Secondary => {
          std::process::exit(0);
      }
  }
  ```
- 模块声明 `mod single_instance;`；新增 `static RESTORE_RX: std::sync::OnceLock<tokio::sync::mpsc::UnboundedReceiver<Vec<String>>> = std::sync::OnceLock::new();`（在 `main.rs`，供 `app::init` 通过 `crate::main` 或公开 getter 读取）。为干净起见，把 `OnceLock` 放进 `single_instance` 模块并暴露 `pub fn take_restore_rx() -> Option<UnboundedReceiver<Vec<String>>>`。二实例不执行到后续逻辑。

### 4. `src/app.rs`
- `Remotrix` 新增字段：`restore_rx_slot: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Vec<String>>>>`。
- `init()` 中：`let restore_rx = crate::single_instance::take_restore_rx();` 存入 `restore_rx_slot`。
- 新增 `RestoreSlot(Arc<Mutex<Option<...>>>)`，仿 `EventSlot` 实现 `Hash/PartialEq/Eq/Clone`。
- 新增 `fn build_restore_stream(slot: &RestoreSlot) -> impl Stream<Item = Message>`：`iced::stream::channel(8, move |sender| async move { if let Some(mut rx) = rx { while let Some(args) = rx.recv().await { let _ = sender.send(Message::ForwardedArgs(args)).await; } } })`（复用 `slot.0.lock().take()` 取出）。
- `subscription()`：把 `Subscription::run_with(RestoreSlot(...), build_restore_stream)` 加入 `batch`。
- `update()` 增加 `Message::ForwardedArgs(args)` 分支：
  - 先执行窗口拉起（见下）。
  - 分类 `args`（遍历 `args_os` 已转 String）：URL 白名单前缀 `http:// https:// ftp:// magnet:`（不区分大小写）→ 收集进 `urls`；其余若 `Path::new(a).is_file()` 视为 torrent 文件 → 取首个进 `torrent_path`；其余忽略。
  - 若 `urls` 与 `torrent_path` 均为空：仅拉起窗口，不打开对话框。
  - 否则：`state.add_dialog.open_prefill(urls.join("\n"), torrent_path, state.settings.download_dir.clone(), state.settings.split)`（新增方法）。关闭 history 弹层。
  - 返回 `Task::none()`。
- 窗口拉起 Task：如已有 `state.window_id`，组合 `Task::batch([iced::window::unminimize::<Message>(id), iced::window::gain_focus::<Message>(id)])`；否则忽略待 `WindowOpened` 后由后续转发再处理（罕见：第二实例在主实例窗口尚未 open 完成前转发——可接受，丢弃本次拉起，参数已落 AddDialog）。

### 5. `src/ui/add_dialog.rs`
- 新增 `pub fn open_prefill(&mut self, urls_text: String, torrent_path: Option<PathBuf>, default_dir: PathBuf, default_split: u16)`：
  - `self.visible = true;`
  - `self.url_editor = text_editor::Content::with_text(&urls_text);`
  - `self.save_picker.set_value(default_dir.to_string_lossy());`
  - `self.split = default_split;`
  - `self.torrent_picker.set_value(torrent_path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default());`
  - 不调用 `close_history()`（由 `app::update` 在外层统一关闭）。
  - 复用提示：`can_submit()` 已支持 `torrent_picker`/`url_editor` 非空判断，转发预填后“开始下载”按钮即可用。

### 6. `src/message.rs`
- 在 `Message` 加 `ForwardedArgs(Vec<String>)`。

### 7. `Cargo.toml`
- 无需新增依赖：`serde_json` 已在；Unix 套接字走 std；tokio `full` 已启用。

## 风险 / 边界
- **savor ID 绑定于 `Arc` 指针**：`RestoreSlot` 与 `EventSlot` 实现一致，`Subscription::run_with` 用其区分，互不冲突。每个订阅一条。
- **stale socket**：旧实例崩溃留下 socket 文件 → `connect` 失败 → `remove_file` → 重新 bind。Windows 下无 Unix socket，回退不强制单实例。
- **窗口尚未打开即收到转发**：`window_id` 为 `None` 时拉起 Task 被跳过；AddDialog 已预填，用户可手动点开（主实例窗口正打开中，通常很快出现）。属可接受的极小窗口竞态。
- **超大/恶意 payload**：读上限 1 MiB，超出截断并以 `vec![]` 处理；本地 IPC，威胁面有限。
- **aria2 sidecar 不会因第二实例重复启动**：第二实例在 `acquire()` 阶段 `exit(0)`，`spawn_engine` 不会执行。
- **多 torrent 一并转发**：仅填首个 torrent 路径；其余 torrent 文件当前对话框不支持批量。记录为已知限制，后续可扩展为批量任务。
- **既有日志/DB 文件**：第二实例在 `init_tracing` 前 exit，不创建日志/不打开 DB，避免竞争。

## 验证步骤
1. `cargo fmt --check`
2. `cargo clippy --workspace`（无警告）
3. `cargo build`（离线）
4. 手动：
   - 启动实例 A（`cargo run --`）。再启动实例 B：A 窗口被前置；B 立即退出（进程列表只剩 A）。
   - `cargo run -- https://example.com/file.zip`：A 窗口前置且 AddDialog 打开、URL 已填入、保存目录为默认、连接数=默认 split，点“开始下载”创建任务。
   - `cargo run -- /path/to/test.torrent`：A 窗口前置且 AddDialog 的 torrent 路径已填。
   - `cargo run -- https://a.com/1 https://b.com/2`：URL 以两行填入。
   - 杀掉 A（`kill`）后残留 socket：再启动新实例 → 能成为 Primary 并正常显示窗口（stale 清理生效）。
   - 第二实例：确认 `ps` 中没有第二个 aria2-next 子进程。