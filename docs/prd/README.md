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

- [mcp-typed-errors.md](mcp-typed-errors.md) — PR #TBD：MCP JSON-RPC typed error mapping for stable client-side diagnostics.
- [batch-3.md](batch-3.md) — P2 maturity：metrics、dashboard、strategy、tag governance、LongMemEval automation。
- [compiler-canonicalization-v2.md](compiler-canonicalization-v2.md) — PR #47：pre-write compiler resolver, bounded candidate retrieval, persisted alias/canonical mapping, and small LLM fallback.
- [compiler-deferred-resolution-agent.md](compiler-deferred-resolution-agent.md) — PR #54：machine-only resolver/fixer for compiler `deferred_resolutions`, DB-first apply order, production apply smoke completed.
- [notion-incremental-sync.md](notion-incremental-sync.md) — Notion API incremental sync, cursor tables, refresh existing source bodies/tags, automation job.
- [notion-source-vault-projection.md](notion-source-vault-projection.md) — PR #42：DB-backed Notion source projection to Obsidian, block body refresh, Obsidian-safe tags.
