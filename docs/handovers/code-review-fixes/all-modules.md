# Handover: Code Review Fixes — CR-01 Batch

## 改动摘要

本批次实现了代码审查发现的全部 Critical/High/Medium 问题的修复，共 8 个模块（M1–M8）。所有改动均经 `cargo fmt`、`cargo clippy -D warnings`、`cargo test --workspace` 验证通过。

## 修改文件

| 文件 | 改动 |
| --- | --- |
| `crates/wiki-kernel/src/memory.rs` | `to_snapshot` 对四个集合按 id 排序；新增 `to_snapshot_is_deterministic` 单测 |
| `crates/wiki-mempalace-bridge/src/lib.rs` | `SourceIngested` unresolved scope 改为 `record_unresolved` 而非 `record_filtered`；doc comment 同步；新增 2 个单测 |
| `crates/wiki-kernel/src/engine.rs` | `flush_outbox_to_repo_with_policy` drain 范围精确（只 drain 已成功追加的事件）；新增 `EngineError::Internal`；`expect` 替换 |
| `crates/wiki-storage/src/lib.rs` | `save_snapshot` 加 BEGIN IMMEDIATE / COMMIT / ROLLBACK 事务包装 |
| `crates/wiki-cli/src/consistency.rs` | `notion_uuid_from_target` 改为锚定提取：文件名末尾 32hex + URL 路径末尾段；不再滑动窗口扫描 |
| `crates/wiki-migration-notion/src/writer.rs` | `url_index` 改为先 multi-map 再取首条；`WriteStats` 添加 `duplicate_urls` 字段 |
| `crates/rust-mempalace/src/db.rs` | `migrate_schema` 新增 `hits` 列迁移（兼容旧行） |
| `crates/rust-mempalace/src/service.rs` | `benchmark_run` 存入 `out.total`（实际执行数）和 `out.hits`；`latest_benchmark` 读取 `hits` 列 |
| `crates/wiki-kernel/src/wiki_writer.rs` | `cleanup_stale_managed_pages` 补充保护注释 |

## 文档文件

| 文件 | 说明 |
| --- | --- |
| `docs/prd/code-review-fixes.md` | 本批 PRD |
| `docs/specs/code-review-fixes/requirements.md` | 功能需求 |
| `docs/specs/code-review-fixes/design.md` | 设计方案 |
| `docs/specs/code-review-fixes/tasks.md` | 任务清单（已全部勾选） |

## 暴露接口变更

- `EngineError::Internal(String)` — 新增 enum 变体，不影响现有 match 穷举（thiserror enum，非 #[non_exhaustive]），若有外部 exhaustive match 需补分支。
- `WriteStats::duplicate_urls: usize` — 新增字段，`#[derive(Default)]` 默认为 0，不影响现有构建代码。
- `BenchmarkResult.hits` — 已存在，`latest_benchmark` 现在返回真实值而非 0。

## 已知限制

- M4 (`notion_uuid_from_target`) 的新提取逻辑只覆盖两种标准 Notion 格式。非标准导出格式（如自定义 slug 含多段 hex）可能不匹配，但比原来的滑动窗口更安全（false negative 优于 false positive）。
- MCP `_wiki_dir` 未启用 vault projection（属于独立 PRD 范围，本批仅保留参数）。
- benchmark `mode=random` 仍无确定性种子（属于独立 PRD 范围）。
- outbox 消费者游标（at-exactly-once）属于独立 outbox-v2 PRD 范围。

## 新增依赖

无新依赖。

## 测试结果

```
cargo fmt --all -- --check   ✓
cargo clippy --workspace --all-targets -- -D warnings   ✓
cargo test --workspace   ✓ (全部通过，包含新增单测)
```

## Spec / checklist 状态

`docs/specs/code-review-fixes/tasks.md` 全部勾选完成。

## 下一步建议

1. **MCP Vault Sync**（roadmap 已登记）：为 `wiki-cli/src/mcp.rs` 添加 `--sync-wiki` flag，在写操作后调用 `write_projection`（独立 PRD）。
2. **Outbox Consumer Cursors**（roadmap 已登记）：为 `export_outbox_ndjson_from_id` 添加 consumer-scoped cursor 表，实现 at-exactly-once 语义（独立 outbox-v2 PRD）。
3. **Benchmark Reproducibility**（roadmap 已登记）：为 `benchmark_run` 添加 `--seed` 参数并存入 DB（独立配置 PRD）。
4. **Embedding Tx Atomicity**（roadmap 已登记）：将 `upsert_embedding` 纳入 snapshot+outbox 同一 SQLite transaction，需存储层改造（风险最高，建议最后处理）。
5. 合并 PR #34 后，回填 `docs/prd/code-review-fixes.md` Status 中的 "Merged" 和 "Roadmap updated" 勾选，并将 roadmap 中 CR-01 条目状态改为 `✅ 已合入`。
