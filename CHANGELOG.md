# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- 任务筛选侧栏新增「失败」分类，可快速查看 aria2 报告为 error 的任务。

### Changed

- 任务列表筛选栏数量改为右对齐胶囊显示，封顶 "99+"，零计数时隐藏胶囊；胶囊字体使用 FONT_TINY 以弱化视觉权重。
- 「已完成」「失败」标签下的工具栏按钮精简为新建、刷新、排序、清空记录，移除「全部开始 / 全部暂停 / 删除全部」以避免误操作已完成或失败任务。
- 「全部」标签下的「全部开始 / 全部暂停 / 删除全部 / 移除记录」仅作用于当前下载中子集（Active / Waiting / Paused），不再影响已完成或失败任务；「清空记录」在「失败」标签下仅清理失败任务，其他标签下沿用原行为。

### Fixed

### Removed