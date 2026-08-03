# 修改 Toast 默认显示位置为 Top

## 目标
将 `Toast` 的默认位置从 `BottomRight` 改为 `Top`，避免每次调用都需要写 `.position(ToastPosition::Top)`。

## 改动

### 1. `src/ui/components/toast.rs` — 修改默认值

**`ToastPosition` 枚举**：将 `#[default]` 从 `BottomRight` 移到 `Top`。

**`Toast::new()`**：将 `position: ToastPosition::BottomRight` 改为 `position: ToastPosition::Top`。

## 影响分析

| 调用位置 (`app.rs`) | 当前 | 改后 |
|---|---|---|
| Line 1227 (ClearLogs success) | 无 `.position()`，显示 BottomRight | 显示 Top |
| Line 1235 (ClearLogs failed) | 无 `.position()`，显示 BottomRight | 显示 Top |
| Line 1922, 1951, 1977, 2021, 2830, 2955 | `.position(ToastPosition::Top)` 显式指定 | 与默认一致，冗余但无害 |

## 验证
- `cargo build` 编译通过
- `cargo clippy --workspace` 无警告
