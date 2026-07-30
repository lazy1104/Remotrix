# NumberStepper 组件：替代 iced_aw::NumberInput

## 背景与动机
现状：`settings_page.rs:605` 的 `labeled_number` 包 `iced_aw::NumberInput::new(value, bounds, on_change).step(s)`，13 处调用；`add_dialog.rs:104` 的 split 输入用裸 `text_input` + `Message::SplitChanged`。
目标：新增一个同 `PathPicker` 样式的数字组件（输入框 + 减号图标 + 加号图标），替换现有数字输入；可选只读（只读时仅 hover 边框、不可输入、± 不可点击）。

## 关键决策（已与用户确认）
1. **复用 iced 的 `text_input`**，不重写输入框（光标/键盘/文字绘制仍由 `text_input` 负责）。
2. **外层焦点边框走自定义 widget**：从子 `text_input` 的 Tree 状态经 `operation::Focusable::is_focused()` 精确读取真实键盘焦点，驱动外层统一边框颜色（与 iced 内部判定方式一致）。不靠 `mouse_area` 合成 focus。
3. **本地编辑缓冲**：自定义 widget 的 Tree State 持有 `buffer: String` + `focused: bool`。focus 时 text_input 绑 buffer（可中途输入半截非法/空值）；on_input 实时同步 buffer 且 emit 原始文本；blur 时按范围 clamp、把 buffer 回填为合法字符串并 emit 最终值。体验对齐 iced_aw::NumberInput。
4. **只读模式**：`text_input` 无 `on_input`（其 Status 恒为 Disabled，不响应键盘）；± 按钮不带 `on_press`；仅外层 hover 由 `mouse_area::on_enter/on_exit` 合成（只读无需精确 focus，连 PathPicker 的 read_only 一致）。

> 注意：iced 0.14 的 `text_input` **没有 on_focus/on_blur**，故自定义 widget 用 Operation 探测，而非消息。

## 受影响边界
- 新增：`src/ui/components/number_stepper.rs`
- 改：`src/ui/components/mod.rs`（注册模块）
- 改：`src/ui/settings_page.rs`（`labeled_number` 改用新组件；导入调整）
- 不改：`src/app.rs` 的 `SettingChanged` 分支（仍收 `String`，parse 失败回退旧值，天然兼容中间非法输入）
- `add_dialog.rs` 的 split 输入：本计划范围内**一并替换**为新组件（只读=false），消息仍走 `Message::SplitChanged` —— 见下。

## API 设计

### 公开构造（声明式，贴近 iced_aw 用法）
```rust
pub struct NumberStepper<'a, T, M> {
    value: &'a T,
    bounds: (T, T),      // min..=max 拍平
    step: T,
    on_change: Box<dyn Fn(T) -> M + 'a>,
    read_only: bool,
    // 占位/theme 不暴露，组件内自行取 theme
}
```
因 `RangeBounds` 难以泛型存储，构造时分理 `start`/`end`：
```rust
pub fn number_stepper<'a, T, M>(
    value: &'a T,
    bounds: impl std::ops::RangeBounds<T>,
    step: T,
    on_change: impl Fn(T) -> M + 'a,
) -> NumberStepper<'a, T, M>
```
（为 `add_dialog` 的 `SplitChanged(String)` 提供 `number_stepper_map`：`on_change` 内把 `T` 转 `String` 再包消息。）

泛型约束（沿用 `labeled_number`）：
```rust
T: num_traits::Num + num_traits::NumAssignOps + PartialOrd
  + std::fmt::Display + std::str::FromStr + Clone + Copy + 'static,
T::Err: std::fmt::Debug,
```

### Tree State（自定义 widget 内部）
```rust
#[derive(Default)]
struct State { buffer: String, focused: bool, hovered: bool }
```
- `new(state)`：`buffer = value.to_string()`，focused/hovered=false。
- `diff`：若 `!focused`（外部值变更或首次），把 `buffer` 同步到 `value.to_string()`。
- `operate`：转发给子节点 `text_input`（保证外部 `focus_next` 等正常作用到输入框）。

