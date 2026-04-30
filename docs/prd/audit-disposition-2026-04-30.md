# PRD: Audit Disposition 2026-04-30

## Summary

- Goal: 完成 Notion《wiki-mempalace 全方位代码审计报告》2026-04-30 处置决策中标记为“待修复”的 3 项。
- User value: 搜索默认质量、文档一致性、MCP API 可维护性回到可信状态。
- Success criteria: M-5/L-4/I-8 均由独立 PR 合入；M-2 接受风险，L-5/I-5/I-6 暂缓记录不丢。

## Plain-Language Product Scope

- User can: 继续把审计报告当后续治理真源，不需要逐项手动确认。
- User cannot: 期望本批处理接受风险或暂缓项。
- User decision needed: 无。2026-04-30 处置决策已经等同产品确认。

## Scope

In:

- M-5：MCP `wiki_query` 默认走 SQLite-backed search ports。
- L-4：修 active docs 的 edition / Notion sync 状态矛盾。
- I-8：刷新独立 MCP API reference。

Out:

- M-2 LLM Prompt Injection 深层重构。
- L-5 chrono/time 依赖统一。
- I-5 wiki pipeline benchmark。
- I-6 并发/压力/故障注入扩展。
- 任何生产 `/Users/mac-mini/Documents/wiki` 写操作。

## Modules

| Module | Goal | Owner area | Status |
| --- | --- | --- | --- |
| audit-disposition-01-mcp-query-storage-ports | MCP query 默认检索质量与 CLI query 对齐 | `crates/wiki-cli/src/mcp.rs` | Merged PR #95 |
| audit-disposition-02-doc-consistency | 修 active docs 矛盾 | docs | Merged PR #96 |
| audit-disposition-03-mcp-api-reference | API reference 与 tools/schema/current behavior 对齐 | docs + MCP tests | Merged PR #97 |

## Acceptance

- 每个模块有 spec 三件套、handoff、roadmap/spec index/LESSONS 回填。
- 每个 PR 本地 gate 和 GitHub quick 通过后合入。
- 最终 roadmap 将三项标为完成，暂缓项仍保留触发条件。

## Risks

- MCP query 记录 `QueryServed` 时会保存 snapshot；实现必须避免用 stale in-memory store 覆盖 repo。
- MCP API docs 容易与 `tools_list()` 漂移；PR3 必须加 docs sync test。

## Rollout

- Branch strategy: `codex/audit-disposition-XX-*`，每项一个 PR。
- PR sequence: PR1 M-5 -> PR2 L-4 -> PR3 I-8。
- Merge gate: local full gate + GitHub quick green。

## Status

- [x] PRD approved by 2026-04-30 disposition decision
- [x] Plain architecture approved by roadmap plan
- [x] Specs created for PR1
- [x] Specs created for PR2
- [x] Specs created for PR3
- [x] Modules implemented
- [x] CI green
- [x] Merged
- [x] Roadmap updated
