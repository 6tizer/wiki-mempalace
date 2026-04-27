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
| B5 Orphan Governance | ✅ 已合入并已跑生产 apply | PR #28 / PR #30 已 merge；`vault-audit` timestamped 报告、LLM plan、中文报告、白名单 apply 已实现；生产 vault 已真实跑过 |
| DB/Vault/Palace Consistency Governance | ✅ 已合入并已跑生产 apply | PR #32 已 merge；已真实 apply 到 `/Users/mac-mini/Documents/wiki`，最终 plan 可执行动作 0，Vault 无新 pages 文件，Mempalace 缺失 page drawer 0 |
| CR-01 Code Review Fixes | ✅ 已合入 | PR #34 已 merge；修复快照序列化确定性、SourceIngested unresolved 语义、flush_outbox drain 精度、save_snapshot 事务、notion_uuid 锚定提取、url_index 重复 URL、benchmark hits 真实存储；4 项延后 follow-up 已登记 roadmap |
| MCP Vault Sync | 💤 未开始 | CR-01 延后项：`wiki-cli/src/mcp.rs` `_wiki_dir` 实际 projection 集成，写操作后自动触发 `write_projection`；需独立 PRD 和接口设计 |
| Outbox Consumer Cursors | 💤 未开始 | CR-01 延后项：outbox 消费者游标 / at-exactly-once 语义；`export_outbox_ndjson_from_id` 添加 consumer-scoped cursor 表，防止重复派发；需独立 outbox-v2 PRD |
| Embedding Tx Atomicity | 💤 未开始 | CR-01 延后项：`upsert_embedding` 纳入 snapshot+outbox 同一 SQLite transaction；需存储层改造 PRD |
| Benchmark Reproducibility | 💤 未开始 | CR-01 延后项：`rust-mempalace benchmark --mode random` 添加 `--seed` 参数并存入 `benchmark_runs`，使跨次 recall 可比；需独立配置 PRD |
| Notion Archived Source Retirement | 💤 未开始 | 待 PRD/spec；Notion 已归档 source 应同步退役到本地 DB/Vault。已知样本：`sources/wechat/微信公众号文章链接汇总.md`，Notion `is_archived=true`，本地仍在 `wiki.db.sources` 和 Vault 中 |
| Notion Incremental Sync | ✅ 已合入 | PR #36 / PR #38 / PR #42；`wiki-cli notion-sync`、automation `notion-sync` daily job 已实现；增量游标 `notion_sync_cursors`/`notion_page_index`；速率限制 350ms + 429 重试；`--refresh-existing` 刷新已有 source body/tags；`--writeback-notion` 接口完整默认关闭；PRD: `docs/prd/notion-incremental-sync.md` |
| Notion Source Vault Projection | ✅ 已合入并已跑生产 apply | PR #42 已 merge；`notion-sync` 已支持 Notion block 正文抓取、`--refresh-existing`、Obsidian-safe tag projection 和 automation 默认刷新；生产 refresh 覆盖 X 782 / WeChat 485 个窗口内页面，刷新 161 个已有 source，最终 `notion-source-vault-sync --dry-run --refresh-existing --repair-tags` 为 planned=0 / tags_rewritten=0；176 个 DB-backed Notion source 已投影到 `sources/x` / `sources/wechat` |
| Production Wiki Compiler | ✅ 已合入，scale-up 已开始 | PR #44/#46/#47/#54 已 merge；compiler 已支持 raw source -> resolver -> summary + concept/entity pages -> Vault projection -> Mempalace -> lint/audit；2026-04-27 已跑两轮真实 10-source 批次，均 success=10 / failed=0；剩余未编译 source 132 条 |
| Compiler Canonicalization v2 | ✅ 已合入 | PR #47 已 merge；在 compiler draft 和 DB 写入之间插入 pre-write resolver；新增 bounded candidate retrieval、`wiki_canonical_alias` persisted alias/canonical mapping、small LLM fallback、machine-owned `deferred_resolutions` JSON；低置信度/模糊项不污染 active graph，交给后续 resolver/lint/fixer agent lane |
| Compiler Deferred Resolution Agent | ✅ 已合入并已跑生产 apply | PR #54 已 merge；`compiler-resolve-deferred` 机器-only 后置治理已实现并用于真实 production reports；apply 顺序为 DB -> Vault -> Mempalace -> lint/audit；最新复查 deferred dry-run 为 aliases=0 / creates=0 / changed=0，低置信 `keep_deferred` 保留不污染 active graph |
| Scheduled Vault Reports | 💤 未开始 | 待 PRD；把 `vault-audit`、`metrics`、`dashboard`、`automation health`、`suggest` 等报告接入定时生成和保留策略 |
| C16A Atomic snapshot + outbox | ✅ 已合入 | PR #25 已 merge；新增 `save_snapshot_and_append_outbox` 单事务持久化路径；CLI/MCP/backfill 写路径已切到原子提交 |
| C16B Embedding ANN index | 💤 未开始 | 仍保留在 [embedding-ann-index](specs/embedding-ann-index/)；可单独规划，不和存储一致性混在一个 PR |

