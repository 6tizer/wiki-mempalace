# Current Roadmap

本文是当前计划源。历史总计划和批次 issue 已归档到 [archive/](archive/README.md)。

## 状态总览

| 模块 | 状态 | 当前证据 |
| --- | --- | --- |
| M1 调度编排层 | ✅ 已完成 | `wiki-cli automation list-jobs/run/run-daily`、固定 job registry、dry-run、失败短路 |
| M2 运行状态与心跳 | ✅ 已完成 | `wiki_automation_run`、`AutomationHeartbeat`、`automation status/doctor` |
| M3 告警与运维出口 | ✅ 已完成 | `automation health/last-failures`、green/yellow/red、stderr alert、阈值环境变量 |
| M4 Outbox 闭环增强 | ✅ 已完成 | `docs/outbox-event-matrix.md`、bridge dispatch stats、active/ignored/unresolved 统计 |
| M5 恢复与回滚 | ✅ 已完成 | `automation verify-restore`、`scripts/recovery-drill.sh`、runbook、CI smoke |
| M6 Gap 工作流 | ✅ 已完成 | `wiki-cli gap`、`GapFinding`、missing_xref/low_coverage/orphan_source、报告与 page 写入 |
| M7 Fixer 工作流 | ✅ 已完成 | `wiki-cli fix`、`FixAction`、lint/gap finding 映射、低风险 auto fix |
| M8 消费链产品化 | ✅ 已完成 | `PageContract`、`finalize_consumed_page`、`qa`/`synthesis`、统一 entry_type/status 骨架 |
| M9 查询融合增强 | ✅ 已完成 | `query/explain --palace-db`、`MempalaceSearchPorts`、`CompositeSearchPorts`、scope 过滤与去重 |
| M10 指标与评估 | ✅ 已合入 | PR #12 已 merge；`wiki-cli metrics` 已实现；支持 `--consumer-tag`、`--low-coverage-threshold`、`--json`、`--report <PATH>`；覆盖 content/lint/gaps/outbox/lifecycle 5 组指标 |
| M11 运维控制台 | ✅ 已合入 | PR #14 已 merge；`wiki-cli dashboard` 已实现；默认输出 `wiki/reports/dashboard.html`，支持 `--output <PATH>`、`--consumer-tag <TAG>`、`--low-coverage-threshold <N>`；生成静态自包含 HTML；默认只读 |
| M12 策略层增强 | ✅ 已合入 | PR #16 已 merge；`wiki-cli suggest` 已实现；支持文本、`--json`、`--report-dir [PATH]`；timestamped JSON 为真源、Markdown 为同源人读视图；默认只读，不执行 supersede/crystallize/fix 写入 |
| Schema T2 tag governance | ✅ 已合入 | PR #13 已 merge；`Claim/Source/LlmClaimDraft` tags、tag normalize/validate、deprecated_tags 拦截、max_new_tags_per_ingest 限流、CLI/MCP/batch ingest tags 已实现 |
| J13 LongMemEval auto benchmark | ✅ 已合入 | PR #19 已 merge；`rust-mempalace` 本地检索基线 runner、fetch/cache script、nightly/weekly workflow、30 天 artifact、fixture tests、review handoff 已实现；不进 PR 必跑 CI |
| Vault Backfill + Palace Init | ✅ 已合入并已跑生产初始化 | PR #23 已 merge；`vault-audit`、`vault-backfill`、`palace-init`、MCP `shared:wiki` runtime defaults 已实现；2026-04-25 已对 `/Users/mac-mini/Documents/wiki` 完成生产 backfill + palace init |
| B5 Orphan Governance | 🛠 Follow-up in progress | PR #28 已 merge；当前分支 `codex/orphan-governance-followup` 正在补 timestamped audit、LLM plan、中文报告、白名单 apply |
| DB/Vault/Palace Consistency Governance | 🛠 PR #32 in review | 分支 `codex/db-vault-palace-consistency`；已真实 apply 到 `/Users/mac-mini/Documents/wiki`，最终 plan 可执行动作 0，Vault 无新 pages 文件，Mempalace 缺失 page drawer 0 |
| Scheduled Vault Reports | 💤 未开始 | 待 PRD；把 `vault-audit`、`metrics`、`dashboard`、`automation health`、`suggest` 等报告接入定时生成和保留策略 |
| C16A Atomic snapshot + outbox | ✅ 已合入 | PR #25 已 merge；新增 `save_snapshot_and_append_outbox` 单事务持久化路径；CLI/MCP/backfill 写路径已切到原子提交 |
| C16B Embedding ANN index | 💤 未开始 | 仍保留在 [embedding-ann-index](specs/embedding-ann-index/)；可单独规划，不和存储一致性混在一个 PR |

## 当前下一阶段

1. DB/Vault/Palace Consistency Governance：新增三层一致性审计、计划、dry-run/apply，确保 DB 是原点，Vault/Mempalace 只通过程序路径修复。
2. C16B Embedding ANN index 如需推进，单独从 PRD/spec 开新分支。
3. 观察 J13 scheduled artifacts：先积累至少 7 份 nightly report 和 1 份 weekly full report，确认 artifact 稳定和 full run 真实耗时。
4. J14 Semantic Fusion Benchmark：只有在 J13 报告显示同义表达/词面不匹配是主要错因，且运行预算明确后，再评估 `wiki-cli --vectors --palace-db` 语义融合 lane。
5. M12 后续 operator/executor、dashboard latest suggestion report、QueryServed scope/hash schema 改进单独规划，不混入首版 suggest。
6. Scheduled Vault Reports：新增定时报告流水线 PRD，明确哪些报告由 cron/automation 生成、生成频率、输出目录、latest 指针和历史保留/清理策略。

执行计划见 [automation-issue-batch-3.md](automation-issue-batch-3.md)。开发流程见
[dev-workflow.md](dev-workflow.md)，batch-3 PRD 见 [prd/batch-3.md](prd/batch-3.md)。

## 不再重复开发

- `mempalace_*` MCP 工具已经通过 `wiki_mempalace_bridge::make_tools` 访问 bridge。
- outbox ack 已经以 `wiki_outbox_consumer_progress(consumer_tag, acked_up_to_id, acked_at)` 为消费者进度真源。
- `consume-to-mempalace --palace` 的 live bank 已由 `--viewer-scope` 派生。
- `--graph-extras-file` 已按 viewer scope 过滤 wiki doc id，只允许 `mp_drawer:` / `mp_kg:` 外部 id。
- `write_projection` 已清理带合法 page-id frontmatter 的 stale managed page。
