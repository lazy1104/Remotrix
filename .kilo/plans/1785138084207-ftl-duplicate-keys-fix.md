# Fix: Duplicate FTL message IDs causing startup panic

## Problem
`fluent-templates 0.14.0` panics at startup:
```
Failed to add FTL resources to the bundle.:
[Overriding { kind: Message, id: "max-concurrent" }, Overriding { kind: Message, id: "split" }]
```

The settings restructure (commit prior to this) reorganized the FTL key block but left two original standalone lines in place that now collide with their reorganized counterparts inside the new block.

## Root Cause (verified by reading both files)
Both `i18n/locales/en/main.ftl` and `i18n/locales/zh-CN/main.ftl` contain duplicate keys:

| File | Leftover (lines 34-35) | New block (lines 37-38) |
|---|---|---|
| en | `max-concurrent = Max concurrent downloads` / `split = Split / max connections per task` | `max-concurrent = Max concurrent downloads` / `split = Split` |
| zh-CN | `max-concurrent = 最大并发数` / `split = 分片 / 每任务连接数` | `max-concurrent = 最大并发数` / `split = 分片数` |

Only `max-concurrent` and `split` are duplicated (the reorganized block replaced the range starting at `speed-limits`, so those two were the only keys sitting just above the replaced range). No other duplicates exist - confirmed by scanning both files.

## Decision
**Keep the new-block versions (lines 37-38), delete the leftover originals (lines 34-35).** Rationale: the new `split` label (`Split` / `分片数`) is the intended shorter label matching the `Tr::Split` NumberInput in `settings_page.rs`; `max-concurrent` is identical in both locations.

## Tasks
1. **`i18n/locales/en/main.ftl`** - delete line 34 (`max-concurrent = Max concurrent downloads`) and line 35 (`split = Split / max connections per task`). The new-block copies on lines 37-38 remain.
2. **`i18n/locales/zh-CN/main.ftl`** - delete line 34 (`max-concurrent = 最大并发数`) and line 35 (`split = 分片 / 每任务连接数`). The new-block copies on lines 37-38 remain.

After deletion, line 33 (`download-folder`) is followed directly by line 36 (`connection-segment`), preserving the group ordering: download-folder -> connection-segment -> ... 

## Validation
- `cargo build` (the FTL loader runs at startup via `static_loader!`; a clean build + run proves no panic).
- Optionally `cargo run` briefly to confirm the app window opens without the fluent panic.
- No code changes required; the fix is purely in the two `.ftl` data files.

## Risk
None. The duplicate IDs were a data-layer oversight; removing them restores the single-definition invariant fluent-templates requires. No Rust code references change.
