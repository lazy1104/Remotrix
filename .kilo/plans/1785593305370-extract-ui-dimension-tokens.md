# 提取 UI 尺寸常量（字体大小 / spacing / padding）

## 目标
把散落在 20 个 UI 文件中的约 160 处硬编码 `.size(N)`、`.spacing(N)`、`.padding(...)` 字面量统一提取到新的 `src/ui/dims.rs` 常量模块，避免每次单独填值，后续可一处调整整体排版。**不改变任何数值，无视觉变化。**

## 现状与先例
- `theme.rs` 已有 `RADIUS_*`、`INPUT_SIZE`(u32=13)、`INPUT_PADDING`、`INPUT_PADDING_GROUPED` 及 builder 辅助函数 `input_layout`/`grouped_input_layout`/`editor_layout`（先例 1785486468424）。
- 局部常量保持不动：`icons.rs` 的 `SIDEBAR_W`/`CATEGORY_W`、`components/mod.rs` 的 `CONTROL_HEIGHT`、`resize_frame.rs` 的 `BORDER`、`toast.rs` 的 `CARD_WIDTH`/`OVERLAY_PADDING`、`piece_map.rs` 的 `CELL_SIZE` 等。

## 类型约束（已核对 iced 0.14 / iced_widget 0.14.2 源码）
- `.size()` 与 `.spacing()` 均接受 `impl Into<Pixels>`，而 `Pixels` 仅实现 `From<f32>` 与 `From<u32>` → **字体常量用 `u32`，spacing 常量用 `f32`**（不要用 u16，编译不过；现有 `INPUT_SIZE` 就是 u32）。
- `.padding()` 接受 `impl Into<Padding>`：`u16`、`[u16; 2]`、`f32`、`iced::Padding` 均可。
- `iced::Padding::bottom()` 不是 `const fn` → 仅底部留白的常量必须用 struct literal 初始化。

## 实施步骤

