# Torrent 文件列表改为「最大高度」布局

## 目标
文件数量少时，torrent 文件列表按内容自动收缩（不再固定占满/Fixed(230)），文件多时仍按上限高度滚动。

## 方案原理（已验证 iced 0.14 布局）
- `iced_core::layout::limits::Limits::resolve`：容器 `height(Length::Shrink)` + `max_height(n)` 时，高度 = `min(content_height, n)`，即「内容收缩 + 上限封顶」。
- 外层容器为 `Shrink`（设置 compression）时，内部 `file_tree::view` 的 `scrollable`（`height(Length::Fill)`）在 flex 压缩布局下解析为 `min(content, max)`（见 `iced_core/src/layout/flex.rs` 压缩分支），因此小树会收缩、大树在达到上限后滚动。

## 改动

### 1. `src/ui/components/torrent_file_list.rs`（唯一实质改动）
- 参数 `height: Length` 改名为 `max_height: Length`，语义为「最大高度」：
  - `Length::Fixed(n)` → 容器 `Shrink` + `.max_height(n)`（如 add 对话框上限 230px）
  - `Length::Fill` / `Length::Shrink` → 仅 `Shrink`，不设显式上限（由父容器在布局期封顶，如 details 面板固定 480px）
- 替换第 126 行的 `.height(if collapsed { Length::Shrink } else { height })`：

```rust
let mut outer = container(content).width(Length::Fill).height(Length::Shrink);
if let Length::Fixed(n) = max_height {
    outer = outer.max_height(n);
}
outer
    .padding(iced::Padding {
        top: SPACE_LG,
        right: SPACE_MD,
        bottom: if collapsed { SPACE_LG } else { PADDING_XS as f32 },
        left: SPACE_MD,
    })
    .style(theme::style::tree_frame)
    .into()
```

注意：两个调用方均按位置传参，无需修改调用代码；`collapsed` 分支继续用 `Shrink`（header 很小，max_height 无影响）。

### 2. 调用方（无需改动，仅语义变化）
- `src/ui/details_dialog.rs:372` 传 `Length::Fill` → 现在含义为「收缩到内容，上限为面板可用高度（面板 Fixed(480)）」。文件多时仍占满可用高度并滚动，文件少时紧凑。
- `src/ui/add_dialog.rs:424` 传 `Length::Fixed(230.0)` → 现在含义为「收缩到内容，上限 230px」。文件少时面板变小，文件多时仍封顶 230px 滚动。

## 验证
1. `cargo build` / `cargo check`
2. `cargo clippy --workspace`（无警告）
3. `cargo fmt --check`
4. 手动：
   - 添加对话框：1 个文件的种子 → 文件面板明显变小；几百个文件 → 高度仍为 230px 可滚动；收起/展开面板正常。
   - 详情对话框 Files 页：文件少的种子 → 列表紧凑、下方留白；文件多的种子 → 列表填满可用区域并可滚动。

## 风险
- 无显式上限时（details 的 `Length::Fill`），依赖父链有界高度（面板 Fixed(480)）防止树无限撑大 —— 已确认布局有界。
- `file_tree::view` 仅被 `torrent_file_list.rs` 引用，改动不波及其他页面。
