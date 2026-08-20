//! Centralised UI dimension constants — font sizes, spacing, padding,
//! and a handful of fixed heights/widths. Constants here are the single
//! source of truth used by every page so visual rhythm stays consistent
//! across the app.

use iced::Padding;

// 尺寸常量：按像素值一一对应，常量名反映主要用途。所有值与原字面量完全一致。
// ---- 字体 / 图标字形大小 (px) ----
pub const FONT_HIDDEN: u32 = 1; // 零尺寸占位字形（resize_frame、title_bar 拖动区）
pub const FONT_TINY: u32 = 11; // 提示小字
pub const FONT_SMALL: u32 = 12; // 次要/元信息文本、行内小图标
pub const FONT_MEDIUM: u32 = 13; // 输入框、字段标签、菜单/排序项
pub const FONT_BODY: u32 = 14; // 按钮文字、对话框正文
pub const FONT_ICON: u32 = 15; // lucide 图标字形、任务名
pub const FONT_TITLE: u32 = 16; // 区块/对话框标题、toast 类型图标
pub const FONT_HERO: u32 = 18; // 空态标题、大图标（关闭按钮、速度 HUD）
pub const FONT_DIALOG_TITLE: u32 = 20; // 对话框标题、侧栏导航图标
pub const FONT_PAGE_TITLE: u32 = 22; // 设置页标题

// ---- 行列间距 (px) ----
pub const SPACE_NONE: f32 = 0.0; // 分组输入框行
pub const SPACE_XS: f32 = 2.0; // 图标簇、overlay 项
pub const SPACE_SCROLL: f32 = 3.0; // slim_scrollable 间距
pub const SPACE_SM: f32 = 4.0; // 紧凑分组
pub const SPACE_MD: f32 = 6.0; // 筛选/行内列
pub const SPACE_LG: f32 = 8.0; // 中等分组
pub const SPACE_XL: f32 = 10.0; // 按钮行、表单
pub const SPACE_2XL: f32 = 12.0; // 工具栏、操作行
pub const SPACE_3XL: f32 = 14.0; // 对话框正文
pub const SPACE_4XL: f32 = 16.0; // 区块间距

pub const SWATCH_SIZE: f32 = 28.0; // 主题色圆点尺寸
pub const SIDEBAR_LOGO_W: f32 = 40.0; // 侧栏 logo 宽
pub const SIDEBAR_LOGO_H: f32 = 24.0; // 侧栏 logo 高
pub const ABOUT_LOGO_SIZE: f32 = 96.0; // About 对话框 logo 边长（方形）

// ---- 内边距 ----
pub const PADDING_NONE: u16 = 0; // 铺满按钮（标题栏、侧栏导航）
pub const PADDING_XS: u16 = 2; // toast 关闭按钮
pub const PADDING_ICON_BTN: u16 = 4; // 任务卡片图标按钮
pub const PADDING_TOAST_CLOSE: u16 = 6; // toast 关闭按钮点击区
pub const PADDING_DROPDOWN: u16 = 6; // 下拉卡片、关闭按钮、路径历史浮层
pub const PADDING_EDITOR: u16 = 10; // URL 多行编辑器
pub const PADDING_CARD: u16 = 16; // 任务卡片
pub const PADDING_DETAILS: u16 = 20; // 详情面板
pub const PADDING_DIALOG: u16 = 20; // Dialog 容器
pub const PADDING_GROUPED: f32 = 1.0; // 分组控件（路径选择、数字步进）边框内缩
pub const SUB_ITEM_INDENT: f32 = 24.0; // 设置页子项左缩进
pub const PADDING_TOOLBAR_CAPSULE: [u16; 2] = [2, 6]; // 任务卡片工具栏胶囊
pub const PADDING_BUTTON_XS: [u16; 2] = [6, 8]; // 小菜单/工具栏按钮
pub const PADDING_BUTTON_SM: [u16; 2] = [6, 12]; // 设置引擎按钮
pub const PADDING_TAB: [u16; 2] = [6, 14]; // 详情页签
pub const PADDING_BUTTON_MD: [u16; 2] = [8, 18]; // 添加对话框按钮
pub const PADDING_FILTER: [u16; 2] = [10, 14]; // 分类栏筛选/设置项
pub const FILTER_ITEM_H: f32 = 36.0; // 分类栏筛选/设置项固定高度（滑动胶囊几何基准）
pub const FILTER_STEP: f32 = FILTER_ITEM_H + SPACE_MD; // 滑动胶囊每项步长
pub const PADDING_BUTTON_LG: [u16; 2] = [10, 22]; // 对话框操作按钮
pub const PADDING_BUTTON_XL: [u16; 2] = [10, 24]; // 设置主操作按钮
pub const ACTION_BUTTON_H: f32 = 36.0; // 设置页主操作按钮固定高度
pub const RESTART_ICON_BOX_FACTOR: f32 = 1.4; // 旋转所需最小方形盒（约 1.414×字形）
pub const PADDING_SIDEBAR_LOGO: [u16; 2] = [2, 0]; // 侧栏 logo
pub const PADDING_SIDEBAR: [u16; 2] = [12, 0]; // 侧栏容器
pub const PADDING_CATEGORY_BAR: [u16; 2] = [20, 14]; // 分类栏容器
pub const PADDING_PAGE: [u16; 2] = [24, 28]; // 页面容器
pub const PADDING_EMPTY_STATE: [u16; 2] = [80, 0]; // 空状态
pub const PADDING_BOTTOM_TOOLBAR: Padding = Padding {
    // 任务列表工具栏底部
    top: 0.0,
    right: 0.0,
    bottom: 12.0,
    left: 0.0,
};
pub const PADDING_BOTTOM_HEADER: Padding = Padding {
    // 详情对话框头底部
    top: 0.0,
    right: 0.0,
    bottom: 12.0,
    left: 0.0,
};
pub const PADDING_HUD: Padding = Padding {
    // 速度 HUD 胶囊
    top: 4.0,
    right: 12.0,
    bottom: 4.0,
    left: 0.0,
};
pub const PADDING_TOAST: Padding = Padding {
    // toast 卡片
    top: 10.0,
    right: 12.0,
    bottom: 10.0,
    left: 12.0,
};
