# aria2 二进制目录扫描自愈 + 升级测试方案

## 背景与问题

当前 `ensure_aria2_next`（`src/aria2_fetcher.rs:64-141`）的缓存命中逻辑严格依赖 `.installed`：
读 `.installed` -> 用 `version+slug` 拼出二进制文件名 -> 检查该路径是否存在 -> 校验 sha256。

**问题**：用户删除二进制、手动替换不同版本二进制、或删了 `.installed` 后，代码忽略目录里已有的可用二进制，直接走 GitHub 下载（可能超时失败）。

**测试需求**：本机已有 `target/debug/aria2/aria2-next-2.5.2-linux-x86_64`，GitHub 最新也是 2.5.2。要测试"低版本->高版本"升级流程，需让 app 以为本地是低版本。当前代码做不到——改文件名不改 `.installed` 则缓存失效直接重新下载。

## 决策

| 决策点 | 选择 |
|---|---|
| 二进制发现策略 | `.installed` 缓存命中（快路径）-> 目录扫描兜底（自愈 `.installed`）-> GitHub 下载 |
| 扫描匹配规则 | 文件名匹配 `aria2-next-{version}-{slug}`，且 Unix 下可执行 |
| 多个二进制 | 选版本号最高的 |
| sha256 校验 | 扫描命中时不做外部校验（信任本地文件），仅计算并写入 `.installed` 供下次快路径校验 |
| 版本号来源 | 从文件名解析（非运行 `--version`），便于用户通过重命名伪造低版本测试 |

## 实施任务

### 1. 目录扫描 + 自愈（`src/aria2_fetcher.rs`）

新增以下函数：

- `fn parse_version_from_filename(filename: &str, slug: &str) -> Option<String>`：
  去掉 `aria2-next-` 前缀和 `-{slug}` 后缀（slug 含 `-`，从末尾匹配），剩余部分需能解析为 `.` 分隔数字（如 `2.5.0`）。Windows slug 为 `windows-x86_64.exe`，后缀含 `.exe` 一并处理。

- `fn version_tuple(v: &str) -> Vec<u64>`：按 `.` 分割解析为数字向量，用于比较大小。

- `fn scan_for_binary(dir: &Path, slug: &str) -> Option<(PathBuf, String)>`：
  `std::fs::read_dir` 遍历目录，对每个条目用 `parse_version_from_filename` 解析版本（过滤 `.part`/`.installed`/`.pending-update`/`session.txt` 等非二进制文件）。Unix 下用 `PermissionsExt` 验证可执行权限（跳过非可执行）。收集候选 `(path, version)`，选 `version_tuple` 最大的返回。

- `fn self_heal_installed(dir: &Path, bin_path: &Path, version: &str, slug: &str) -> Result<(), String>`：
  `sha256_file(bin_path)` -> 写 `.installed`（`InstalledInfo { version, slug, sha256 }`）。

改 `ensure_aria2_next`（`src/aria2_fetcher.rs:64`）：

将 `let slug = updater::platform_slug();`（当前 line 99）提前到 `apply_pending_update` 之后（line 76 之后）。在 `.installed` 缓存未命中后（line 95 之后、`emit_status("downloading")` line 97 之前）插入：

```rust
if let Some((bin_path, version)) = scan_for_binary(&dir, slug) {
    tracing::info!(%version, ?bin_path, "aria2-next found via directory scan, self-healing .installed");
    self_heal_installed(&dir, &bin_path, &version, slug)?;
    emit_status(event_tx, "ready", &format!("aria2-next {version} ready"));
    return Ok((bin_path, applied));
}
```

改 `installed_version()`（`src/aria2_fetcher.rs:175`）：`.installed` 读取失败时兜底扫描：

```rust
pub fn installed_version() -> Option<String> {
    let dir = aria2_bin_dir()?;
    if let Some(info) = read_installed(&dir) {
        return Some(info.version);
    }
    let slug = updater::platform_slug();
    scan_for_binary(&dir, slug).map(|(_, v)| v)
}
```

### 2. 验证

`cargo clippy --workspace`（无 warning）+ `cargo fmt --check`。

## 测试方案

### 测试 A：扫描自愈（无网络）

