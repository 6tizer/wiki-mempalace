# Code Review Fixes — Requirements

## FR-01: 快照序列化确定性 (M1)

- `InMemoryStore::to_snapshot` 产生的 `StorageSnapshot` 的 `sources`、`claims`、`pages`、`entities` 字段必须按稳定 id 升序排列。
- 同一逻辑 state 在任意次调用 `to_snapshot` 后产生完全相同的 JSON 字节串。

## FR-02: SourceIngested unresolved 语义修正 (M2)

- 当 `resolver.source_scope(id)` 返回 `None` 时，该事件必须计入 `stats.unresolved`，不计入 `stats.filtered`。
- `scope_filter` 返回 `false` 的情况才计入 `filtered`。
- 不带 resolver（`NoResolver`）路径行为不变：直接调用 `on_source_ingested` 并计入 `dispatched`。

## FR-03: flush_outbox drain 范围修正 (M3-a)

- `flush_outbox_to_repo_with_policy` 在第 i 个事件追加失败时，`drain` 的范围只包含第 0..i 条（已成功追加的部分），不能包含第 i 条（失败）及之后未尝试的部分。
- 修复后重试时从正确位置继续，不重复追加已成功部分，不丢失未尝试部分。

## FR-04: expect 替换为 EngineError (M3-b)

- `save_to_repo_with_retry` 中的 `last_err.expect("...")` 替换为返回具名 `EngineError` 变体，不在库代码中 panic。

## FR-05: notion_uuid_from_target 锚定 (M4)

- 仅在以下两种情况下提取 32 位 hex UUID：
  1. Notion 本地文件名格式：文件名末尾（`.md` 之前）有以空格分隔的 32 hex 字符段，如 `PageTitle abc123...def.md`。
  2. Notion Web URL 格式：URL 路径最后一个段含 32 hex 字符串（通常是 `<slug>-<32hex>` 或纯 32hex）。
- 不再用滑动窗口扫描整串，避免匹配无关 hex 子串。

## FR-06: url_index 重复 URL 处理 (M5)

- 当两个 Notion 页面具有相同标准化 `文章链接` URL 时，`url_index` 必须保留两者，不静默覆盖。
- 实现：改为 `HashMap<String, Vec<&PageLocation>>`；或在插入时记录 `WriteStats.duplicate_urls` 并用第一条（保持旧行为但有可见计数）。
- 下游 `rewrite_body` 在命中重复 URL 时取第一条并在 stats 中标记，以保持链接合法。

## FR-07: benchmark hits 真实存储与读取 (M6-a)

- `benchmark_runs` 表增加 `hits INTEGER NOT NULL DEFAULT 0` 列。
- `benchmark_run` 写入时存 `out.hits`。
- `latest_benchmark` 读取时返回 `hits` 列真实值。
- 迁移：用 `ALTER TABLE … ADD COLUMN` 兼容旧行（旧行 hits=0）。

## FR-08: benchmark samples 存 actual total (M6-b)

- `benchmark_run` 存入 `benchmark_runs.samples` 的值改为 `out.total`（实际执行查询数），而非函数参数 `samples`。
- 理由：`samples` 参数是"请求数"，`out.total` 才是"有效执行数"（跳过空内容后）。

## NFR

- 所有改动通过 `cargo test --workspace` 和 `cargo clippy --workspace --all-targets -- -D warnings`。
- 不引入新的 unsafe 代码。
- 不改变任何 public API 签名。
