# 全局速度胶囊（右下角 HUD）+ 每任务上传速度

## 目标
1. **右下角速度胶囊**（固定、不随导航栏页面切换）：
   - **无速度**（download + upload 都为 0）：圆形胶囊，仅显示一个下载图标。
   - **有速度**：向左展开为 `[图标] | [块]`；块内上下两行——**上=上传速度（strong 色）**，**下=下载速度（primary 色）**；每行前缀一个小方向箭头图标区分方向。
   - 颜色跟随当前主题：胶囊背景 `background.base`，边框 `background.strong`，主图标与上传速度行文字用 `background.strong`，下载速度行文字用 `primary`。
2. **每任务上传速度**：在 `DownloadTask` / `EngineEvent::Progress` / 持久化中新增 `upload_speed` 字段，便于日后查看单个任务上传速度（本轮不新增 UI 展示，仅完成数据链路与存储）。

## 数据来源决策
- 现有 `EngineEvent::Progress` 只携带每任务 `download_speed`；全局上传速度无来源。
- 引擎新增每秒 `get_global_stat()` 轮询，发出 `EngineEvent::GlobalSpeed { download, upload }`（用户已确认）。
- 每任务上传速度：`aria2_ws::response::Status` 已含 `upload_speed`（response.rs:51），`emit_progress` 透传即可。

## 实施步骤

### 1. 引擎：事件与透传
- `src/engine.rs`：
  - `EngineEvent::Progress` 新增字段 `upload_speed: u64`。
  - 新增 `EngineEvent::GlobalSpeed { download: u64, upload: u64 }`。
  - `emit_progress`（engine.rs:290）补充 `upload_speed: s.upload_speed`。
  - `EngineCmd::Snapshot` 分支（engine.rs:453-464）发出的 `Progress` 同样补 `upload_speed: s.upload_speed`。
  - 在 1s 轮询任务（`on_sidecar_ready` 的 ticker，约 engine.rs:660-677）每次 tick 在 `tell_active` 之后调用 `poll_client.get_global_stat().await`，成功则 `event_tx.send(EngineEvent::GlobalSpeed { download: st.download_speed, upload: st.upload_speed })`；失败 `tracing::debug!` 后 continue。

### 2. 任务模型
- `src/task.rs`：`DownloadTask` 新增 `pub upload_speed: u64`。
  - 所有构造点初始化为 0（`app.rs` 的 `Added` 分支 engine.rs Added→app.rs:607，`db::load_all` 返回值）。

### 3. app.rs 状态更新
- `Remotrix` 新增字段 `global_speed: Option<(u64, u64)>`（`init` 置 `None`）。
- `update` 的 `Message::Engine` 分支：
  - `Progress { upload_speed, .. }`：写入 `t.upload_speed = upload_speed`（与现有 speed 同处）。
  - `GlobalSpeed { download, upload } => state.global_speed = Some((download, upload));`
  - `EngineStopped => state.global_speed = None;`
  - `Added`：构造 `DownloadTask` 时 `upload_speed: 0`。
- `FlushDirty` 批次元组携带 `upload_speed`（见步骤 4）。

### 4. 持久化（DB 迁移 + 落库）
- `src/db.rs`：
  - `CREATE TABLE` 语句增加列 `upload_speed INTEGER NOT NULL DEFAULT 0`。
  - 新增迁移：`open()` 在建表后执行
    ```sql
    ALTER TABLE tasks ADD COLUMN upload_speed INTEGER NOT NULL DEFAULT 0;
    ```
    用 `conn.execute_batch` + 忽略 "duplicate column" 错误（已有列即旧库已迁过）。实现时以 rusqlite 错误码判断或先 PRAGMA 检查列是否存在，避免 panic。
  - `load_all` SELECT 增列并在构造 `DownloadTask` 时回填 `upload_speed`。
  - `upsert_progress` / `flush` 的 UPDATE 语句增 `upload_speed=?`，参数顺序相应调整。
  - `flush` 签名改为 `flush(&self, dirty: &[(String, u64, u64, u64, u64, u64, String)])`（元组新增 upload_speed）。
- `app.rs::FlushDirty`：批次构造 `(gid, downloaded, total, speed, upload_speed, connections, status)`，与 db 签名对齐。

### 5. UI：胶囊组件
- 新建 `src/ui/speed_hud.rs`：
  - `pub fn view<'a>(theme: &iced::Theme, download: u64, upload: u64) -> Element<'a, Message>`。
