# Code Review Fixes — Design

## M1: 快照序列化确定性

**文件**: `crates/wiki-kernel/src/memory.rs`

`to_snapshot` 当前直接从 `HashMap::values()` collect，顺序不确定。

修复：对四个集合 sort 后再 collect。
- `sources`: `.sort_by_key(|x| x.id.0)` → 按 `Uuid` 排序（Uuid 实现 Ord）
- `claims`: `.sort_by_key(|x| x.id.0)`
- `pages`: `.sort_by_key(|x| x.id.0)`
- `entities`: `.sort_by_key(|x| x.id.0)`

`edges: Vec<TypedEdge>` 已是 Vec，顺序由插入决定，不动。

测试：新增 `to_snapshot_is_deterministic` 单测，向 store 插入多个 claim/page/source，两次调用 `to_snapshot` 后序列化 JSON 相同。

## M2: SourceIngested unresolved 语义修正

**文件**: `crates/wiki-mempalace-bridge/src/lib.rs`

当前代码（约 line 292-305）：

```rust
WikiEvent::SourceIngested { source_id, .. } => {
    let allow = match resolver {
        Some(r) => match r.source_scope(source_id) {
            Some(scope) => sink.scope_filter(&scope),
            None => false,          // <-- 走 filtered 分支，逻辑错误
        },
        None => true,
    };
    if allow {
        ...dispatched
    } else {
        stats.record_filtered(event_name);   // <-- 应为 record_unresolved
    }
}
```

修复：将 `None => false` 路径改为单独处理，调用 `record_unresolved` 而非归入 `filtered`：

```rust
WikiEvent::SourceIngested { source_id, .. } => {
    match resolver {
        Some(r) => match r.source_scope(source_id) {
            Some(scope) => {
                if sink.scope_filter(&scope) {
                    sink.on_source_ingested(source_id)?;
                    stats.record_dispatched(event_name);
                } else {
                    stats.record_filtered(event_name);
                }
            }
            None => stats.record_unresolved(event_name),
        },
        None => {
            sink.on_source_ingested(source_id)?;
            stats.record_dispatched(event_name);
        }
    }
}
```

测试：新增 `source_ingested_unresolved_scope_counted_as_unresolved` 单测（类比 `unresolved_supersede_scope_is_not_dispatched`）。

## M3-a: flush_outbox drain 范围修正

**文件**: `crates/wiki-kernel/src/engine.rs`

当前代码 `self.outbox.drain(..n)` 在追加第 i 个事件失败时执行，但 `n` 是外层循环变量（batch 起始位置），指向当前 batch 起始，不包含已在本次内层循环成功追加的事件。

检查后：`n` 在外层每次迭代更新 `n = end`（上一批结束）。内层循环在第 `e` 个事件失败时调用 `drain(..n)`，`n` 仍是当前 batch 的起始，而 batch 内 `0..e` 个事件已成功追加，这部分丢失了。

修复：引入 `flushed` 计数器追踪成功追加数量，失败时 drain `..flushed`：

```rust
let mut flushed = 0usize;
while flushed < self.outbox.len() {
    let end = usize::min(flushed + size, self.outbox.len());
    for event in &self.outbox[flushed..end] {
        let mut last_err: Option<EngineError> = None;
        for _ in 0..=retry_count {
            match repo.append_outbox(event) {
                Ok(()) => { last_err = None; break; }
                Err(err) => { last_err = Some(err.into()); }
            }
        }
        if let Some(err) = last_err {
            self.outbox.drain(..flushed);
            return Err(err);
        }
        flushed += 1;
    }
}
self.outbox.clear();
Ok(flushed)
```

## M3-b: expect 替换

**文件**: `crates/wiki-kernel/src/engine.rs`

`save_to_repo_with_retry` 中 `last_err.expect("retry loop runs at least once")` 替换为：

```rust
Err(last_err.unwrap_or_else(|| EngineError::Internal("retry loop produced no error".into())))
```

需确认 `EngineError` 有 `Internal(String)` 变体，或添加。

## M4: notion_uuid_from_target 锚定

**文件**: `crates/wiki-cli/src/consistency.rs`

Notion 本地文件名格式：`Some Title abc123...def.md`，UUID 位于最后一个空格之后、`.md` 之前。
Notion Web URL 格式：`https://www.notion.so/slug-abc123...def`，UUID 位于最后一个 `-` 之后或路径末尾。

修复：先尝试文件名模式（末尾 `<32hex>.md` 或 ` <32hex>.md`），再尝试 URL 模式（路径末尾 32hex 段）：

```rust
fn notion_uuid_from_target(target: &str) -> Option<String> {
    // 文件名模式：末尾 <32hex>.md（可选前缀空格）
    if let Some(stem) = target.strip_suffix(".md").or(Some(target)) {
        // 取最后一个空格或斜杠之后的段
        let last_seg = stem.rsplit(|c| c == ' ' || c == '/').next().unwrap_or(stem);
        if last_seg.len() >= 32 {
            let candidate = &last_seg[last_seg.len() - 32..];
            if candidate.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(candidate.to_ascii_lowercase());
            }
        }
    }
    // URL 模式：路径最后段（去掉查询串）末尾 <slug>-<32hex>
    let path_part = target.split('?').next().unwrap_or(target);
    let last_seg = path_part.rsplit('/').next().unwrap_or(path_part);
    let hex_part = last_seg.rsplit('-').next().unwrap_or(last_seg);
    if hex_part.len() == 32 && hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(hex_part.to_ascii_lowercase());
    }
    None
}
```

## M5: url_index 重复 URL 处理

**文件**: `crates/wiki-migration-notion/src/writer.rs`

改 `url_index` 为 `HashMap<String, Vec<&PageLocation>>`。`rewrite_body` 取第一条（原行为），同时 `stats.duplicate_urls += 1` 当 `> 1` 条时。需在 `WriteStats` 添加 `duplicate_urls: usize` 字段。

## M6: benchmark fixes

**文件**: `crates/rust-mempalace/src/db.rs` + `service.rs`

1. `db.rs`：`init_db` 在 `CREATE TABLE IF NOT EXISTS benchmark_runs` 后追加迁移语句：
   ```sql
   ALTER TABLE benchmark_runs ADD COLUMN hits INTEGER NOT NULL DEFAULT 0;
   ```
   用 `execute` + 忽略 "duplicate column" 错误来兼容旧库。

2. `service.rs`：`benchmark_run` INSERT 语句加入 `hits` 列，值为 `out.hits`。

3. `service.rs`：`benchmark_run` INSERT 的 `samples` 参数改为 `out.total as i64`。

4. `service.rs`：`latest_benchmark` SELECT 加 `hits`，`BenchmarkResult` 从 `r.get(6)?` 读取。