### 1. 新建 `src/ui/dims.rs`
```rust
use iced::Padding;

// 尺寸常量：按像素值一一对应，常量名反映主要用途。所有值与原字面量完全一致。
// ---- 字体 / 图标字形大小 (px) ----
pub const FONT_HIDDEN: u32 = 1;          // 零尺寸占位字形（resize_frame、title_bar 拖动区）
pub const FONT_TINY: u32 = 11;           // 提示小字
pub const FONT_SMALL: u32 = 12;          // 次要/元信息文本、行内小图标
pub const FONT_MEDIUM: u32 = 13;         // 输入框、字段标签、菜单/排序项
pub const FONT_BODY: u32 = 14;           // 按钮文字、对话框正文
pub const FONT_ICON: u32 = 15;           // lucide 图标字形、任务名
pub const FONT_TITLE: u32 = 16;          // 区块/对话框标题、toast 类型图标
pub const FONT_HERO: u32 = 18;           // 空态标题、大图标（关闭按钮、速度 HUD）
pub const FONT_DIALOG_TITLE: u32 = 20;   // 对话框标题、侧栏导航图标
pub const FONT_PAGE_TITLE: u32 = 22;     // 设置页标题

// ---- 行列间距 (px) ----
pub const SPACE_NONE: f32 = 0.0;         // 分组输入框行
pub const SPACE_XS: f32 = 2.0;           // 图标簇、overlay 项
pub const SPACE_SCROLL: f32 = 3.0;       // slim_scrollable 间距
pub const SPACE_SM: f32 = 4.0;           // 紧凑分组
pub const SPACE_MD: f32 = 6.0;           // 筛选/行内列
pub const SPACE_LG: f32 = 8.0;           // 中等分组
pub const SPACE_XL: f32 = 10.0;          // 按钮行、表单
pub const SPACE_2XL: f32 = 12.0;         // 工具栏、操作行
pub const SPACE_3XL: f32 = 14.0;         // 对话框正文
pub const SPACE_4XL: f32 = 16.0;         // 区块间距

// ---- 内边距 ----
pub const PADDING_NONE: u16 = 0;         // 铺满按钮（标题栏、侧栏导航）
pub const PADDING_XS: u16 = 2;           // toast 关闭按钮
pub const PADDING_ICON_BTN: u16 = 4;     // 任务卡片图标按钮
pub const PADDING_DROPDOWN: u16 = 6;     // 下拉卡片、关闭按钮、路径历史浮层
pub const PADDING_EDITOR: u16 = 10;      // URL 多行编辑器
pub const PADDING_CARD: u16 = 16;        // 任务卡片
pub const PADDING_DETAILS: u16 = 20;     // 详情面板
pub const PADDING_DIALOG: u16 = 28;      // Dialog 容器
pub const PADDING_GROUPED: f32 = 1.0;    // 分组控件（路径选择、数字步进）边框内缩
pub const PADDING_TOOLBAR_CAPSULE: [u16; 2] = [2, 6];   // 任务卡片工具栏胶囊
pub const PADDING_BUTTON_XS: [u16; 2] = [6, 8];         // 小菜单/工具栏按钮
pub const PADDING_BUTTON_SM: [u16; 2] = [6, 12];        // 设置引擎按钮
pub const PADDING_TAB: [u16; 2] = [6, 14];              // 详情页签
pub const PADDING_BUTTON_MD: [u16; 2] = [8, 18];        // 添加对话框按钮
pub const PADDING_TRAY: [u16; 2] = [8, 22];             // 关闭对话框托盘按钮
pub const PADDING_FILTER: [u16; 2] = [10, 14];          // 分类栏筛选/设置项
pub const PADDING_BUTTON_LG: [u16; 2] = [10, 22];       // 对话框操作按钮
pub const PADDING_BUTTON_XL: [u16; 2] = [10, 24];       // 设置主操作按钮
pub const PADDING_SIDEBAR_LOGO: [u16; 2] = [8, 0];      // 侧栏 logo
pub const PADDING_SIDEBAR: [u16; 2] = [12, 0];          // 侧栏容器
pub const PADDING_CATEGORY_BAR: [u16; 2] = [20, 14];    // 分类栏容器
pub const PADDING_PAGE: [u16; 2] = [24, 28];            // 页面容器
pub const PADDING_EMPTY_STATE: [u16; 2] = [80, 0];      // 空状态
pub const PADDING_BOTTOM_TOOLBAR: Padding = Padding {   // 任务列表工具栏底部
    top: 0.0, right: 0.0, bottom: 12.0, left: 0.0,
};
pub const PADDING_BOTTOM_HEADER: Padding = Padding {    // 详情对话框头底部
    top: 0.0, right: 0.0, bottom: 12.0, left: 0.0,
};
pub const PADDING_HUD: Padding = Padding {              // 速度 HUD 胶囊
    top: 8.0, right: 12.0, bottom: 8.0, left: 12.0,
};
pub const PADDING_TOAST: Padding = Padding {            // toast 卡片
    top: 10.0, right: 12.0, bottom: 10.0, left: 12.0,
};
```

### 2. `src/ui/mod.rs` 注册模块
在模块列表中加入 `pub mod dims;`（按字母序放在 `components` 之后、`confirm_dialog` 之前）。

### 3. `src/ui/theme.rs` — INPUT_SIZE 统一到 FONT_MEDIUM
- 顶部加 `use crate::ui::dims;`
- `input_layout` / `grouped_input_layout` / `editor_layout` 三处的 `.size(INPUT_SIZE)` 改为 `.size(dims::FONT_MEDIUM)`
- 删除 `pub const INPUT_SIZE: u32 = 13;`
- 保留 `INPUT_PADDING` / `INPUT_PADDING_GROUPED`（输入框专用，仅在此使用）

### 4. 逐文件替换（值 → 常量，一一对应）
每个用到常量的文件顶部加 `use crate::ui::dims::*;`（glob；调用点多，`dims::` 前缀会过长。clippy 默认不启用 `wildcard_imports`，安全）。替换后删除原 `[6_u16, 8]` 等处的 `_u16` 后缀。

