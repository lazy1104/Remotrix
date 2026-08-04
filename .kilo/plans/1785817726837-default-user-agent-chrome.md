# 默认 User-Agent 改为 Chrome 浏览器 UA

## 目标
把 Remotrix 默认 User-Agent 从 `Remotrix/{version}` 改为一个常见的 Chrome 浏览器 UA，避免服务器把该应用识别为下载工具而拦截。

## 变更
单点修改 `src/config.rs` 的 `default_user_agent()`（当前第 102-104 行）：

```rust
fn default_user_agent() -> String {
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36".to_string()
}
```

- `#[serde(default = "default_user_agent")]`（config.rs:46）与 `impl Default`（config.rs:166）已引用该函数，无需改动。
- 兼容性说明：已保存的配置文件里非空 `user_agent` 保持用户自设值，不会被覆盖；仅影响从未设置过的默认值。

## 边界
- `aria2_fetcher.rs` / `updater.rs` 的 `.user_agent("remotrix-updater")` 是更新下载用的，保持不动。
- 每个任务可单独覆盖 UA（add dialog / TaskAdvancedOptions），不影响。

## 验证
- `cargo build` 编译通过。
- `cargo clippy --workspace` 无警告。
- 运行后在 Settings > Network 的 User-Agent 编辑器应显示 Chrome UA。
