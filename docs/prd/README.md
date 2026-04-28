# PRD Index

本目录存放产品范围文档。PRD 只描述目标、用户价值、范围和验收，不承载实现细节。

规则：

- Agent 不主动改 PRD 范围。
- 如果实现中发现范围需要变，先暂停并让用户决定。
- PRD 通过白话架构对话确认后，再拆 spec 三件套。

## Active PRDs

- [production-wiki-compiler.md](production-wiki-compiler.md) — Notion-equivalent local compiler operation: raw source -> resolver -> summary + concept/entity pages -> Vault -> outbox -> Mempalace -> query/explain; controlled production scale-up is active.
- [vault-backfill-and-palace-init.md](vault-backfill-and-palace-init.md) — 历史 vault 回填、`wiki.db` 初始化、`palace.db` 初始化、共享 Agent 运行默认值。
- [storage-embeddings-followup.md](storage-embeddings-followup.md) — C16 follow-up: single-transaction `wiki_state` + outbox write path; optional ANN/index for `wiki_embedding` search (replaces O(n) scan at scale). Spec: [persist-snapshot-outbox](../specs/persist-snapshot-outbox/requirements.md), [embedding-ann-index](../specs/embedding-ann-index/requirements.md).

## Completed PRDs Still Useful As Reference

- [cli-command-modularization.md](cli-command-modularization.md) — PR #78/#79：staged `wiki-cli` command handler extraction, no-engine dispatch/runtime setup, and CLI smoke coverage without clap behavior drift.
- [row-level-wiki-state.md](row-level-wiki-state.md) — PR #76/#77：row-level `wiki_state` migration/dual-write, row-primary read, blob fallback, and verification API.
- [semantic-fusion-benchmark.md](semantic-fusion-benchmark.md) — PR #75：J14 LongMemEval side-by-side query baseline vs semantic-fusion benchmark lane.
- [contradiction-scan-scaling.md](contradiction-scan-scaling.md) — PR #74：bounded candidate scan for `naive_contradiction_pairs`, with stale prefilter and scope buckets.
- [embedding-tx-atomicity.md](embedding-tx-atomicity.md) — PR #71：snapshot/outbox/embedding rows commit in one SQLite transaction for vector-enabled source/claim writes.
- [benchmark-reproducibility.md](benchmark-reproducibility.md) — PR #70：deterministic random benchmark seed for `rust-mempalace bench`.
- [notion-archived-source-retirement.md](notion-archived-source-retirement.md) — PR #68/#69：DB-first Notion archived source retirement audit/plan plus guarded apply.
- [scheduled-vault-reports.md](scheduled-vault-reports.md) — PR #67：Automation `vault-reports` job for timestamped Vault report bundles, latest pointers, and retention.
- [dependency-audit-automation.md](dependency-audit-automation.md) — PR #66：Scheduled/manual `cargo audit` lane with artifact upload, kept out of quick PR checks.
- [reliability-test-matrix.md](reliability-test-matrix.md) — PR #65：Reliability regression coverage for DB failure injection, large smoke, MCP malformed input, and LLM bad JSON.
- [multi-process-writer-lease.md](multi-process-writer-lease.md) — PR #64：Repository-adjacent writer lease for write CLI/MCP entrypoints.
- [outbox-consumer-cursors.md](outbox-consumer-cursors.md) — PR #62/#63：Consumer-scoped outbox cursor export API plus CLI/consumer cutover.
- [mcp-vault-sync.md](mcp-vault-sync.md) — PR #61：MCP write tools trigger Vault projection when `--sync-wiki` is enabled.
- [mcp-typed-errors.md](mcp-typed-errors.md) — PR #60：MCP JSON-RPC typed error mapping for stable client-side diagnostics.
- [batch-3.md](batch-3.md) — P2 maturity：metrics、dashboard、strategy、tag governance、LongMemEval automation。
- [compiler-canonicalization-v2.md](compiler-canonicalization-v2.md) — PR #47：pre-write compiler resolver, bounded candidate retrieval, persisted alias/canonical mapping, and small LLM fallback.
- [compiler-deferred-resolution-agent.md](compiler-deferred-resolution-agent.md) — PR #54：machine-only resolver/fixer for compiler `deferred_resolutions`, DB-first apply order, production apply smoke completed.
- [notion-incremental-sync.md](notion-incremental-sync.md) — Notion API incremental sync, cursor tables, refresh existing source bodies/tags, automation job.
- [notion-source-vault-projection.md](notion-source-vault-projection.md) — PR #42：DB-backed Notion source projection to Obsidian, block body refresh, Obsidian-safe tags.
