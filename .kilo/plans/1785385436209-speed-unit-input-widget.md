# Plan: 速度限制输入替换 — number_stepper + pick_list 内联

不创建独立组件文件。直接在 `settings_page.rs` 中用 `row![number_stepper, pick_list]` 替换 5 个速度限制的 `labeled_number`。

---

## 改动的文件

### 1. `src/message.rs`

添加：
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedUnit {
    Kbps,
    Mbps,
}
```

`Message` 新增变体：
```rust
SpeedUnitChanged(SettingKey, SpeedUnit),
```

### 2. `src/ui/settings_page.rs`

**`SettingsUiState`** 新增字段：
```rust
pub speed_units: HashMap<SettingKey, SpeedUnit>,
```
`new()` 中初始化为所有 5 个 key → `SpeedUnit::Kbps`。

**新增局部辅助函数**（非 pub，仅仅在这个文件内复用）：
```rust
fn speed_labeled_input<'a>(
    label: String,
    value_kb: &'a u64,
    unit: SpeedUnit,
    on_value: impl Fn(u64) -> Message + 'a,
    on_unit: impl Fn(SpeedUnit) -> Message + 'a,
) -> Element<'a, Message>
```

内部逻辑：
- 以 `SpeedUnit` 为泛型 `U`，`u64` 为泛型 `T`
- 构造 `UnitOption<SpeedUnit>` 列表（`Kbps → "KB/s"`, `Mbps → "MB/s"`）
- 根据 `unit` 决定 display 值和 step：
  - `Kbps`: `display = *value_kb`, `step = 100`
  - `Mbps`: `display = *value_kb / 1024`, `step = 1`
- 构造 `setting_row(label, row![number_stepper(display, ..), pick_list(..)])`
- `on_value` closure 交给 caller（caller 自己做 KB 转换）

**替换 5 处 `labeled_number`**：
1. `DownloadLimit` (`settings.download_limit_kb`)  
2. `UploadLimit` (`settings.upload_limit_kb`)  
3. `MaxDownloadLimit` (`settings.aria2.max_download_limit_kb`)  
4. `MaxUploadLimit` (`settings.aria2.max_upload_limit_kb`)  
5. `LowestSpeedLimit` (`settings.aria2.lowest_speed_limit_kb`)  

每一处：
```rust
let unit = settings_ui.speed_units.get(&SettingKey::DownloadLimit).copied().unwrap_or(SpeedUnit::Kbps);
speed_labeled_input(
    fluent.get(Tr::DownloadLimit),
    &settings.download_limit_kb,
    unit,
    move |v| Message::SettingChanged(SettingKey::DownloadLimit, v.to_string()),
    move |u| Message::SpeedUnitChanged(SettingKey::DownloadLimit, u),
)
```

注意：对于 `MaxDownloadLimit`、`MaxUploadLimit`、`LowestSpeedLimit`，它们的 `on_value` closure 需要捕获各自的 `SpeedUnit` 并在设置前做换算：

```rust
let unit = ...;
let key = SettingKey::MaxDownloadLimit;
speed_labeled_input(..., unit,
    move |v| {
        let kb = if unit == SpeedUnit::Kbps { v } else { v * 1024 };
        Message::SettingChanged(key, kb.to_string())
    },
    move |u| Message::SpeedUnitChanged(key, u),
)
```

同理 `MaxUploadLimit`、`LowestSpeedLimit`。

### 3. `src/app.rs`

新增 handler：
```rust
Message::SpeedUnitChanged(key, unit) => {
    state.settings_ui.speed_units.insert(key, unit);
}
```

---

## 不改的文件

- `config.rs`：字段名、序列化、单位转换全部不变
- `i18n.rs`：不新增翻译键
- `src/ui/components/`：无新增文件或模块
- `theme.rs`、`number_stepper.rs`：不动

---

## 验证

```bash
cargo clippy --workspace
cargo build
```

不得有新警告。
