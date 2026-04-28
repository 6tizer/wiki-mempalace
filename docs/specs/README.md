# Spec Index

本目录存放每个功能模块的 spec 三件套：

- `requirements.md` — 行为、输入输出、验收标准。
- `design.md` — 数据结构、接口、流程、兼容性。
- `tasks.md` — task 分级、状态、review、验证。

规则：

- spec 是实现源。代码和 spec 不一致时，先改 spec，再改代码。
- 每个模块独立维护三件套。
- 模块完成后更新 tasks 状态和 checklist。

## Active Specs

- [m10-metrics/](m10-metrics/) — M10 unified metrics core. Merged PR #12。
- [m11-dashboard/](m11-dashboard/) — M11 read-only dashboard/report. Merged PR #14。
- [m12-strategy/](m12-strategy/) — M12 strategy suggestions. Merged PR #16。
- [schema-t2-tags/](schema-t2-tags/) — Schema T2 tag governance. Merged PR #13。
- [longmemeval-auto/](longmemeval-auto/) — J13 LongMemEval `rust-mempalace` local retrieval baseline artifacts. Merged PR #19。
- [vault-report-paths/](vault-report-paths/) — Vault-relative report output paths for dashboard, suggest, metrics, and automation health. Active branch `codex/vault-report-paths`。
- [vault-audit/](vault-audit/) — B1 read-only vault audit and report model. Merged PR #23。
- [vault-backfill/](vault-backfill/) — B2 stable IDs and vault-to-`wiki.db` backfill. Merged PR #23。
- [palace-init/](palace-init/) — B3 `palace.db` initialization from wiki outbox and fusion validation. Merged PR #23。
- [agent-runtime-defaults/](agent-runtime-defaults/) — B4 shared vault-local CLI/MCP defaults. Merged PR #23。
- [orphan-governance/](orphan-governance/) — B5 follow-up: timestamped audit, LLM plan, Chinese report, and whitelist apply. Merged PR #30。
- [db-vault-palace-consistency/](db-vault-palace-consistency/) — DB/Vault/Palace consistency audit, plan, apply, and Mempalace page replay. Merged PR #32。
- [notion-incremental-sync/](notion-incremental-sync/) — Notion API incremental sync into `wiki.db`, plus automation job integration. Merged.
- [notion-sync-index-backfill/](notion-sync-index-backfill/) — Historical Notion index backfill for already imported source pages. Merged.
- [notion-sync-trusted-tag-policy/](notion-sync-trusted-tag-policy/) — Treat Notion AI auto-fill tags as trusted upstream tags during Notion sync. Merged.
- [notion-source-vault-projection/](notion-source-vault-projection/) — Project DB-backed `notion://` sources into visible Obsidian source Markdown, refresh Notion block bodies, and emit Obsidian-safe source tags. Merged PR #42；production apply completed.
- [production-wiki-compiler/](production-wiki-compiler/) — Notion-equivalent local compiler: source -> resolver -> summary + concept/entity pages with backlinks -> Vault -> Mempalace; implementation merged and controlled production scale-up started.
- [compiler-canonicalization-v2/](compiler-canonicalization-v2/) — Pre-write compiler canonical resolver with bounded DB candidates, persisted aliases, small LLM fallback, and temp-vault regression. Merged PR #47.
- [compiler-deferred-resolution-agent/](compiler-deferred-resolution-agent/) — Machine-only post-write resolver/fixer for compiler `deferred_resolutions`; DB -> Vault -> Mempalace -> lint/audit apply order. Merged PR #54; production apply smoke completed.
- [persist-snapshot-outbox/](persist-snapshot-outbox/) — C16a: `wiki_state` + outbox append in one SQLite transaction; replaces split `save_to_repo` / `flush_outbox` autocommit for crash-safety. Merged PR #25. PRD: [storage-embeddings-followup.md](../prd/storage-embeddings-followup.md).
- [embedding-ann-index/](embedding-ann-index/) — C16b: bounded-work vector search for `wiki_embedding` (`ann-embed` locality-bucket path; fallback to full scan). PRD: [storage-embeddings-followup.md](../prd/storage-embeddings-followup.md)。
- [audit-report-hardening/](audit-report-hardening/) — Notion 全方位代码审计报告的可落地安全/可靠性修复：MCP 输入上限、LLM plan 校验、脱敏扩展、outbox batch 事务、FTS token quoting。已合入 PR #58。
- [mcp-typed-errors/](mcp-typed-errors/) — MCP JSON-RPC typed error mapping：稳定 `error.code` 与 `error.data.kind`，供后续 MCP Vault Sync / 客户端诊断复用。已合入 PR #60。
- [mcp-vault-sync/](mcp-vault-sync/) — MCP write tools 在 `--sync-wiki` 启用时自动刷新 Vault projection。已合入 PR #61。
- [outbox-consumer-cursors/](outbox-consumer-cursors/) — Consumer-scoped outbox cursor export API plus CLI/consumer cutover。已合入 PR #62/#63。
- [multi-process-writer-lease/](multi-process-writer-lease/) — `wiki.db.writer.lock` writer lease for write CLI/MCP entrypoints。已合入 PR #64。
- [reliability-test-matrix/](reliability-test-matrix/) — DB failure injection、大 snapshot smoke、MCP malformed input、LLM bad JSON 回归矩阵。已合入 PR #65。
- [dependency-audit-automation/](dependency-audit-automation/) — Scheduled/manual `cargo audit` lane with artifact upload, kept out of quick PR checks。已合入 PR #66。
- [scheduled-vault-reports/](scheduled-vault-reports/) — Automation `vault-reports` job for timestamped Vault report bundles, latest pointers, and retention。已合入 PR #67。
- [notion-archived-source-retirement/](notion-archived-source-retirement/) — DB-first Notion archived source retirement audit/plan plus guarded apply。已合入 PR #68/#69。
- [benchmark-reproducibility/](benchmark-reproducibility/) — `rust-mempalace bench --mode random --seed` deterministic sampling and `benchmark_runs.seed` persistence。已合入 PR #70。
- [embedding-tx-atomicity/](embedding-tx-atomicity/) — vector-enabled source/claim writes commit snapshot + outbox + embedding rows in one SQLite transaction。已合入 PR #71。
- [contradiction-scan-scaling/](contradiction-scan-scaling/) — bounded contradiction candidate scan for `naive_contradiction_pairs`。已合入 PR #74。
- [semantic-fusion-benchmark/](semantic-fusion-benchmark/) — J14 LongMemEval query baseline vs semantic-fusion comparison lane。Active PR #75。
