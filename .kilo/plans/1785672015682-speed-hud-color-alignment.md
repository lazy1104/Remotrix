# Speed HUD — 颜色改用 primary.weak + 悬停边框（改 button 样式）

目标文件：`src/ui/components/speed_hud.rs`、`src/ui/theme.rs`。

## 背景
- 上次将 HUD 图标/上传/边框改为 `background.base.text`，浅色主题下接近纯黑，过重。
- 新方向：图标、上传速度 → `primary.weak`；边框 → `background.strong`（回到原有）；鼠标移入时边框 → `primary.weak`。
- `container` 无法感知悬停，已确认改用 **button 样式** 方案。
- 已存在 `Message::Noop`（`src/message.rs:146`），在 `src/app.rs:1999` 无操作处理，可直接作为 `on_press`。

## 颜色决策
- 折叠图标（line 21）：`primary.weak`
- 展开大图标（line 27）：`primary.weak`
- 上传行（line 31-32，arrow_up + 文本）：`primary.weak`
- 下载行（line 38-39，download_arrow + 文本）：保持 `primary`（未要求改）
- 边框：普通态 `background.strong`；Hovered 态 `primary.weak`
- 背景：保持 `background.base`，半径 `RADIUS_PILL`

## 改动步骤

### 1. `src/ui/theme.rs` — 新增 button 样式，移除容器样式
- 移除 `style::speed_hud_background`（`src/ui/theme.rs:357`），改为在 `style::button` 模块内新增：
  ```rust
  pub fn speed_hud<'a>() -> impl Fn(&iced::Theme, Status) -> Style + 'a {
      move |t: &iced::Theme, status: Status| -> Style {
          let p = t.extended_palette();
          let border = match status {
              Status::Hovered => p.primary.weak.color,
              _ => p.background.strong.color,
          };
          Style {
              background: Some(p.background.base.color.into()),
              text_color: p.background.base.text,
              border: iced::Border {
                  color: border,
                  width: 1.0,
                  radius: iced::border::rounded(super::super::RADIUS_PILL).radius,
              },
              shadow: Shadow::default(),
              ..Default::default()
          }
      }
  }
  ```
- 注意 `Shadow`/`Status`/`Style` 已在 `style::button` 模块内导入，无需新增。

### 2. `src/ui/components/speed_hud.rs` — 容器改 button + 换色
- 导入 `button`（改用 `iced::widget::button`），移除 `container` 的容器样式用法。
- `view` 签名保持不变，返回值仍为 `Element<'a, Message>`。
- 折叠分支（原 line 20-25）：
  ```rust
  button(
      container(icon::download().size(FONT_HERO).color(primary_weak))
          .center_x(Length::Fill)
          .center_y(Length::Fill)
          .width(Length::Fill)
          .height(Length::Fill),
  )
  .on_press(Message::Noop)
  .padding(iced::Padding::ZERO)
  .width(Length::Fixed(44.0))
  .height(Length::Fixed(44.0))
  .style(theme::style::button::speed_hud())
  .into()
  ```
  - **重要**：`button` 的布局用 `layout::padded`（`iced_core/src/layout.rs:170`），子元素定位在内容盒左上角，**不会居中**（与 `container` 不同）。所以必须把图标包一层 `container(...).center_x(Fill).center_y(Fill).width(Fill).height(Fill)`，使其铺满 44×44 的 button 内容盒并居中，等价于原有 `center_x/center_y(Fixed(44))`。
  - `container` 已在 speed_hud.rs 导入（用于 `icon_col`），无需新增。
- 展开分支：
  - `icon_col` 大图标 `.color(primary_weak)`（保持 `center_x(Fixed(44))`，配合 `PADDING_HUD.left=0` 左内缩与折叠一致）。
  - `up_row` 的 arrow_up 与文本 `.color(primary_weak)`。
  - `down_row` 保持 `.color(primary)` 不变。
  - 外层容器改为 `button(content).on_press(Message::Noop).padding(PADDING_HUD).style(theme::style::button::speed_hud()).into()`。
- 颜色来源：在函数开头用 `let primary_weak = palette.primary.weak.color;`，并移除临时 `text_fg`。

### 3. 保持不变
- `PADDING_HUD`（`src/ui/dims.rs`，left=0）不改，保证展开态图标左对齐。
- `src/app.rs:2136` 外层包裹 container 无需改动（button 作为其子元素被定位）。

## 风险 / 注意
- button 会捕获 HUD 区域内的点击事件（触发 `Message::Noop`，无实际副作用）；HUD 位于右下角覆盖层，点击原无意义，行为可接受。
- `primary.weak` 为弱化强调色，浅/深主题下均为低饱和强调色调，不会过黑——符合要求。
- 需确认 `Message::Noop` 已存在且 `app.rs:1999` 有对应处理（已核实），无需改 message.rs/app.rs。
- **回归点**：上一轮把折叠态从 `container` 改为裸 `button` 后，图标因 button 不居中而偏左上。本计划的折叠分支必须包一层居中 container，否则问题依旧。

## 验证
- `cargo build`、`cargo clippy --workspace`、`cargo fmt --check` 全部通过（无 warning）。
- 运行：明/暗主题下图标、上传速度呈 `primary.weak` 色调；边框普通态 `background.strong`，鼠标移入变 `primary.weak`；折叠/展开图标左对齐一致。
- **折叠态回归验证**：空闲（无活动任务）时折叠胶囊内的下载图标应水平/垂直居中（四边等距），且悬停仍触发边框变色。
- 移除 `speed_hud_background` 容器样式后确认无残留引用（原仅 speed_hud.rs 引用）。