### focus 探测（核心，无消息机制）
在 `update` 末尾，对子 widget Tree 跑一个自定义 Operation：
```rust
struct FocusProbe { focused: bool }   // Operation<()>
impl Operation for FocusProbe {
    fn focusable(&mut self, _id, _bounds, state: &mut dyn Focusable) {
        if state.is_focused() { self.focused = true; }
    }
    fn traverse(&mut self, op: &mut dyn FnMut(&mut dyn Operation)) { op(self); }
}
```
- 跑完后对比 `state.focused` 与 probe：发生 `false→true` 时 `state.focused = true`、同步 `buffer = value.to_string()`（避免显示上次 blur 前的脏值）；发生 `true→false` 时执行 **blur-clamp**：
  1. 用 `value`（外部已 parse 成功的当前值）回算 `buffer`：若 `buffer.parse()` 落在 bounds 内 → 取该值；否则回退 `*value`。
  2. `buffer = clamped.to_string()`。
  3. `shell.publish(on_change(clamped))` 保证最终落值（含外部以 `unwrap_or` 兜底也无副作用）。
- `state.focused` 用于绘制外层边框（accent）。

### hover（两模式共用）
只读与非只读都用 `mouse_area::on_enter/on_exit` 合成 `state.hovered`（自定义 widget 本身也能读 `cursor.is_over(bounds)`，但沿用 PathPicker 的合成方式更一致、只读也适用）。实现：在 `update` 里调子 `mouse_area`?——更简单：**自定义 widget 直接读 cursor**：`state.hovered = cursor.is_over(layout.bounds())`，每帧更新，无需 mouse_area 与 hover 消息。无 `mousemove` 消息成本。

### 绘制
- `children` 返回列：`[text_input, minus_button, plus_button]`（只读时仍渲染，但不带 on_press/text_input 无 on_input）。
- `layout`：用 `iced::advanced::layout` 按 `row` 的方式排（与 `PathPicker::view` 的 row 结构一致：input Fill + 两按钮固定 36 高）。可复用 `iced::widget::row` + `Element` 组合成单个子，再只画外层 quad —— **推荐做法**：children 只放一个 `row` Element（内部含 text_input+button+button），自定义 widget 仅负责外层框与焦点探测。这样 layout 直接委托 row，draw 在 row 之下先 `fill_quad` 画框再画 row。
- 外框样式：复用 `theme::style::grouped_frame_state(focused, hovered)`（已存在，`theme.rs:131`），但它是闭包 `Fn(&Theme)->container::Style`；在 `fill_quad` 处手动取其 `(background, border.color, border.width, border.radius)`。padding 1.0（沿用 PathPicker 的 `grouped_frame` 防按钮压框做法）。
- 子按钮样式：复用 `theme::style::button::grouped_icon(false/true)`、`icon::minus()` / `icon::plus()`，与 PathPicker 完全同款。
- text_input 样式：复用 `theme::style::input::grouped`，`width(Fill)`、`padding([0,10])`、`size(13)`，与 PathPicker 的 input 段一致。
- 分隔线：复用 `PathPicker::separator` 的做法（width 1.0、`theme::style::separator`）；为不暴露 PathPicker 私有 fn，在 number_stepper 内复制该 4 行 helper。

### on_change 路由
- settings：`NumberStepper` 直接 emit `Message::SettingChanged(key, v.to_string())`（与旧 `iced_aw::NumberInput::new` 的 `on_change` 完全一致，`labeled_number` 改造零行为差异）。
- add_dialog split：emit `Message::SplitChanged(v.to_string())`（`app.rs:260` 已 `parse::<u16>().max(1)`，兼容）。

### 只读
构造额外方法 `number_stepper_read_only(value)`：内部 `on_change` 给 `|_| unreachable!()` 占位（因只读从不触发）。view 内：text_input 无 `.on_input`；± 按钮无 `.on_press` 且 `grouped_icon(true)` 的 disabled 态即可。focus 探测照跑（Disabled 下恒 false，边框只随 hover）。

