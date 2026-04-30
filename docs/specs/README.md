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

- [ai-provider-profiles/](ai-provider-profiles/) — PR1 for Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3: LLM profile resolver plus Exa/Tavily web search runtime. Merged PR #101.
- [wiki-governance-scan/](wiki-governance-scan/) — PR2 for Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3: read-only lifecycle/reference/lint/duplicate/retire/synthesis-signal scan. Merged PR #102.
- [evidence-fixer-plan/](evidence-fixer-plan/) — PR3 for Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3: typed dry-run fixer plan from governance scan evidence. Merged PR #103.
- [evidence-fixer-apply-restore/](evidence-fixer-apply-restore/) — PR4 for Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3: evidence-auto apply, tombstones, and restore. Merged PR #104.
- [synthesis-discovery/](synthesis-discovery/) — PR5 for Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3: point/line/plane/body candidate discovery from governance scan signals. Merged PR #105.
- [web-backed-synthesis-composer/](web-backed-synthesis-composer/) — PR6 for Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3: internal evidence + dual web evidence + LLM composition/verification. Merged PR #106.
- [governance-automation-docs/](governance-automation-docs/) — PR7 for Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3: automation jobs, daily/manual lanes, MCP boundary, Notion capability matrix, and docs closeout. Merged PR #107.
- [m10-metrics/](m10-metrics/) — M10 unified metrics core. Merged PR #12。
- [m11-dashboard/](m11-dashboard/) — M11 read-only dashboard/report. Merged PR #14。
- [m12-strategy/](m12-strategy/) — M12 strategy suggestions. Merged PR #16。
- [schema-t2-tags/](schema-t2-tags/) — Schema T2 tag governance. Merged PR #13。
- [longmemeval-auto/](longmemeval-auto/) — J13 LongMemEval `rust-mempalace` local retrieval baseline artifacts. Merged PR #19。
- [vault-report-paths/](vault-report-paths/) — Vault-relative report output paths for dashboard, suggest, metrics, and automation health. Merged PR #22。
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
- [semantic-fusion-benchmark/](semantic-fusion-benchmark/) — J14 LongMemEval query baseline vs semantic-fusion comparison lane。已合入 PR #75。
- [row-level-wiki-state/](row-level-wiki-state/) — row-level `wiki_state` migration/dual-write while keeping blob compatibility。已合入 PR #76。
- [row-level-wiki-state-cutover/](row-level-wiki-state-cutover/) — row-primary snapshot read path with blob fallback and verification API。已合入 PR #77。
- [cli-command-modularization-phase1/](cli-command-modularization-phase1/) — low-risk `wiki-cli` command handler extraction into `commands/`。已合入 PR #78。
- [cli-command-modularization-phase2/](cli-command-modularization-phase2/) — no-engine dispatcher and shared runtime setup extraction with CLI smoke coverage。已合入 PR #79。
- [time-library-unification/](time-library-unification/) — replace remaining `chrono` timestamp usage with `time` across local crates。已合入 PR #80。
- [m12-executor-dry-run-planner/](m12-executor-dry-run-planner/) — derive typed dry-run executor plans from M12 `StrategyReport`。已合入 PR #81。
- [m12-executor-guarded-apply/](m12-executor-guarded-apply/) — guarded apply for M12 executor plans with explicit allowlist。已合入 PR #82。
- [audit-v2-01-mcp-input-boundary/](audit-v2-01-mcp-input-boundary/) — Audit Report Follow-up v2 PR 01: MCP limit clamp and lint report path traversal guard。已合入 PR #83。
- [audit-v2-02-mcp-scope-capability/](audit-v2-02-mcp-scope-capability/) — Audit Report Follow-up v2 PR 02: MCP write scope capability and supersede visibility guard。已合入 PR #84。
- [audit-v2-03-mempalace-bank-capability/](audit-v2-03-mempalace-bank-capability/) — Audit Report Follow-up v2 PR 03: derive mempalace bank from MCP viewer scope and scope KG reads。已合入 PR #85。
- [audit-v2-04-cross-bank-dedupe/](audit-v2-04-cross-bank-dedupe/) — Audit Report Follow-up v2 PR 04: drawer dedupe by `(bank_id, content_hash)`。已合入 PR #86。
- [audit-v2-05-queryserved-privacy/](audit-v2-05-queryserved-privacy/) — Audit Report Follow-up v2 PR 05: QueryServed hash/scope/schema privacy。已合入 PR #87。
- [audit-v2-06-llm-governance/](audit-v2-06-llm-governance/) — Audit Report Follow-up v2 PR 06: wiki-cli LLM env keys, untrusted prompt boundary, limits, redaction, and provider allowlist。已合入 PR #88。
- [audit-v2-07-ci-hardening/](audit-v2-07-ci-hardening/) — Audit Report Follow-up v2 PR 07: required quick CI now includes clippy and cargo-deny policy checks。已合入 PR #89。
- [audit-v2-08-production-searchports/](audit-v2-08-production-searchports/) — Audit Report Follow-up v2 PR 08: query/explain default to storage-backed wiki search ports with mempalace composition and in-memory fallback。已合入 PR #90。
- [audit-v2-09-cjk-unicode-retrieval/](audit-v2-09-cjk-unicode-retrieval/) — Audit Report Follow-up v2 PR 09: Unicode FTS tokens plus bounded CJK LIKE fallback for `rust-mempalace` retrieval。已合入 PR #91。
- [audit-v2-10-outbox-sqlite-reliability/](audit-v2-10-outbox-sqlite-reliability/) — Audit Report Follow-up v2 PR 10: per-consumer outbox ack count plus SQLite busy timeout and transaction wrapper hardening。已合入 PR #92。
- [audit-v2-11-vault-docs-hardening/](audit-v2-11-vault-docs-hardening/) — Audit Report Follow-up v2 PR 11: Vault projection managed marker/quarantine, docs consistency, and scheduled hardening lane。已合入 PR #93。
- [audit-disposition-01-mcp-query-storage-ports/](audit-disposition-01-mcp-query-storage-ports/) — Audit Disposition PR 01: MCP `wiki_query` defaults to storage-backed wiki search ports。已合入 PR #95。
- [audit-disposition-02-doc-consistency/](audit-disposition-02-doc-consistency/) — Audit Disposition PR 02: active docs edition / Notion sync consistency cleanup。已合入 PR #96。
- [audit-disposition-03-mcp-api-reference/](audit-disposition-03-mcp-api-reference/) — Audit Disposition PR 03: MCP API reference refresh plus docs sync tests。已合入 PR #97。
- [next-three-closeout-2026-04-30/](next-three-closeout-2026-04-30/) — Stale branch/docs cleanup, read-only row-state production validation, and scheduled hardening observation。已合入 PR #99。
