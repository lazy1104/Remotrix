# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- 「立即检查」在 GitHub release 记录内嵌 `assets` 数组为空时（GitHub 已知 snapshot 不一致，例如发布后再补传资产的 release），回退到该 release 的 `assets_url` 拉取权威资产清单再匹配；之前会让 beta 通道或类似情况下的「最新」漏报。

## [0.5.1] - 2026-09-23

### Changed

- 任务列表每张卡片增加 hover 反馈：鼠标移入时边框切换为主题主色并显示轻微投影，背景保持不变。

### Fixed

- 修复 Windows 设置页「网络」「高级」等分类中，行内控件文字比左侧标签字符略高的基线偏移问题（统一通过 `setting_row` 的容器居中标签实现）。

## [0.5.0] - 2026-09-22

### Added

- 后台下载补齐进度节流（~5Hz）、有限次数重试加退避（500ms/1s/2s）、流式 sha256 校验与 `{dest}.part` Range 断点续传；同时被 aria2 更新和应用更新两条路径共享。
- 应用更新独立于 aria2 sidecar，直接在 UI 任务中通过共享下载器拉取，下载完成后弹带「更新」动作按钮的粘性 toast，由用户点击后再 apply；「立即检查」与启动检查会识别磁盘上已下载但未应用的更新包，复用同一 toast，不重复下载。
- aria2 更新完成后的应用内 toast 也加上「重启引擎」动作按钮，复用 `EngineMsg::RestartEngine` 确认流程；系统通知按钮和设置页入口保留。
- 新文案 `update-apply`（应用）、`update-ready-click-to-apply`（更新已就绪，点击应用）。

### Changed

- 移除应用更新走 aria2 sidecar 的整套通道（`EngineCmd::DownloadAppUpdate` / `EngineEvent::AppUpdateDownloaded/Failed` / `download_via_engine` / `handle_download_app_update_via_engine`），改用 app 层共享后台下载；不再依赖 engine 处于可用状态。
- 应用更新的「Apply」由原来的下载完成即自动触发（deb 打开目录、AppImage 立即替换、Windows 立即启动安装器）改为弹 toast 等待用户点击，避免磁盘上残留 `.deb` 时应用后无法定位包。
- 设置面板移除单独的「aria2-next 静默更新」开关；新增「静默更新范围」下拉（关闭 / 引擎 / 应用 / 应用 + 引擎，4 选项），决定哪个组件在检查到更新时跳过对话框直接后台下载；应用静默只下载，应用动作仍需用户点击 toast 上的「更新」按钮触发（与 aria2 静默「不自动重启引擎」语义一致）。原有 `aria2_silent_update=true` 的旧字段通过一次性迁移映射为「引擎」，`false` 映射为「关闭」。
- 系统深色模式检测依赖升级到 `dark-light 3`，新版本的 `Mode::Unspecified` 与旧版「无法确定」语义一致，统一回退到浅色主题。
- 主题模式为「跟随系统」时，OS 深色 / 浅色切换（macOS / Windows / XDG Desktop Portal 支持的桌面环境）现在会在运行时即时应用到 UI，无需重启应用；手动选择「深色」/「浅色」时仍以用户设置为准。
- 默认字体改为跟随系统 UI 字体；当 `Settings.font_family` 为空（「系统默认」或首次启动）时，每次启动按平台调 OS-native API 重新解析（Linux 走 `fc-match`、macOS 走 `defaults read -g AppleSystemUIFont`、Windows 走 `SPI_GETNONCLIENTMETRICS.lfMessageFont`），结果仅作内存值不入盘；任一平台 OS-native 查询失败时回退到 `system_fonts::find_for_system_locale`（fontdb locale-aware 枚举）。用户在设置中选了具体族名则被锁定到该族。

### Fixed

- 顶部进度条在 BitTorrent tracker 同步和 ED2K bootstrap（server.met / nodes.dat）同步进行时也会显示，与 aria2 启动、升级下载、立即检查等后台任务一致。
- 引擎处于降级 / 失败态时，顶部动画进度条不再无限转动；改由静态错误 toast 提示。瞬时忙碌（启动、下载、立即检查等）仍会触发动画条。
- ED2K 设置页中 server.met 与 nodes.dat 的文件选择组件，路径未填写时「复制」「在文件夹中显示」两个图标按钮现在仍显示为可点击（hover 变手指、点击有响应），并以 35% 透明度的图标呈现禁用态；其余下载路径 / 应用路径 / 日志路径选择器以及数字步进器的「-」「+」按钮沿用同一禁用态风格。
- 设置/ED2K/添加任务面板中所有「选择文件夹」「选择 .torrent」「选择 .metalink」「选择 server.met」「选择 nodes.dat」文件对话框现在通过 `iced::window::run` 拿到主窗口原生句柄并以 `set_parent` 绑定，定位与 z 序跟随主窗口；主窗口尚未就绪时仍以未绑定形式打开。

### Removed

- 移除 release workflow 中的 cargo-deny 检查任务及其 `deny.toml` 配置；`bans licenses sources` 的默认配置在大量合法依赖下误报，advisories 路径本来就 `continue-on-error`，整体收益不抵维护成本。
- 移除内置 HarmonyOS Sans SC 字体（~8 MB），二进制不再随包分发 CJK 字体文件。

## [0.4.0] - 2026-09-20

### Added

- 任务筛选侧栏新增「失败」分类，可快速查看 aria2 报告为 error 的任务。
- 导航切换引入 Swap 组件、SwapTarget 状态机与缩放浮出动画；分类栏内容在页面切换时跟随动画同步出现。

### Changed

- 任务列表分类栏数量改为右对齐胶囊徽标显示，封顶 "99+"，零计数时隐藏；胶囊字体使用 FONT_TINY 以弱化视觉权重。
- 顶部边框在后台任务（aria2 启动 / 升级二进制下载 / 远端更新检查 / 应用自更新下载 / 引擎重启）期间显示为以主题 primary 为主色、向两侧渐变到普通边框色并循环位移的 2px 横条；其他时段保持原 1px 静态边框。
- 工具栏「全部开始 / 全部暂停 / 删除全部 / 移除记录」改为按当前筛选范围生效：默认「全部」/「下载中」标签下仅作用于 Active / Waiting / Paused 子集；「已完成」「失败」标签下移除这三个按钮，仅保留新建、刷新、排序、清空记录；「清空记录」在「失败」标签下仅清理失败任务。
- 字体选择器（已有）改为可搜索列表，并按当前界面语言显示本地化字体名称。
- 颜色选择器（已有）重构为弹出浮层，移除 Alpha 通道支持；新增色板图标组件替换默认图标；Hex 输入支持实时校验与错误提示。
- 设置页「最小分片大小」「重试等待」「自动下载磁力与种子内容」「日志管理」等标签措辞优化；隐藏「包含预发布版本更新 (Beta)」选项（后端字段保留，默认关）。

### Fixed

- 启动时改为基于 aria2 RPC 主动剔除 session 中处于 error 状态的任务，止住跨重启重试循环（aria2 自身的 `--input-file` 在 `--save-session` 重写时若不带 `pause=true` 会反复复活失败任务）。
- 中间筛选栏宽度加宽至 216 px，防止英文文本在紧凑窗口下换行。

### Removed