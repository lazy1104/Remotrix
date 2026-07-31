# Min Split Size: plain number input in MiB

Convert the `MinSplitSize` setting from a free-text field (aria2 raw string like `"1M"`) into a plain number stepper in MiB.

## Constraint verified in aria2-next source

`src/OptionHandlerFactory.cc`:
```cpp
new UnitNumberOptionHandler(PREF_MIN_SPLIT_SIZE, TEXT_MIN_SPLIT_SIZE, "20M", 1_m, 1_g, 'k')
```
`min-split-size` accepts only `[1M, 1G]` — minimum 1 MiB, maximum 1 GiB (1024 MiB). Values outside are rejected by aria2. This is why no GB unit is possible and a fixed M unit is the right UI.

## Storage decision

Store the value directly in MiB as `min_split_size_mb: u64` (default `1`). The stepper displays the stored value directly (no unit conversion), so no derived-value/`Box::leak` workaround is needed (unlike the speed-limit inputs).

- Backward compat: existing config files contain `aria2.min_split_size: "1M"` (string). serde ignores the unknown key, `min_split_size_mb` falls back to default `1` (= 1M, the previous default). A custom value silently resets to 1M — acceptable (0.1.0, no release).
- aria2 passthrough in bytes: `Value::String((mb * 1024 * 1024).to_string())` — matches the `* 1024` conversion style used for neighboring speed limits.

## Changes

### 1. `src/config.rs`
- `Aria2Options`: replace
  ```rust
  #[serde(default = "default_min_split_size")]
  pub min_split_size: String,
  ```
  with
  ```rust
  #[serde(default = "default_min_split_size_mb")]
  pub min_split_size_mb: u64,
  ```
- Replace `default_min_split_size() -> String` with `default_min_split_size_mb() -> u64 { 1 }`.
- `Default` impl: `min_split_size_mb: 1` (was `min_split_size: "1M".to_string()`).
- `to_aria2_task_options` (~line 126): change to
  ```rust
  extra.insert(
      "min-split-size".into(),
      Value::String((self.aria2.min_split_size_mb * 1024 * 1024).to_string()),
  );
  ```

### 2. `src/app.rs`
- `SettingKey::MinSplitSize` arm (line 562): parse `u64` instead of storing the raw string:
  ```rust
  SettingKey::MinSplitSize => {
      if let Ok(n) = value.parse::<u64>() {
          state.settings.aria2.min_split_size_mb = n;
      }
  }
  ```
  (No new message variant, no unit state — the previous plan's `SizeUnit`/`SizeUnitChanged`/`split_units` are dropped entirely.)

### 3. `src/ui/settings_page.rs`
- Replace the `labeled_text_input` call (lines 286-290) with a `setting_row` wrapping a `number_stepper` (already imported) plus an "M" suffix label:
  ```rust
  .push(
      setting_row(
          fluent.get(Tr::MinSplitSize),
          row![]
              .spacing(8)
              .push(number_stepper(
                  &settings.aria2.min_split_size_mb,
                  1..=1024u64,
                  1,
                  move |v| Message::SettingChanged(SettingKey::MinSplitSize, v.to_string()),
                  Length::Fixed(160.0),
              ))
              .push(
                  text("M")
                      .size(13)
                      .style(theme::style::text::secondary),
              )
              .align_y(Alignment::Center)
              .into(),
      ),
  )
  ```
- No changes to `SettingsUiState` (no unit map), no new `size_labeled_input`, no `UnitOption` genericization.

No i18n changes: `Tr::MinSplitSize` label reused; the fixed "M" unit suffix is a hardcoded secondary-styled text (same pattern as the hardcoded "KB/s" unit labels).

## Validation
- `cargo build`
- `cargo clippy --workspace` (no warnings)
- `cargo fmt --check`
- Manual: settings → Download → 最小分片大小 shows a stepper (default `1` M, range 1-1024); typing/+/− updates the value; Apply persists and `aria2` receives `min-split-size` in bytes within `[1048576, 1073741824]`.

## Notes
- Step is fixed at 1 MiB; values are integers only (aria2 also accepts fractional sizes like `1.5M`, out of scope).
- Cap at 1024 MiB matches aria2's `1_g` maximum exactly; the aria2 minimum of 1 MiB is enforced by the stepper's lower bound.
