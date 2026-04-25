# Code Review C15 Handoff

- Branch: `fix/code-review-issues`
- Scope: 全项目代码审查（C15）——修复安全/正确性/性能/可维护性问题 13 项，推迟 2 项并立 spec。

## 文件变更

| 文件 | 改动内容 |
|------|---------|
| `crates/wiki-kernel/src/engine.rs` | `flush_outbox_to_repo_with_policy`：失败时先 trim 已成功写入的事件再返回 Err，防止 retry 重复写；`rank_fused_with_retention` 排序改用 `total_cmp`（NaN 安全）。 |
| `crates/wiki-storage/src/lib.rs` | `mark_outbox_processed`：用 `BEGIN IMMEDIATE…COMMIT` 事务（新增 `mark_outbox_processed_inner`）；`newly_acked` COUNT 加 `AND processed_at IS NULL`；`search_embeddings_cosine` 排序改 `total_cmp`，dim/blob 不匹配 `eprintln!` 警告；`backlog_events` 改 `saturating_sub`。 |
| `crates/wiki-mempalace-bridge/src/lib.rs` | `ClaimUpserted` unresolved 路径不再调 sink（与 `ClaimSuperseded` 行为一致）；带 resolver 时 `SourceIngested` scope 不可解则拒绝（默认从允许改为拒绝）；更新相关单测断言。 |
| `crates/wiki-core/src/schema.rs` | `SchemaValidationError` 增加 `UnreachableInitialStatus` 与 `OutOfRange` 变体；`validate()` 新增规则 4（initial_status 须在 promotion 图节点中）、规则 5（数值范围）；新增 4 个对应测试。 |
| `crates/wiki-cli/src/mcp.rs` | `wiki_export_graph_dot`：边按两端 entity 的 viewer scope 过滤；`wiki_ingest_llm` 向量 upsert 失败改 `eprintln!`；两处 `eng.schema.clone()` 改为借用。 |

## 已修复的 15 项中的 13 项

| # | 严重级 | 结论 |
|---|--------|------|
| 1 | Critical | ✅ engine: flush 部分失败后不再重复写 outbox |
| 3 | Critical | ✅ storage: mark_outbox_processed 四步改为单事务 |
| 4 | Critical | ✅ storage: newly_acked COUNT 加 processed_at IS NULL |
| 5 | Critical | ✅ bridge: ClaimUpserted unresolved 不再调 sink |
| 6 | Critical | ✅ bridge: SourceIngested resolver=None 时默认拒绝 |
| 8 | High | ✅ storage/engine: NaN 排序改 total_cmp |
| 9 | High | ✅ storage: dim/blob 不匹配改为 eprintln! 警告 |
| 10 | High | ✅ storage: backlog 改 saturating_sub |
| 11 | Medium | ✅ schema: initial_status 可达性校验 |
| 12 | Medium | ✅ schema: 数值范围校验 |
| 13 | Medium | ✅ mcp: DOT 边按 scope 过滤 |
| 14 | Medium | ✅ mcp: embedding upsert 错误改 eprintln! |
| 15 | Low | ✅ mcp: 去掉多余 schema.clone() |

## 推迟到 C16 的 2 项及原因

| # | 严重级 | 问题 | 推迟原因 | 追踪位置 |
|---|--------|------|---------|---------|
| 2 | Critical | `save_snapshot` + `flush_outbox` 不在同一事务（崩溃可致二者不一致） | `WikiRepository` trait 全是 `&self`，`Connection::transaction` 需要 `&mut`；需要 trait 签名变更和所有实现迁移，属于架构变更，不宜混入 hotfix PR | [specs/persist-snapshot-outbox/](../specs/persist-snapshot-outbox/requirements.md)，PRD: [storage-embeddings-followup.md](../prd/storage-embeddings-followup.md) |
| 7 | High | `search_embeddings_cosine` 全表 O(n) 扫描 | 需要引入 sqlite-vec / VSS 等 SQLite 扩展或外部 ANN 结构；涉及构建依赖、平台兼容、feature flag，是独立性能专项 | [specs/embedding-ann-index/](../specs/embedding-ann-index/requirements.md)，PRD: [storage-embeddings-followup.md](../prd/storage-embeddings-followup.md) |

## 验证

- `cargo test --workspace` — 374 个测试全部通过，0 失败。

## 下一步

1. 本 PR review + CI 通过后 merge main。
2. Merge 后在 `docs/LESSONS.md` 追加一节。
3. C16 须先走白话架构对话（trait 变更方案 A/B/C）确认后再开 `codex/persist-snapshot-outbox` 分支。
4. ANN 扩展须先确认构建/发行方案再开 `codex/embedding-ann-index` 分支。
