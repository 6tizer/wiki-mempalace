# Code Review Fixes — Tasks

## Checklist

### M1: 快照序列化确定性
- [x] impl: memory.rs `to_snapshot` 对 sources/claims/pages/entities 按 id 排序
- [x] test: 新增 `to_snapshot_is_deterministic` 单测

### M2: SourceIngested unresolved 语义修正
- [x] impl: mempalace-bridge/lib.rs 分离 unresolved / filtered 路径
- [x] test: 新增 `source_ingested_unresolved_scope_counted_as_unresolved`
- [x] test: 新增 `source_ingested_filtered_scope_counted_as_filtered`

### M3: flush_outbox drain + expect
- [x] impl: engine.rs `flush_outbox_to_repo_with_policy` drain 范围修正
- [x] impl: engine.rs `save_to_repo_with_retry` expect 替换 + `EngineError::Internal` 新增

### M4: notion_uuid_from_target 锚定
- [x] impl: consistency.rs `notion_uuid_from_target` 锚定模式（文件名末尾 32hex + URL 末尾段）
- [x] test: 全套现有 consistency 测试通过

### M5: url_index 重复 URL
- [x] impl: writer.rs `url_index` 先 build multi-map，再取第一条，记录 `duplicate_urls`
- [x] impl: WriteStats 添加 `duplicate_urls: usize` 字段

### M6: benchmark fixes
- [x] impl: db.rs `migrate_schema` 添加 `hits` 列迁移（兼容旧行 hits=0）
- [x] impl: service.rs INSERT 加 hits 列，values 改 out.hits；samples 改为 out.total
- [x] impl: service.rs `latest_benchmark` SELECT 加 hits 列，从 r.get(6) 读取

### M7: save_snapshot 事务一致性
- [x] impl: storage.rs `save_snapshot` 加 BEGIN IMMEDIATE / COMMIT / ROLLBACK 包装

### M8: cleanup_stale_managed_pages 文档补充
- [x] doc: wiki_writer.rs 函数注释说明保护条件和手动文件注意事项

### Cross-cutting
- [x] `cargo fmt --all`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 提交并推送
- [x] 更新 handover 文档
- [x] 创建 PR (#34)

### 延后 follow-up（独立 PRD，不在本批范围）
- [ ] MCP Vault Sync — `wiki-cli/src/mcp.rs` 写操作后自动调用 `write_projection`（roadmap: "MCP Vault Sync"）
- [ ] Outbox Consumer Cursors — at-exactly-once 消费语义，添加 consumer-scoped cursor 表（roadmap: "Outbox Consumer Cursors"）
- [ ] Embedding Tx Atomicity — `upsert_embedding` 纳入 snapshot+outbox 同一事务（roadmap: "Embedding Tx Atomicity"）
- [ ] Benchmark Reproducibility — `--seed` 参数存入 `benchmark_runs`（roadmap: "Benchmark Reproducibility"）

## Status

- [x] tasks.md 创建
- [x] M1 完成
- [x] M2 完成
- [x] M3 完成
- [x] M4 完成
- [x] M5 完成
- [x] M6 完成
- [x] M7 完成
- [x] M8 完成
- [x] CI green (local: fmt ✓ clippy ✓ test ✓)
- [x] PR #34 open (draft)