- 折叠判定 `download == 0 && upload == 0` → 圆形容器（`width=height=44`）居中 download 主图标，主图标颜色 `background.strong`。
   - 展开 `row![ icon_col, block ]`：
     - `icon_col`：左块含 download 主图标，颜色 `background.strong`，宽 44。
     - `block = column!(up_row, down_row)`（**上传在上、下载在下**）：
       - `up_row = row![ arrow_up, text(format_speed(upload)) ]`，颜色 `background.strong`（图标同色）。
       - `down_row = row![ arrow_down, text(format_speed(download)) ]`，颜色 `primary`（图标同色）。
     - 整体容器圆角 `RADIUS_PILL`，padding 约 8x12。
  - 速度格式化复用 `crate::task::format_speed`；纯展示无 `on_press`，事件透传。

### 6. 图标
- `fonts/icons.toml` 新增上行箭头：
  ```toml
  arrow_up = "arrow-up-from-line"
  ```
  复用已有 `download_arrow`（"arrow-down-to-line"）作为下载行小箭头；主图标用已有 `download`。
  build 后 `src/ui/icon.rs` 自动生成 `arrow_up()`（已 `cargo::rerun-if-changed`）。

### 7. 顶层布局放置（固定、不随页面切换）
- `src/app.rs::view`：胶囊放在外层 `stack!` 中、紧跟 `framed`（base）之后、所有模态对话框之前。
- 构造定位容器锚定右下角：
  ```rust
  let (dl, up) = state.global_speed.unwrap_or((0, 0));
  let hud_overlay = container(speed_hud::view(t, dl, up))
      .width(Length::Fill).height(Length::Fill)
      .align_x(Horizontal::Right).align_y(Vertical::Bottom)
      .padding(Padding::from([0.0, 16.0, 16.0, 0.0]));
  let stacked = stack!(framed, hud_overlay); // 取代 `let stacked = framed;`
  ```
  - 定位容器 `Fill` 但内层 `speed_hud` 为 `Shrink` 且无 `mouse_interaction`，`iced stack` 不对其外区域 levitate cursor，事件透传到基础层、不拦截右侧点击。
  - 其余对话框 `stack![]` 依次 push 在 `stacked` 之上，确保弹窗覆盖胶囊。

### 8. 样式
- `src/ui/theme.rs` `style` 模块新增 `speed_hud_background`：
  ```rust
  pub fn speed_hud_background(t: &iced::Theme) -> iced::widget::container::Style {
      iced::widget::container::Style {
          background: Some(t.extended_palette().background.base.color.into()),
          border: iced::Border {
              color: t.extended_palette().background.strong.color,
              width: 1.0,
              radius: RADIUS_PILL.into(),
          },
          ..Default::default()
      }
  }
  ```
- 胶囊（折叠与展开共用）应用该样式；icon_col 与 block 不再单独设背景（继承外层）或仅视觉分隔时用透明容器。
- 颜色取值：主图标 / 上传行文字 / 上行小箭头 → `t.extended_palette().background.strong.color`；下载行文字 / 下行小箭头 → `primary(t)`。

## 风险 / 注意
- `get_global_stat` 在引擎降级/未连接时失败：仅 debug 日志、continue；UI 保持上次值或 `None`，可接受。
- DB 迁移：旧库无 `upload_speed` 列，`ALTER TABLE ADD COLUMN` 必须幂等（PRAGMA `table_info(tasks)` 检查列存在再 ADD，避免每次启动报错）。
- `FlushDirty` 元组与 `db::flush` 签名需严格对齐，避免编译错误。
- 每任务上传字段本轮不展示，仅数据链路；后续可在 `details_dialog` / `task_list` 复用 `task.upload_speed` 渲染。
- iced `container` 的 `padding`/`align` 参数以 iced 0.14 实际 API 为准，实现时核对。

## 验证
- `cargo fmt --check`
- `cargo clippy --workspace`（无 warning）
- `cargo build`（离线可构建）
- 运行观察：无任务/暂停时右下圆点；启动下载/BT 后胶囊左展开，**上=上传速度(strong)、下=下载速度(primary)**；切换 Tasks/Settings 页面胶囊位置不变；打开任意对话框时胶囊被对话框覆盖。
- 单个任务：`DownloadTask.upload_speed` 随 `Progress` 更新并落库（重启后 `load_all` 能读回）。