## 当前下一阶段

### P0：Production Compiler scale-up（先做，非新功能）

继续用小批量真实编译剩余 source。每批后固定跑 deferred resolver/fixer、Vault
projection、Mempalace consume、lint/audit、重复 concept/entity 检查和 Obsidian
抽查。不要在这一阶段混入新功能开发；scale-up 的目标是暴露真实质量问题。

### P1：Source 生命周期治理（同一批规划，分 PR 实现）

1. Notion Archived Source Retirement：同步 Notion archived 状态，生成退役 plan，通过 DB 原点更新 + Vault 投影清理处理，不手工删除 Markdown。
2. MCP Vault Sync：MCP 写操作后自动触发 Vault projection；和 archived retirement 同属 DB -> Vault 生命周期治理，但建议独立 PR。

### P2：报告自动化（低风险独立批次）

Scheduled Vault Reports：把 `vault-audit`、`metrics`、`dashboard`、
`automation health`、`suggest` 接入定时生成、latest 指针、输出目录和历史保留/清理策略。
`dashboard latest suggestion report` 可放进同一批次。

### P3：Outbox 消费语义（独立批次）

Outbox Consumer Cursors：新增 consumer-scoped cursor / at-exactly-once 语义，
减少重复派发。该项触碰 outbox 协议和 Mempalace 消费边界，不和 reporting 或 compiler
scale-up 混做。

### P4：Embedding / Retrieval 线（同一方向，按阶段实现）

1. Embedding Tx Atomicity：先把 embedding 写入纳入 snapshot/outbox 事务。
2. C16B Embedding ANN index：再做 bounded-work vector search。
3. J14 Semantic Fusion Benchmark：等 J13 报告证明语义不匹配是主因后再启动。
4. Benchmark Reproducibility：`--mode random` 加 `--seed`，可作为 P4 前置小 PR 或同批第一步。

### P5：M12 executor（最后）

M12 operator/executor 属于自动行动层。等 compiler、source 生命周期、outbox 和报告自动化稳定后再规划，避免自动放大错误。

执行计划见 [automation-issue-batch-3.md](automation-issue-batch-3.md)。开发流程见
[dev-workflow.md](dev-workflow.md)，batch-3 PRD 见 [prd/batch-3.md](prd/batch-3.md)。

## 不再重复开发

- `mempalace_*` MCP 工具已经通过 `wiki_mempalace_bridge::make_tools` 访问 bridge。
- outbox ack 已经以 `wiki_outbox_consumer_progress(consumer_tag, acked_up_to_id, acked_at)` 为消费者进度真源。
- `consume-to-mempalace --palace` 的 live bank 已由 `--viewer-scope` 派生。
- `--graph-extras-file` 已按 viewer scope 过滤 wiki doc id，只允许 `mp_drawer:` / `mp_kg:` 外部 id。
- `write_projection` 已清理带合法 page-id frontmatter 的 stale managed page。