## Task 列表

1. **`src/ui/components/mod.rs`**：`pub mod number_stepper;`
2. **`src/ui/components/number_stepper.rs`**：实现
   - `NumberStepper<'a,T,M>` + `State { buffer, focused, hovered }`
   - 构造 `number_stepper(value, bounds, step, on_change)`、`number_stepper_read_only(value)`
   - `impl Widget`：`tag/state/diff/children/size/layout/operate/update/mouse_interaction/draw/overlay`
   - `focus_probe` Operation
   - 内部拼装 `iced::widget::row`（text_input + separator + minus btn + separator + plus btn），外层 `fill_quad` 用 `grouped_frame_state`
   - clamp helper：`fn clamp(v, (lo,hi)) -> Option<T>`（处理 `RangeBounds` 归一）
3. **`src/ui/settings_page.rs`**：
   - 导入新组件
   - `labeled_number` 内把 `iced_aw::NumberInput::new(...).step(step).width(Fixed(160.0))` 换成 `number_stepper(value, bounds, step, move |v| Message::SettingChanged(key, v.to_string()))`，外层 `.width(Length::Fixed(160.0))` 保持
   - 13 个调用点无需改动（签名不变）
4. **`src/ui/add_dialog.rs`**：
   - split 输入 `text_input("", split_str).on_input(SplitChanged)`（`add_dialog.rs:104` 区段）替换为 `number_stepper(&state.split, 1..=128u16, 1, |v| Message::SplitChanged(v.to_string())).width(Length::Fixed(80.0))`，去掉 `split_str` 与该 row 的手动拼装
   - 保留 label + `Space::Fill` 布局
5. **`Cargo.toml`**：`iced_aw` 仅剩 `drop_down` feature —— 改 `features = ["drop_down"]`，移除 `number_input`。**需先确认无其它 `iced_aw` number_input 使用**（grep 已确认仅 `settings_page.rs:625`）。若担心，先保留 feature 再裁；本步可最后做。
6. **校验**
   - `cargo build`
   - `cargo clippy --workspace`（无 warning）
   - `cargo fmt --check`
   - 人工（下条交由用户实测）：
     - settings 各数字字段键盘可编辑、中途清空不瞬回、失焦回填合法；± 步进按 bounds 限制；focus 时整组边框变 accent、hover 变 secondary、idle 变 border；只读字段只 hover、不可输入/点击。

## 风险与边界情形
- **`RangeBounds` 拍平**：现存调用均为 `a..=b`（types 通用 RangeInclusive）。`number_stepper` 中用 `bounds.start_bound()/end_bound()` 取 `Included(v)`/`Unbounded`，转 `(T::MIN? …)`。**现存 13 处全是 `a..=b`**，故实现只需支持 Inclusive 两端（其余 unbounded 运行时 panic 或用 `T: Bounded` 的 min/max 兜底）。`labeled_number` 的 `T: num_traits::Bounded` 约束已存在 → unbounded 用 `T::min_value()/max_value()`。
- **blur-clamp 时机**：自定义 Operation 在 `update` 内对子 Tree 跑 —— 注意 `update` 每帧都跑会增加开销；用 `shell.is_event_captured()` 后才跑 focus probe 减少? 实际 Operation 很轻（仅读 bool），直跑可接受。
- **只读 + focus**：只读输入无 on_input，其 Focusable `is_focused` 恒 false；probe 安全。
- **`diff` 同步 buffer**：外部值因 emit 而变（如点 ±）时，若当前未 focus，buffer 跟随更新；若 focused 且 buffer 已被用户改脏，不覆盖（保持编辑中状态）—— 通过 `state.focused` 判断。
- **add_dialog split 初始值**：`AddDialogState::split` 为 `u16`，范围 `1..=128`，`default_split` 来自 settings.split（u16）。已对齐。