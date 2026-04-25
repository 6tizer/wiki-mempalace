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
- [vault-audit/](vault-audit/) — B1 read-only vault audit and report model. Active branch `codex/vault-backfill-palace-init`。
- [vault-backfill/](vault-backfill/) — B2 stable IDs and vault-to-`wiki.db` backfill. Active branch `codex/vault-backfill-palace-init`。
- [palace-init/](palace-init/) — B3 `palace.db` initialization from wiki outbox and fusion validation. Active branch `codex/vault-backfill-palace-init`。
- [agent-runtime-defaults/](agent-runtime-defaults/) — B4 shared vault-local CLI/MCP defaults. Active branch `codex/vault-backfill-palace-init`。
- [orphan-governance/](orphan-governance/) — B5 read-only orphan governance report from production `vault-audit.json`. Active branch `codex/orphan-governance`。
- [persist-snapshot-outbox/](persist-snapshot-outbox/) — C16a: `wiki_state` + outbox append in one SQLite transaction; replaces split `save_to_repo` / `flush_outbox` autocommit for crash-safety. Merged PR #25. PRD: [storage-embeddings-followup.md](../prd/storage-embeddings-followup.md).
- [embedding-ann-index/](embedding-ann-index/) — C16b: bounded-work vector search for `wiki_embedding` (optional extension / ANN; fallback to full scan). PRD: [storage-embeddings-followup.md](../prd/storage-embeddings-followup.md)。