**字体大小 `.size(N)`：**
| 值 | 常量 | 调用点 |
|---|---|---|
| 1 | `FONT_HIDDEN` | resize_frame.rs:14,51; title_bar.rs:14,26,85 |
| 11 | `FONT_TINY` | add_dialog.rs:199; close_dialog.rs:15; details_dialog.rs:380 |
| 12 | `FONT_SMALL` | 约 38 处：task_list.rs(185 除外全部 12)、settings_page.rs:743,747,764,784,581、details_dialog.rs 各 12、speed_hud.rs:30,31,37,38、path_picker.rs:235、add_dialog.rs:140,155,165,188,281、about_dialog 无、close_dialog 无 |
| 13 | `FONT_MEDIUM` | 约 22 处：task_list.rs:61,80,185、settings_page.rs:260,525,542,693,698,838,912,955、details_dialog.rs:83,102,141,147,364,367、confirm_dialog.rs:24、close_dialog.rs:11、about_dialog.rs:28,33、toast.rs:141、path_picker.rs 无 |
| 14 | `FONT_BODY` | 约 24 处：各对话框按钮/正文（confirm_dialog、close_dialog、about_dialog、add_dialog.rs:133,234,240、details_dialog.rs:98,288,392、settings_page.rs:116,127、toast.rs:148） |
| 15 | `FONT_ICON` | 约 25 处：task_list.rs:27,43,96,120,225,248,252,255,259,269,277,287,295,307、category_bar.rs:50,101、title_bar.rs:37,55,68、path_picker.rs:168,188,202、number_stepper.rs:168,185、icons.rs:12 |
| 16 | `FONT_TITLE` | category_bar.rs:28; details_dialog.rs:65; add_dialog.rs:328; settings_page.rs:1037; toast.rs:137 |
| 18 | `FONT_HERO` | task_list.rs:179; dialog.rs:107; details_dialog.rs:60; speed_hud.rs:20,27; icons.rs:8 |
| 20 | `FONT_DIALOG_TITLE` | dialog.rs:103; sidebar.rs:28 |
| 22 | `FONT_PAGE_TITLE` | settings_page.rs:110 |

**行列间距 `.spacing(N)`：**
| 值 | 常量 | 调用点 |
|---|---|---|
| 0 | `SPACE_NONE` | path_picker.rs:154; number_stepper.rs:162 |
| 2 | `SPACE_XS` | task_list.rs:67,327; details_dialog.rs:358; path_picker.rs:247; close_dialog.rs:36; speed_hud.rs:43 |
| 3 | `SPACE_SCROLL` | slim_scrollable.rs:18 |
| 4 | `SPACE_SM` | 21 处（task_list、details_dialog、settings_page、category_bar、add_dialog、sidebar、speed_hud 各 4） |
| 6 | `SPACE_MD` | details_dialog.rs:182,334,386; category_bar.rs:70,120; add_dialog.rs:202 |
| 8 | `SPACE_LG` | 13 处（details、task_list、settings、category_bar、path_picker、speed_hud、toast） |
| 10 | `SPACE_XL` | confirm_dialog.rs:44,59,70,85; add_dialog.rs:248,352; close_dialog.rs:46 |
| 12 | `SPACE_2XL` | details_dialog.rs:296; task_list.rs:390; settings_page.rs:114,750,795 |
| 14 | `SPACE_3XL` | add_dialog.rs:229,254（含 `Dialog::new().spacing(14.0)`，f32 直接可用） |
| 16 | `SPACE_4XL` | about_dialog.rs:20; details_dialog.rs:95; category_bar.rs:142 |

