# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

- 顶部进度条在 BitTorrent tracker 同步和 ED2K bootstrap 同步进行时也会显示。

### Fixed

### Removed

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