# Remotrix

> **[中文](README.md) | [English](README_EN.md)**

基于 Rust 的原生桌面下载管理器，灵感来自 [Motrix-next](https://motrix-next.pages.dev/)，采用 [`iced`](https://github.com/iced-rs/iced) GUI 框架，并通过 WebSocket JSON-RPC 驱动 [`aria2-next`](https://github.com/AnInsomniacy/aria2-next) 侧车引擎（[`aria2-ws`](https://crates.io/crates/aria2-ws)）。

> 名称 "Remotrix" 是 **Rust** 与 **Motrix** 的合成词。

> **⚠ 开发中 / 非稳定版**
> Remotrix 是一个**由 AI 辅助开发的学习项目**，当前处于开发阶段，功能**尚未完全覆盖、未经全面验证**。部分功能（断点续传、完整性校验、限速、失败重试及跨平台兼容性）尚未经过完整回归测试，可能包含 bug 或数据损失风险。**请仅用于测试，勿用于下载重要文件**。欢迎提交 Issue / PR。

## 为什么做 Remotrix？

Remotrix 最初是一个学习项目。我喜欢 Motrix / Motrix-next 的设计，但基于 Tauri 的 Motrix-next 在我的 Windows 10 机器上无法正常运行，在 Linux 机器上又有严重的性能问题。我想学习 Rust 和 `iced` GUI 框架，于是决定从零开始做一个原生的 Rust 下载管理器，以 Motrix-next 作为设计参考。本项目由 AI 辅助开发。

## 功能特性

- **下载与引擎** —— aria2-next 自管理侧车（运行时获取、sha256 校验、缓存、自愈、自动更新）；HTTP/HTTPS/FTP、BitTorrent、磁力链接，可配置每服务器分段与连接数，全局与任务级下载 / 上传限速持久化，任务元数据与进度存本地 SQLite。
- **窗口与桌面集成** —— `iced 0.14` 纯 Rust 渲染、自定义无边框窗口与缩放；系统托盘（X11 完整支持，Wayland 仅最小化，Windows 附带 `TaskbarCreated` 看门狗）；跨平台系统通知（Windows 额外走 `win_toast`）；`app-single-instance` 单实例聚焦。
- **主题与国际化** —— 强调色色板自动派生完整浅 / 深色调色板（含 M3 风格表面背景），可选 HEX 取色 + HCT 派生；跟随系统深色 / 浅色以及 macOS 14+ / Windows 11 / GNOME 强调色，主题切换走平滑过渡；`zh_CN` / `en_US` 自动检测；默认字体跟随系统 UI 字体（Linux `fc-match`、macOS NSFont、Windows SPI），用户可锁定具体字体。
- **任务浏览** —— 任务详情多标签页（摘要 / 活动 / 文件 / Advanced），BT 分片完成度图与三态复选框文件树单任务选择；按添加时间 / 名称 / 大小 / 进度 / 状态排序，按全部 / 下载中 / 已完成 / 失败筛选，分类胶囊带计数徽标。
- **剪贴板与浏览器接管** —— 自动检测复制到剪贴板的 http/ftp/magnet/ed2k/bt 链接；本地 Salvo HTTP 服务（默认 `127.0.0.1:29110`，支持热重启）兼容上游 [motrix-next-extension](https://github.com/AnInsomniacy/motrix-next-extension)（MIT，零改动复用）一键接管浏览器下载。
- **扩展协议** —— ED2K（server.met / nodes.dat 自动 bootstrap + 内置搜索）、Metalink（`.metalink` / `.meta4`）、BT tracker 自动同步（预设源 + 自定义 URL + 定时同步）、BT metadata-only 下载。
- **自动化与系统集成** —— HTTP / SOCKS5 全局 + 单任务代理（Basic Auth）；限速时段调度（时间窗 + 星期）；定时关机 / 全部完成关机；下载时跨平台阻止系统休眠；登录自启动（Linux `.desktop` / Windows `Run`）；设置 → 高级 → 路径可分别覆盖 aria2 / 应用缓存 / 日志目录，切换后下次启动自动迁移数据。
- **更新与升级** —— aria2 与应用统一共享后台下载器（节流、重试、流式 sha256、Range 断点续传）；aria2 静默更新 + 顶边进度条；应用自更新 AppImage / deb / NSIS；「静默更新范围」四档（关闭 / 引擎 / 应用 / 应用 + 引擎），应用静默仅下载，动作由用户点击粘性 toast 上的「更新」按钮触发；RPC 端口可配置，端口冲突自动回退空闲端口。

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

- **首次启动** —— `updaters::aria2_fetcher::ensure_aria2_next()` 从 `AnInsomniacy/aria2-next` 的 GitHub Releases 下载匹配平台的资源到 `<data_dir>/aria2/`，校验 sha256，记录 `.installed` 并赋予可执行权限。后续启动直接命中缓存。
- **覆盖** —— 设置 `ARIA2_BIN=/path/to/aria2-next` 可完全跳过下载（便于开发）。
- **自愈** —— 若 `.installed` 缺失 / 损坏，会扫描目录中缓存的二进制并重建 `.installed`。
- **更新** —— `updaters::updater::fetch_latest_release()` 比较版本；发现新版本后在后台下载并写入 `.pending-update` 标记。待更新的二进制在下次应用 / 引擎重启时替换当前版本。
- **降级模式** —— 若获取或派生失败，引擎不会退出；UI 会显示错误并提供重试（`RetryAria2Fetch`）/ 重启（`RestartEngine`）。

### 代码结构

```
src/
├── main.rs                      # 入口、日志初始化、窗口设置
├── lib.rs                       # 库入口（lib + bin）
├── app.rs                       # Remotrix 状态、update()、view()、subscription()
├── config/
│   ├── mod.rs
│   ├── paths.rs                 # 路径解析器（aria2_bin_dir/session_dir/db_path/log_dir/config_file_path）
│   ├── settings.rs              # 设置加载/保存、aria2 选项映射
│   ├── install.rs               # 用户级 .desktop / 图标安装
│   └── migrations.rs            # 设置一次性迁移
├── core/
│   ├── mod.rs
│   ├── i18n.rs                  # 区域设置检测 + Fluent 翻译
│   ├── logging.rs               # tracing 初始化、按天滚动日志、运行时日志级别
│   ├── message.rs               # Message 枚举 + 页面 / 筛选 / 排序 / 设置枚举
│   ├── scheduler.rs             # 限速时间段 + 星期辅助
│   └── task.rs                  # DownloadTask 模型、格式化器、TaskDetails / TaskFile
├── engine/
│   ├── mod.rs                   # EngineBridge：派生 tokio 监管者 + aria2-next 侧车、mpsc 通道
│   ├── install.rs               # 引擎二进制获取/缓存/校验的运行时封装
│   └── ed2k.rs                  # ED2K 协议钩子
├── integration/
│   ├── mod.rs
│   ├── clipboard_watch.rs       # 剪贴板链接检测（http/ftp/magnet/ed2k/bt）
│   └── extension_api.rs         # 浏览器扩展 API：Salvo HTTP 服务（/ping /stat /add /pause-all /resume-all）
├── platform/
│   ├── mod.rs
│   ├── autostart.rs             # 开机自启动（Linux .desktop / Windows Run）
│   ├── notify.rs                # notify-rust 系统通知
│   ├── port_guard.rs            # RPC 端口冲突回退
│   ├── power.rs                 # 阻止系统休眠（systemd-inhibit / caffeinate / SetThreadExecutionState）
│   ├── shutdown.rs              # 定时关机 / 全部完成关机
│   ├── tray.rs                  # ldtray 托盘 + 菜单 + Wayland 窗口管理
│   ├── tray_watchdog.rs         # Windows 监听 TaskbarCreated 自动重建托盘
│   └── win_toast.rs             # Windows 原生 Toast（AUMID + Locate/Reveal）
├── storage/
│   ├── mod.rs
│   ├── db.rs                    # SQLite 持久化（rusqlite）：任务元数据 + 进度刷新
│   ├── ed2k_bootstrap.rs        # server.met / nodes.dat 自动同步
│   ├── torrent_meta.rs          # .torrent 元数据解析（名称、文件、大小）
│   └── trackers.rs              # BT tracker 列表解析 / 精简 / 合并
├── update/
│   ├── mod.rs
│   ├── add.rs                   # 添加任务路径
│   ├── engine.rs                # 引擎侧更新状态机
│   ├── misc.rs
│   ├── task.rs                  # 更新任务状态机
│   ├── window.rs                # 更新 UI 状态机
│   └── settings/
│       ├── mod.rs
│       └── network.rs           # 更新代理配置
├── updaters/
│   ├── mod.rs
│   ├── app_updater.rs           # 应用自更新（deb / AppImage / NSIS）
│   ├── aria2_fetcher.rs         # aria2 二进制获取 / 缓存 / 校验 / 暂存
│   ├── download.rs              # 共享后台下载器（节流、重试、流式 sha256、Range 断点续传）
│   └── updater.rs               # GitHub Releases 查询、ReleaseInfo、平台 slug
└── ui/
    ├── mod.rs                   # ui 模块重导出
    ├── view.rs                  # 顶层 view 入口
    ├── animation.rs             # ANIM_* 时长 token
    ├── border_bar.rs            # 顶部边框进度条（渐变位移）
    ├── scroll_anim.rs
    ├── system_default_font.rs   # 平台默认字体解析（fc-match / NSFont / SPI）
    ├── font_autopick.rs         # locale-aware fontdb 枚举回退
    ├── color/
    │   ├── mod.rs
    │   ├── theme.rs             # 强调色 → iced 调色板生成、ThemeMode
    │   └── hct.rs               # HCT 感知色彩空间派生
    ├── settings/
    │   ├── mod.rs
    │   ├── general.rs           # 常规设置
    │   ├── download.rs          # 下载设置
    │   ├── bittorrent.rs        # BT 设置（含 tracker 同步）
    │   ├── ed2k.rs              # ED2K 设置（含 server.met / nodes.dat 选择）
    │   ├── network.rs           # 代理设置
    │   └── advanced.rs          # 高级设置（含路径、浏览器接管、日志）
    ├── icon.rs                  # iced_lucide 图标字体模块（构建期生成）
    ├── icons.rs                 # 图标字形常量 + 布局宽度
    ├── dims.rs                  # 共享尺寸常量
    ├── title_bar.rs             # 自定义无边框标题栏 + 窗口控制
    ├── resize_frame.rs          # 无边框窗口的自定义缩放手柄
    ├── close_dialog.rs          # 关闭确认遮罩
    ├── confirm_dialog.rs        # 通用确认遮罩
    ├── sidebar.rs               # 导航：任务 / 新建 / 关于 / 设置
    ├── category_bar.rs          # 任务筛选（全部 / 下载中 / 已完成 / 失败）+ 设置分类
    ├── task_list.rs             # 下载卡片：进度、操作、排序菜单
    ├── add_dialog.rs            # 新建下载遮罩（url / 种子 / 分段 / 高级）
    ├── details_dialog.rs        # 任务详情：摘要 / 活动 / 文件 / Advanced 标签页
    ├── sort.rs                  # 任务排序比较器
    ├── about_dialog.rs          # 关于 / 引擎信息遮罩
    ├── update_dialog.rs         # 应用 / 引擎更新遮罩
    └── components/              # 可复用控件
        ├── dialog.rs / popover.rs / toast.rs          # 弹层 / 浮层 / 提示
        ├── path_picker.rs / color_picker.rs / font_picker.rs / tag_picker.rs / drop_down.rs / number_stepper.rs  # 选择器
        ├── secret_input.rs / ctx_input.rs / ctx_menu.rs / copyable_text.rs / truncated_text.rs / translate.rs  # 文本输入
        ├── file_drop_zone.rs / torrent_upload.rs / drop_overlay.rs  # 拖放
        ├── file_tree.rs / tri_checkbox.rs / torrent_file_list.rs / piece_map.rs / key_value_list.rs / logo.rs  # 任务详情
        ├── speed_hud.rs / speed_popover.rs / shutdown_popover.rs  # HUD
        ├── status_dot.rs / spinner.rs / swatch_icon.rs / scale.rs  # 装饰
        ├── expand.rs / slim_scrollable.rs / scroll_top_gap.rs / tooltip.rs  # 布局
        └── mod.rs
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

> 当前 GitHub Releases 中的版本均为**开发中构建**，用于测试更新机制，不代表稳定正式版。

使用本地 aria2-next 二进制而非自动下载：

```bash
ARIA2_BIN=/path/to/aria2-next cargo run --
```

## 检查

```bash
cargo test --workspace                                          # 单元 / 集成测试
cargo test --test download_e2e -- --nocapture                   # 端到端下载测试（需设置 ARIA2_BIN）
cargo clippy --workspace                                        # 静态检查（不允许有警告）
cargo fmt --check                                               # 格式检查
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

持久化的设置包括下载文件夹、最大并发下载数、分段数、全局与任务级限速、主题模式 + 所选浅色 / 深色主题、跟随系统强调色开关、区域语言、字体选择、自动更新偏好与静默更新范围、关闭到托盘，以及全套 aria2 选项（每服务器最大连接数、最小分段大小、自动重命名、允许覆盖、断点续传、校验完整性、User-Agent、请求头、代理、重试、超时、bt-tracker、做种比例 / 时长、DHT 等）。

「设置 → 高级 → 路径」分组可分别覆盖 **aria2-next 目录**、**应用缓存目录**、**日志目录**。修改后**不会立刻生效**——界面显示「待保存更改」与重启提示，需点「保存并重启应用」才一次性提交；下次启动时若新路径与原 `last_resolved` 不同，应用会把旧路径下对应的数据（aria2 bin dir 的全部文件、日志目录的全部文件、应用缓存目录的 `remotrix.db` / `ed2k-bootstrap/` / `ed2k-search/` 白名单子项）**物理复制**到新路径；新路径若已存在同名文件则跳过单文件复制，原路径文件不会被删除。只改「应用缓存目录」时，默认内嵌其中的 aria2 目录与日志目录会跟随迁移到新缓存目录；单独设置 aria2 / 日志目录时仍以显式设置为准。`ARIA2_BIN` 环境变量优先级最高，UI 上的覆盖对其无效。

## 浏览器接管（扩展 API）

Remotrix 在本地暴露一个 HTTP 服务（默认端口 `29110`，仅监听回环地址），实现与上游 [motrix-next-extension](https://github.com/AnInsomniacy/motrix-next-extension)（MIT 许可，零改动复用）开箱即用的整套协议：

- `GET /ping` —— 心跳（需鉴权）
- `GET /stat` —— 全局统计（`downloadSpeed` / `numActive` 等，字符串形式镜像 aria2 `getGlobalStat`）
- `POST /add` —— 添加下载（referer / cookie / UA / 请求头随任务提交）
- `POST /pause-all` / `POST /resume-all` —— 全局暂停 / 恢复

**使用方式**：

1. 从浏览器商店安装 `motrix-next-extension`（Remotrix 不打包扩展）。
2. 在 Remotrix 设置 →「高级」→「浏览器接管」中启用并复制端口与密钥。
3. 在扩展选项页把地址填为 `http://127.0.0.1:<端口>` 并填入密钥，`checkConnection` 通过即可。

默认情况下 `/add` 会**静默自动提交**任务并弹出系统通知；可在设置中改为弹「添加」对话框二次确认。**密钥始终必需**：若设置里没有密钥，会在启动时自动生成并持久化；所有接口（`/ping /stat /add /pause-all /resume-all`）均要求 `Authorization: Bearer <密钥>`。默认只提供全局暂停 / 恢复，单任务管理请回到 Remotrix 主窗口完成。

修改「启用 / 端口 / 密钥 / 自动提交」后点击「**应用**」会**立即热重启**扩展服务（无需重启应用），并以 toast 提示「正在重启… / 已重启 / 重启失败」；若关闭启用开关则会停止服务并提示「已停止」。仅在点击「应用」时生效，修改开关或输入本身不会触发重启。

> 已知限制：`motrixnext://` 深链协议未实现，「打开 motrix-next」唤醒按钮暂不可用；下载接管走 HTTP，不受影响。

## 技术栈

| 组件 | 选型 | 理由 |
|---|---|---|
| GUI | `iced 0.14`（+tokio、advanced、canvas、svg、markdown） | 纯 Rust、基于控件、支持深色主题 |
| 引擎 | `aria2-next` 侧车 + `aria2-ws 0.5` | C++ aria2 分支，WebSocket 上的 JSON-RPC，以子进程方式派生 |
| 异步 | `tokio 1.x`（full） | 引擎 + UI 共享运行时 |
| 持久化 | `rusqlite 0.40`（bundled + fallible_uint） | 内嵌 SQLite，存储任务元数据 / 进度 |
| 主题 | iced `Theme::custom` + `palette 0.7` | 强调色 → 调色板生成；HCT 感知色彩空间派生浅 / 深色主题 |
| 系统偏好 | `mundy 0.2`（color-scheme + accent-color + async-io） | 统一系统深色 / 浅色 + OS 强调色读取（取代 dark-light） |
| i18n | `fluent-templates 0.15` | Fluent 翻译（zh / en） |
| 字体 | `fontdb 0.24` + `system-fonts 0.1` | locale-aware 系统字体枚举；平台默认字体走 OS-native API |
| 文件对话框 | `rfd 0.17` | 原生文件选择器 |
| 系统托盘 | `ldtray 0.1`（固定 `=0.1.2`） | 托盘图标 + 菜单；Windows 看门狗自动恢复 Explorer 重启后的图标 |
| 通知 | `notify-rust 4.17`（tokio） | 跨平台原生通知；Windows 额外走 `win_toast`（AUMID + Locate / Reveal） |
| 单实例 | `app-single-instance 0.1` | 单实例运行并聚焦已有窗口 |
| HTTP 客户端 | `reqwest 0.13`（rustls、json、socks） | GitHub Releases 获取 / 更新器 / 代理 |
| HTTP 服务 | `salvo 0.95`（cors + http1 + server-handle） | 浏览器扩展本地回环 API，支持热重启 |
| 哈希 | `sha2 0.11` | aria2-next / 应用更新二进制校验和验证 |
| 图标 | `iced_lucide 0.1`、`iced_aw 0.14` | 图标字体 + 时间选择器 |
| 动画 | `iced_anim =0.3.1` | 7 个统一时长 token 的过渡动画 |
| 图像 | `image 0.25`（png） | 应用图标加载 |
| 日志 | `tracing` + `tracing-appender 0.2` + `tracing-subscriber 0.3` | 滚动文件日志 + env-filter |
| 配置目录 | `directories 6` | XDG / 用户数据路径 |
| 进程 | `libc 0.2` | 对残留 aria2-next 进程发送 SIGTERM/SIGKILL |
| 时间 | `chrono 0.4`（clock） | 时间戳格式化 |
| 文件 / 目录 | `open 5` | 打开文件 / 目录 / URL |
| Windows | `windows 0.62`、`libloading 0.9` | 任务栏看门狗、AUMID、原生 Toast、阻止休眠 |

## 路线图

**已完成**

- **架构与 UI** —— 双循环引擎桥 + aria2-next 侧车监管者；侧边栏 / 分类栏 / 任务列表 / 添加对话框 / 设置等基础 UI；无边框窗口 + 自定义标题栏 + 缩放；单实例运行；统一动画系统（7 个 `ANIM_*` token，覆盖卡片 / 对话框 / 进度条 / 速度 HUD / 分类胶囊 / Toast / 弹出遮罩）。
- **引擎与协议** —— aria2-next 运行时获取 + sha256 校验 + 自愈 + 自动更新；任务元数据与进度持久化到 SQLite；ED2K（server.met / nodes.dat 自动 bootstrap + 内置搜索）、Metalink、BT tracker 自动同步、BT metadata-only；HTTP / SOCKS5 全局 + 单任务代理（Basic Auth）。
- **桌面集成** —— 系统托盘（X11 完整，Wayland 仅最小化，Windows 含 `TaskbarCreated` 看门狗）；跨平台系统通知（Windows 额外走 `win_toast`）；剪贴板链接自动检测；应用文件拖放（含 overlay 提示）。
- **主题与本地化** —— i18n（zh / en）；强调色主题 + 跟随系统外观 + 跟随系统强调色 + 主题过渡动画；自定义强调色 + HCT 派生；默认字体跟随系统（Linux `fc-match` / macOS NSFont / Windows SPI，失败回退 fontdb）。
- **自动化与系统** —— 限速时段调度（时间窗 + 星期）；定时关机 / 全部完成关机；下载时跨平台阻止系统休眠；登录自启动；设置 → 高级 → 路径自定义覆盖并下次启动自动迁移数据。
- **更新与升级** —— 应用自更新（AppImage / deb / NSIS），统一共享后台下载器（节流 / 重试 / 流式 sha256 / Range 断点续传），完成弹粘性 toast 由用户点击「更新」按钮 apply；aria2 静默更新 + 后台暂存 + 系统通知重启动作；RPC 端口可配置，端口冲突自动回退；顶边进度条统一反馈后台任务。
- **设置扩展与体验细节** —— 任务详情多标签页（含 BT 分片图、文件树、Advanced UA / cookie / headers）；单任务三态复选框文件选择；分类筛选胶囊带计数徽标；排序 / 路径选择历史 / 右键菜单；设置项扩展（自动清理、并发上限、每任务限速、文件分配、日志级别、变更日志渲染等）；优雅关闭；「关于」对话框。
- **测试** —— 引擎 / 任务 / 调度等核心模块单元测试；`download_e2e` 端到端下载测试（需 `ARIA2_BIN`）。

**待办**

- [ ] 下载功能完善与全面测试 —— HTTP/HTTPS 与 BT 目前可用，但尚未覆盖足够多的场景（断点续传、完整性校验、限速、失败重试等），需要完整回归测试
- [ ] 系统级单元测试 —— 补齐 `db.rs` / `app.rs` / 各 dialog 视图 / `tray.rs` / `extension_api.rs` Salvo 处理器等尚未覆盖模块的单元与集成测试
- [ ] 文件关联 —— 设置各类文件（如 `.torrent`）的默认打开程序
- [ ] `motrixnext://` 深链协议 —— 应用未启动时被扩展唤醒 / 网站下载按钮（当前核心 HTTP 拦截不依赖它）
- [ ] Wayland 托盘兼容性完善 —— 目前在 Wayland 下窗口只能最小化无法完全隐藏到托盘；目前通过系统通知唤醒，待完善窗口隐藏 / 托盘 / 通知兼容性
- [ ] UI/UX 优化 —— 持续打磨界面细节与交互体验

## 致谢

- [aria2-next](https://github.com/AnInsomniacy/aria2-next) by AnInsomniacy —— 核心下载引擎。它是一个独立的、独立许可的程序（GPL-2.0-or-later），**不随 Remotrix 一并打包**；运行时从其 GitHub Releases 下载。
- [motrix-next-extension](https://github.com/AnInsomniacy/motrix-next-extension)（MIT）—— 浏览器端扩展，Remotrix **不打包、不修改**，仅由用户从商店 / 上游安装，通过本地扩展 API 与 Remotrix 通信。

## 协议

MIT。完整许可文本见 `LICENSE` 文件。