**内边距 `.padding(...)`：**
| 原字面量 | 常量 | 调用点 |
|---|---|---|
| `0` | `PADDING_NONE` | sidebar.rs:37; title_bar.rs:44,62,75 |
| `2` | `PADDING_XS` | toast.rs:150 |
| `4` | `PADDING_ICON_BTN` | task_list.rs:237,240,263,271,281,289,299,313 |
| `6` | `PADDING_DROPDOWN` | task_list.rs:88; details_dialog.rs:62; dialog.rs:109; path_picker.rs:251 |
| `10` | `PADDING_EDITOR` | add_dialog.rs:132 |
| `16` | `PADDING_CARD` | task_list.rs:398 |
| `20` | `PADDING_DETAILS` | details_dialog.rs:131 |
| `28` | `PADDING_DIALOG` | dialog.rs:89 |
| `1.0` | `PADDING_GROUPED` | path_picker.rs:224; number_stepper.rs:345（`Padding::new(1.0)`）与 :348（`Point::new(1.0, 1.0)`） |
| `[6, 8]` / `[6_u16, 8]` | `PADDING_BUTTON_XS` | task_list.rs:35,51,64,83,99,122; path_picker.rs:240 |
| `[6, 12]` | `PADDING_BUTTON_SM` | settings_page.rs:756,771,778 |
| `[6, 14]` | `PADDING_TAB` | details_dialog.rs:85,104 |
| `[8, 18]` | `PADDING_BUTTON_MD` | add_dialog.rs:236,241 |
| `[8, 22]` | `PADDING_TRAY` | close_dialog.rs:39 |
| `[10, 14]` | `PADDING_FILTER` | category_bar.rs:58,108 |
| `[10, 22]` | `PADDING_BUTTON_LG` | confirm_dialog.rs:29,36,40,51,55,66,77,81; close_dialog.rs:20,25; about_dialog.rs:39 |
| `[10, 24]` | `PADDING_BUTTON_XL` | settings_page.rs:122,129 |
| `[8, 0]` | `PADDING_SIDEBAR_LOGO` | sidebar.rs:22 |
| `[12, 0]` | `PADDING_SIDEBAR` | sidebar.rs:80 |
| `[20, 14]` | `PADDING_CATEGORY_BAR` | category_bar.rs:149 |
| `[24, 28]` / `[24_u16, 28]` | `PADDING_PAGE` | task_list.rs:197,212; settings_page.rs:138 |
| `[80, 0]` / `[80_u16, 0]` | `PADDING_EMPTY_STATE` | task_list.rs:192 |
| `[2, 6]` | `PADDING_TOOLBAR_CAPSULE` | task_list.rs:330 |
| `Padding::new(0.0).bottom(12.0)` | `PADDING_BOTTOM_TOOLBAR` | task_list.rs:170 |
| `Padding::new(0.0).bottom(12.0)` | `PADDING_BOTTOM_HEADER` | details_dialog.rs:72 |
| `Padding{8,12,8,12}` | `PADDING_HUD` | speed_hud.rs:50 |
| `Padding{10,12,10,12}` | `PADDING_TOAST` | toast.rs:158 |

### 5. 校验
```bash
cargo fmt --check
cargo clippy --workspace   # 不允许警告
cargo build
```

## 保留不动（明确 out of scope）
- `theme.rs` 的 `INPUT_PADDING` / `INPUT_PADDING_GROUPED`（输入框专用）。
- 局部常量：`CARD_WIDTH`、`OVERLAY_PADDING`(toast)、`CONTROL_HEIGHT`、`SIDEBAR_W`、`CATEGORY_W`、`BORDER`(resize)、`CELL_SIZE`/`CELL_GAP`(piece_map)、`BAR_HEIGHT`。
- app.rs:1689 速度 HUD 角标偏移 `Padding{0,16,20,0}` —— 定位专用，保留内联。
- slim_scrollable.rs:13 的 `iced::padding::bottom(5.0)` —— 滚动底部留白，保留内联。
- 宽度/高度类字面量（`Length::Fixed(200.0)` 标签宽、`Length::Fixed(160.0)` 步进器宽、`Space::new().height(...)` 等）不在本次范围。

## 风险与注意
- **类型**：字体用 `u32`、spacing 用 `f32`、padding 按上表（u16 / `[u16; 2]` / `f32` / `Padding`），混用会编译失败。
- `FONT_*` 命名按像素值一一对应（"value-keyed"），个别处语义略有出入（如任务名 `.size(15)` → `FONT_ICON`、toast 图标 `.size(16)` → `FONT_TITLE`），按值替换即可，不要自行改值。
- 用常量替换后，`[6_u16, 8]` 等 `_u16` 后缀必须删除。
- `Dialog::new().spacing(14.0)` 参数类型是 `f32`，spacing 常量用 f32 直接匹配。
- 建议每改完一个文件即 `cargo check`，最后统一跑第 5 步全量校验。