```bash
cd /home/caoyucong/workspace/remotrix
BIN_DIR=target/debug/aria2
rm -f $BIN_DIR/.installed
cargo run --
# 预期：设置页显示 aria2-next v2.5.2（扫描命中，自愈 .installed）
# 验证：cat $BIN_DIR/.installed -> version 2.5.2 + 正确 sha256
```

### 测试 B：低版本->高版本升级全流程（需 github.com 可达）

```bash
cd /home/caoyucong/workspace/remotrix
BIN_DIR=target/debug/aria2
# 伪造低版本：重命名二进制 + 删 .installed
mv $BIN_DIR/aria2-next-2.5.2-linux-x86_64 $BIN_DIR/aria2-next-2.5.0-linux-x86_64
rm -f $BIN_DIR/.installed
cargo run --
# 预期流程：
#   - 扫描命中 2.5.0，自愈 .installed，sidecar 启动，显示 v2.5.0
#   - 自动检查 -> GitHub 返回 2.5.2 -> 后台暂存下载 -> "正在下载更新…"
#   - 完成后按钮变 "重启更新"
# 点击 "重启更新":
#   - 引擎重启 -> apply_pending_update -> 删 2.5.0 -> .installed 更新为 2.5.2
#   - 显示 "已更新到 v2.5.2"
#   - ls $BIN_DIR/ -> 仅 aria2-next-2.5.2-linux-x86_64
```

### 测试 C：pending-apply 流程（无需 github.com）

手动构造 pending 状态，直接测试"重启应用 pending 升级"。

```bash
cd /home/caoyucong/workspace/remotrix
BIN_DIR=target/debug/aria2
SHA=e3f448b40487d5899d292ac78598052b938db98dc5e7e3533d6b94a00bc40213

# 低版本二进制 + 高版本"已暂存"二进制
cp $BIN_DIR/aria2-next-2.5.2-linux-x86_64 $BIN_DIR/aria2-next-2.5.0-linux-x86_64

# .installed 指向低版本
cat > $BIN_DIR/.installed <<EOF
{"version":"2.5.0","slug":"linux-x86_64","sha256":"$SHA"}
EOF

# .pending-update 指向高版本
cat > $BIN_DIR/.pending-update <<EOF
{"version":"2.5.2","slug":"linux-x86_64","sha256":"$SHA"}
EOF

cargo run --
# 预期：
#   - apply_pending_update -> 校验 2.5.2 sha256 ✓ -> 删 2.5.0 -> .installed 更新 -> 删 .pending-update
#   - boot 返回 applied=Some("2.5.2") -> Aria2UpdateApplied
#   - 显示 "已更新到 v2.5.2"
# 验证：ls $BIN_DIR/ 仅 2.5.2，cat .installed -> 2.5.2，无 .pending-update
```

### 测试 D：降级模式下检查更新仍可用

```bash
cd /home/caoyucong/workspace/remotrix
BIN_DIR=target/debug/aria2
rm -f $BIN_DIR/aria2-next-* $BIN_DIR/.installed $BIN_DIR/.pending-update
cargo run --
# 预期：扫描无果 -> 下载失败 -> 设置页显示错误 + "重试下载"
# 点"检查更新" -> 不依赖 sidecar -> 返回结果（不静默失效）
# 下载类操作 -> EngineDegraded 提示
```

## 受影响文件

- `src/aria2_fetcher.rs` - 新增 `parse_version_from_filename`/`version_tuple`/`scan_for_binary`/`self_heal_installed`，改 `ensure_aria2_next` 插入扫描兜底，改 `installed_version` 兜底扫描

## 风险

- **孤儿二进制**：`apply_pending_update` 删旧二进制依赖 `.installed` 中的旧版本名。若 `.installed` 缺失则旧二进制残留。影响极小（仅磁盘占用）。
- **Windows 可执行权限**：Windows 无 Unix 权限位，`scan_for_binary` 的可执行检查需 `#[cfg]` 分支跳过。
- **版本号伪造**：扫描从文件名解析版本，用户可重命名伪造任意版本号。这是设计意图（便于测试），非安全问题——二进制内容由用户自行负责。
