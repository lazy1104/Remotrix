# Remotrix

> **[中文](README.md) | [English](README_EN.md)**

基于 Rust 的原生桌面下载管理器，灵感来自 [Motrix-next](https://motrix-next.pages.dev/)，采用 [`iced`](https://github.com/iced-rs/iced) GUI 框架，并通过 WebSocket JSON-RPC 驱动 [`aria2-next`](https://github.com/AnInsomniacy/aria2-next) 侧车引擎（[`aria2-ws`](https://crates.io/crates/aria2-ws)）。

> 名称 "Remotrix" 是 **Rust** 与 **Motrix** 的合成词。

## 为什么做 Remotrix？

Remotrix 最初是一个学习项目。我喜欢 Motrix / Motrix-next 的设计，但基于 Tauri 的 Motrix-next 在我的 Windows 10 机器上无法正常运行，在 Linux 机器上又有严重的性能问题。我想学习 Rust 和 `iced` GUI 框架，于是决定从零开始做一个原生的 Rust 下载管理器，以 Motrix-next 作为设计参考。本项目由 AI 辅助开发。

## 功能特性

- **原生 Rust UI** —— 通过 `iced 0.14` 纯 Rust 渲染，无 Electron / 浏览器内核
- **多协议下载** —— 支持 HTTP/HTTPS/FTP、BitTorrent（`.torrent` 文件）与 Magnet 磁力链接，由 aria2-next 驱动
- **并行分段** —— 可配置每台服务器的分段数 / 最大连接数
- **全局与任务级限速** —— 下载 / 上传限速独立设置，持久化到磁盘
- **内嵌持久化** —— 任务元数据与进度存储在本地 SQLite 数据库，重启后保留
- **自管理引擎** —— aria2-next 在运行时从 GitHub Releases 自动获取（sha256 校验、缓存、自愈），支持自动更新检查与后台暂存更新，下次重启时应用
- **无边框窗口** —— 自定义标题栏，含最小化 / 最大化 / 关闭按钮与关闭确认对话框
- **系统托盘** —— 托盘图标与菜单，支持最小化到托盘 / 关闭到托盘（`ldtray`）。注意：Wayland 下集成不完整，窗口只能最小化到任务栏，无法完全隐藏
- **系统通知** —— 下载完成等事件的原生桌面通知（`notify-rust`）
- **单实例运行** —— 二次启动时聚焦已有窗口（`app-single-instance`）
- **主题系统** —— 选择强调色（一排色块）；iced 自动生成完整的浅色 / 深色调色板，包括由强调色派生出的 M3 风格表面背景；应用可跟随系统外观（`dark-light` 检测）
- **国际化** —— 自动从系统区域设置检测 `zh_CN` / `en_US`，可在设置中切换
- **任务详情** —— 摘要 / 活动 / 文件三个标签页，含 BitTorrent 分片完成度图
- **排序与筛选** —— 按添加时间、名称、大小、进度或状态排序；按全部 / 下载中 / 已完成筛选
- **剪贴板监听** —— 自动检测复制到剪贴板的 http/ftp/magnet/ed2k/bt 链接
- **浏览器接管** —— 本地扩展 API（Salvo HTTP 服务，默认 `127.0.0.1:29110`），配合上游 [motrix-next-extension](https://github.com/AnInsomniacy/motrix-next-extension)（MIT，零改动复用）在浏览器中一键接管下载
- **文件日志** —— 数据目录下按天滚动的日志文件

## 截图

截图待补充——应用图标见 `assets/icon.png`。

## 架构

Remotrix 采用**双事件循环**设计：

- **iced UI 循环**运行在主线程
- **tokio 运行时**在后台线程驱动一个引擎**监管者**，负责派生 `aria2-next` 子进程，并通过 `aria2-ws` WebSocket 客户端与之通信（随机本地端口 + 每次会话独立的 RPC 密钥）
- 两半通过 `tokio::sync::mpsc` 通道通信：
  - GUI → 引擎：通过 `mpsc::Sender` 发送 `EngineCmd`
  - 引擎 → GUI：由 `iced::Subscription` 轮询 `EngineEvent`
- 进度既来自 aria2 WebSocket **通知**，也来自 1 Hz 的**轮询**循环；UI 对脏任务做批处理，每秒刷新一次 SQLite
- aria2 会话状态通过 `--save-session` / `--input-file` 持久化，使进行中的任务跨重启恢复

```
┌──────────────┐  EngineCmd   ┌──────────────────────────┐
│   iced UI    │ ───────────► │  engine supervisor       │
│  (main loop) │              │  (tokio worker)          │
│              │ ◄─────────── │        │                 │
└──────────────┘  EngineEvent │        ├─ aria2-next     │
        ▲        (Subscription)│        │   (subprocess)  │
        └──────────────────────┤        └─ aria2-ws RPC   │
                               │           (WebSocket)    │
                               └──────────────────────────┘
```

### aria2-next 生命周期

- **首次启动** —— `aria2_fetcher::ensure_aria2_next()` 从 `AnInsomniacy/aria2-next` 的 GitHub Releases 下载匹配平台的资源到 `<data_dir>/aria2/`，校验 sha256，记录 `.installed` 并赋予可执行权限。后续启动直接命中缓存。
- **覆盖** —— 设置 `ARIA2_BIN=/path/to/aria2-next` 可完全跳过下载（便于开发）。
- **自愈** —— 若 `.installed` 缺失 / 损坏，会扫描目录中缓存的二进制并重建 `.installed`。
- **更新** —— `updater::fetch_latest_release()` 比较版本；发现新版本后在后台下载并写入 `.pending-update` 标记。待更新的二进制在下次应用 / 引擎重启时替换当前版本。
- **降级模式** —— 若获取或派生失败，引擎不会退出；UI 会显示错误并提供重试（`RetryAria2Fetch`）/ 重启（`RestartEngine`）。

### 代码结构

```
src/
├── main.rs               # 入口、日志初始化、窗口设置
├── app.rs                # Remotrix 状态、update()、view()、subscription()
├── app_updater.rs        # 应用自更新：下载暂存 + 重启时替换
├── config.rs             # 设置（serde）加载/保存、aria2 选项映射、路径辅助
├── db.rs                 # SQLite 持久化（rusqlite）：任务元数据 + 进度刷新
├── engine.rs             # EngineBridge：派生 tokio 监管者 + aria2-next 侧车、mpsc 通道
├── extension_api.rs      # 浏览器扩展 API：Salvo HTTP 服务（/ping /stat /add /pause-all /resume-all）
├── aria2_fetcher.rs      # 运行时获取 / 缓存 / 校验 aria2-next 二进制、暂存更新
├── updater.rs            # GitHub Releases 查询、ReleaseInfo、平台 slug
├── message.rs            # Message 枚举 + 页面 / 筛选 / 排序 / 设置枚举
├── task.rs               # DownloadTask 模型、格式化器、TaskDetails / TaskFile
├── i18n.rs               # 区域设置检测 + Fluent 翻译
├── clipboard_watch.rs    # 剪贴板链接检测（http/ftp/magnet/ed2k/bt）用于自动添加
├── logging.rs            # tracing 初始化、按天滚动日志、运行时日志级别
├── scheduler.rs          # 限速时间段 + 星期辅助
├── torrent_meta.rs       # .torrent 元数据解析（名称、文件、大小）
├── trackers.rs           # BT tracker 列表解析 / 精简 / 合并
├── notify.rs             # 原生系统通知（notify-rust）
├── tray.rs               # 系统托盘（ldtray）：菜单、最小化/关闭到托盘、Wayland 窗口管理
└── ui/
    ├── mod.rs            # ui 模块重导出
    ├── theme.rs          # 强调色 → iced 调色板生成、ThemeMode、控件样式
    ├── icon.rs           # iced_lucide 图标字体模块（构建期生成）
    ├── icons.rs          # 图标字形常量 + 布局宽度
    ├── dims.rs           # 共享尺寸常量
    ├── title_bar.rs      # 自定义无边框标题栏 + 窗口控制
    ├── resize_frame.rs   # 无边框窗口的自定义缩放手柄
    ├── close_dialog.rs   # 关闭确认遮罩
    ├── confirm_dialog.rs # 通用确认遮罩
    ├── sidebar.rs        # 导航：任务 / 新建 / 关于 / 设置
    ├── category_bar.rs   # 任务筛选（全部 / 下载中 / 已完成）+ 设置分类
    ├── task_list.rs      # 下载卡片：进度、操作、排序菜单
    ├── add_dialog.rs     # 新建下载遮罩（url / 种子 / 分段 / 高级）
    ├── details_dialog.rs # 任务详情：摘要 / 活动 / 文件标签页
    ├── sort.rs           # 任务排序比较器
    ├── about_dialog.rs   # 关于 / 引擎信息遮罩
    ├── update_dialog.rs  # 应用 / 引擎更新遮罩
    ├── settings_page.rs  # 常规、下载、BitTorrent、ed2k、网络、高级、外观
    └── components/       # 可复用控件（对话框、拖拽上传、分片图、文件树、时间/路径选择、Toast 等）
```

## 构建与运行

要求：较新的稳定版 Rust 工具链（推荐 `rustup`）。Linux 上可能需要 X11/Wayland 开发包。**构建期无需网络访问** —— `build.rs` 只生成图标模块；aria2-next 二进制在首次运行时获取。

```bash
cargo build                # 调试构建
cargo run --               # 启动应用（首次启动获取 aria2-next）
cargo build --release      # 发布构建（激进优化：fat LTO、strip、panic=abort）
```

## 打包

安装包由 [cargo-packager](https://github.com/crabnebula-dev/cargo-packager)（`cargo install cargo-packager --locked`）生成，由 `packager.toml` 配置。发布二进制在 GitHub Actions（`.github/workflows/release.yml`）上按平台构建——Linux `.deb` / `.AppImage`、Windows NSIS `.exe`——并作为构建产物上传（打 tag 时附加到 GitHub Release）。

```bash
cargo packager --release --config packager.toml --formats deb,appimage   # Linux
cargo packager --release --config packager.toml --formats nsis           # Windows
```

安装包只含二进制（字体、图标、i18n 均为编译期嵌入）。Linux 上运行时**需要 Vulkan**（iced/wgpu 通过 `dlopen` 加载）；aria2-next 二进制在运行时获取，刻意不打包进安装包。`deb.depends` 保持最小，因为 iced 只链接 C 运行时——deb 无法强制依赖 `dlopen` 的 GTK/X11/Vulkan 库。

使用本地 aria2-next 二进制而非自动下载：

```bash
ARIA2_BIN=/path/to/aria2-next cargo run --
```

## 检查

```bash
cargo test --workspace     # 运行测试
cargo clippy --workspace   # 静态检查（不允许有警告）
cargo fmt --check          # 格式检查
```

## 配置

设置以 JSON 持久化到平台配置目录（`directories` crate，`ProjectDirs::from("dev", "remotrix", "Remotrix")`）：

- Linux: `~/.config/remotrix/settings.json`
- macOS: `~/Library/Application Support/dev.remotrix.Remotrix/settings.json`
- Windows: `%APPDATA%\remotrix\Remotrix\config\settings.json`

运行时数据（SQLite 数据库、aria2-next 二进制缓存 + 会话、日志文件）位于数据目录：

- Linux: `~/.local/share/remotrix/`
- macOS: `~/Library/Application Support/dev.remotrix.Remotrix/`
- Windows: `%APPDATA%\remotrix\Remotrix\data\`

持久化的设置包括下载文件夹、最大并发下载数、分段数、全局与任务级限速、主题模式 + 所选浅色 / 深色主题、区域设置、自动更新偏好、关闭到托盘，以及全套 aria2 选项（每服务器最大连接数、最小分段大小、自动重命名、允许覆盖、断点续传、校验完整性、User-Agent、请求头、代理、重试、超时、bt-tracker、做种比例 / 时长、DHT 等）。

## 浏览器接管（扩展 API）

Remotrix 在本地暴露一个 HTTP 服务（默认端口 `29110`，仅监听回环地址），实现与上游 [motrix-next-extension](https://github.com/AnInsomniacy/motrix-next-extension)（MIT 许可，零改动复用）开箱即用的整套协议：

- `GET /ping` —— 心跳（无鉴权）
- `GET /stat` —— 全局统计（`downloadSpeed` / `numActive` 等，字符串形式镜像 aria2 `getGlobalStat`）
- `POST /add` —— 添加下载（referer / cookie / UA / 请求头随任务提交）
- `POST /pause-all` / `POST /resume-all` —— 全局暂停 / 恢复

**使用方式**：

1. 从浏览器商店安装 `motrix-next-extension`（Remotrix 不打包扩展）。
2. 在 Remotrix 设置 →「下载」→「浏览器接管」中启用并复制端口与密钥。
3. 在扩展选项页把地址填为 `http://127.0.0.1:<端口>` 并填入密钥，`checkConnection` 通过即可。

默认情况下 `/add` 会**静默自动提交**任务并弹出系统通知；可在设置中改为弹「添加」对话框二次确认。密钥留空则关闭鉴权（仅限回环地址）。默认只提供全局暂停 / 恢复，单任务管理请回到 Remotrix 主窗口完成。

修改「启用 / 端口 / 密钥 / 自动提交」后点击「**应用**」会**立即热重启**扩展服务（无需重启应用），并以 toast 提示「正在重启… / 已重启 / 重启失败」；若关闭启用开关则会停止服务并提示「已停止」。仅在点击「应用」时生效，修改开关或输入本身不会触发重启。

> 已知限制：`motrixnext://` 深链协议未实现，「打开 motrix-next」唤醒按钮暂不可用；下载接管走 HTTP，不受影响。

## 技术栈

| 组件 | 选型 | 理由 |
|---|---|---|
| GUI | `iced 0.14`（+tokio、advanced、canvas、svg、markdown） | 纯 Rust、基于控件、支持深色主题 |
| 引擎 | `aria2-next` 侧车 + `aria2-ws 0.5` | C++ aria2 分支，WebSocket 上的 JSON-RPC，以子进程方式派生 |
| 异步 | `tokio 1.x`（full） | 引擎 + UI 共享运行时 |
| 持久化 | `rusqlite 0.40`（bundled + fallible_uint） | 内嵌 SQLite，存储任务元数据 / 进度 |
| 主题 | iced `Theme::custom`（内置） | 强调色色块；iced 自动生成浅 / 深色调色板 |
| i18n | `fluent-templates 0.15` | Fluent 翻译（zh / en） |
| 系统主题 | `dark-light 2.0` | 检测系统深色 / 浅色偏好 |
| 文件对话框 | `rfd 0.17` | 原生文件选择器 |
| 系统托盘 | `ldtray 0.1` | 托盘图标 + 菜单 |
| 通知 | `notify-rust 4.17`（tokio） | 原生桌面通知 |
| 单实例 | `app-single-instance 0.1` | 单实例运行并聚焦已有窗口 |
| HTTP 客户端 | `reqwest 0.13`（rustls、json） | GitHub Releases 获取 / 更新器 |
| 哈希 | `sha2 0.11` | aria2-next 二进制校验和验证 |
| 图标 | `iced_lucide 0.1`、`iced_aw 0.14` | 图标字体 + 时间选择器 |
| 图像 | `image 0.25`（png） | 应用图标加载 |
| 日志 | `tracing` + `tracing-appender 0.2` | 滚动文件日志 |
| 配置目录 | `directories 6` | XDG / 用户数据路径 |
| 字体 | `fontdb 0.24` | 设置中用到的系统字体枚举 |
| 进程 | `libc 0.2` | 对残留 aria2-next 进程发送 SIGTERM/SIGKILL |
| 时间 | `chrono 0.4`（clock） | 时间戳格式化 |

## 路线图

**已完成**
- [x] 双循环引擎桥 + aria2-next 侧车监管者
- [x] 基础 UI：侧边栏、分类栏、任务列表、添加对话框、设置
- [x] 无边框窗口 + 自定义标题栏
- [x] i18n（zh / en）+ 强调色主题 + 系统自动主题
- [x] SQLite 任务持久化
- [x] aria2-next 运行时自动获取 + 自动更新
- [x] 任务详情对话框（分片图、文件、BT 信息）
- [x] Magnet 磁力链接支持
- [x] 拖拽文件 / 任务支持
- [x] 系统托盘集成（X11 下支持最小化到托盘 / 关闭到托盘；Wayland 下不完全，界面只能最小化不能隐藏）
- [x] 系统通知（下载完成等）
- [x] 单实例运行

**待办**
- [ ] 下载功能完善与全面测试 —— HTTP/HTTPS 与 BT 目前可用，但尚未覆盖足够多的场景（断点续传、完整性校验、限速、失败重试等），需要完整回归测试
- [ ] 系统级单元测试 —— 为引擎、任务解析、配置、调度等核心模块补充单元测试
- [ ] 应用动画效果 —— 为页面切换、列表更新、进度条等添加流畅的过渡与动效
- [ ] 各类路径的自定义 —— 支持自定义应用缓存、日志、aria2-next 二进制等路径
- [ ] 文件关联 —— 设置各类文件（如 `.torrent`）的默认打开程序
- [ ] `motrixnext://` 深链协议 —— 应用未启动时被扩展唤醒 / 网站下载按钮（当前核心 HTTP 拦截不依赖它）
- [x] 开机自启动 —— 支持登录后自动启动，可设置自启时隐藏到托盘
- [x] 浏览器接管 —— 本地扩展 API + 复用 motrix-next-extension 一键接管浏览器下载
- [ ] 定时关机 / 下载完成关机 —— 支持定时关机与全部任务完成后自动关机
- [ ] Wayland 托盘兼容性完善 —— 目前在 Wayland 下窗口只能最小化无法完全隐藏，待完善窗口隐藏 / 托盘 / 通知兼容性
- [ ] UI/UX 优化 —— 持续打磨界面细节与交互体验

## 致谢

- [aria2-next](https://github.com/AnInsomniacy/aria2-next) by AnInsomniacy —— 核心下载引擎。它是一个独立的、独立许可的程序（GPL-2.0-or-later），**不随 Remotrix 一并打包**；运行时从其 GitHub Releases 下载。
- [motrix-next-extension](https://github.com/AnInsomniacy/motrix-next-extension)（MIT）—— 浏览器端扩展，Remotrix **不打包、不修改**，仅由用户从商店 / 上游安装，通过本地扩展 API 与 Remotrix 通信。

## 协议

MIT。完整许可文本见 `LICENSE` 文件。
