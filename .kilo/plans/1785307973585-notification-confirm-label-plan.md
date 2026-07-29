# 将 `notification-confirm` 改名为 `confirm`

## 背景
i18n key `notification-confirm` 当前值为:
- zh-CN: `通知与确认`
- en: `Notification & Confirm`

用户反馈:"通知"在此场景无意义,应显示"确认 / Confirm";并要求把 key 本身从 `notification-confirm` 改名为 `confirm`。

该 key 被两处复用(同一文案):
- `src/ui/confirm_dialog.rs:67` - 危险操作弹窗(删除全部 / 清空 / 移除任务)的确认按钮文字
- `src/ui/settings_page.rs:295` - 设置页一个分组的标题(该分组下仅"新建任务后跳转下载页"一个开关)

用户决定:两处统一显示"确认 / Confirm",key 改名为 `confirm`,概念整体重命名(含 Rust 枚举变体与调用点),保持代码与文案命名一致。

## 冲突核查(已确认无冲突)
- 无 `Tr::Confirm` 枚举变体(全仓 `.rs` 中无 `\bConfirm\b` 整词)。
- 无 `confirm =` 这个 ftl key(仅有 `confirm-close-title` 等带前缀的 key,不冲突)。
- `NotificationConfirm` 仅出现于 4 处:`src/i18n.rs:168`(枚举)、`src/i18n.rs:321`(match 分支)、`src/ui/confirm_dialog.rs:67`、`src/ui/settings_page.rs:295`。
- locale 文件只有 `zh-CN` 与 `en` 两份,无其他语言需同步。

## 改动清单
1. `i18n/locales/zh-CN/main.ftl` 第 53 行
   - `notification-confirm = 通知与确认` -> `confirm = 确认`
2. `i18n/locales/en/main.ftl` 第 53 行
   - `notification-confirm = Notification & Confirm` -> `confirm = Confirm`
3. `src/i18n.rs` 第 168 行(枚举变体)
   - `NotificationConfirm,` -> `Confirm,`
4. `src/i18n.rs` 第 321 行(match 分支)
   - `Tr::NotificationConfirm => "notification-confirm",` -> `Tr::Confirm => "confirm",`
5. `src/ui/confirm_dialog.rs` 第 67 行
   - `Tr::NotificationConfirm` -> `Tr::Confirm`
6. `src/ui/settings_page.rs` 第 295 行
   - `Tr::NotificationConfirm` -> `Tr::Confirm`

## 后果提示(供审阅)
按此方案,设置页那个分组标题也会显示为"确认 / Confirm"(因为两处复用同一 key)。这是用户明确要求的"全部改成确认"。若后续觉得分组标题叫"确认"不合适,可再拆分为独立 key--本轮不做。

## 不在范围内
- 不拆分弹窗按钮与设置分组标题为两个 key。
- 不调整设置页分组结构。

## 验证
- `cargo build` 确认编译通过(枚举与调用点同步改名,应无残留引用)。
- `cargo clippy --workspace` 无警告。
- 运行 `cargo run --`:
  - 触发危险操作弹窗(如"全部删除")确认按钮显示"确认"。
  - 进入"设置 -> 通用"确认该分组标题显示"确认"。
- 切换 zh-CN / en 两种语言各确认一次。
