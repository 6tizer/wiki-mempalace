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
| MCP Vault Sync | ✅ 已合入 PR #61 | CR-01 延后项：MCP 写操作在 `--sync-wiki` 启用时自动触发 `write_projection`；projection 失败返回 typed `storage_error` |
| Outbox Consumer Cursors | 🚧 Phase 1 已合入 PR #62，cutover 待下一 PR | CR-01 延后项：consumer-scoped export API 已补；CLI/consumer cutover 仍按下一 PR 推进 |
| Embedding Tx Atomicity | 💤 未开始 | CR-01 延后项：`upsert_embedding` 纳入 snapshot+outbox 同一 SQLite transaction；需存储层改造 PRD |
| Benchmark Reproducibility | 💤 未开始 | CR-01 延后项：`rust-mempalace benchmark --mode random` 添加 `--seed` 参数并存入 `benchmark_runs`，使跨次 recall 可比；需独立配置 PRD |
| Notion Archived Source Retirement | 💤 未开始 | 待 PRD/spec；Notion 已归档 source 应同步退役到本地 DB/Vault。已知样本：`sources/wechat/微信公众号文章链接汇总.md`，Notion `is_archived=true`，本地仍在 `wiki.db.sources` 和 Vault 中 |
| Notion Incremental Sync | ✅ 已合入 | PR #36 / PR #38 / PR #42；`wiki-cli notion-sync`、automation `notion-sync` daily job 已实现；增量游标 `notion_sync_cursors`/`notion_page_index`；速率限制 350ms + 429 重试；`--refresh-existing` 刷新已有 source body/tags；`--writeback-notion` 接口完整默认关闭；PRD: `docs/prd/notion-incremental-sync.md` |
| Notion Source Vault Projection | ✅ 已合入并已跑生产 apply | PR #42 已 merge；`notion-sync` 已支持 Notion block 正文抓取、`--refresh-existing`、Obsidian-safe tag projection 和 automation 默认刷新；生产 refresh 覆盖 X 782 / WeChat 485 个窗口内页面，刷新 161 个已有 source，最终 `notion-source-vault-sync --dry-run --refresh-existing --repair-tags` 为 planned=0 / tags_rewritten=0；176 个 DB-backed Notion source 已投影到 `sources/x` / `sources/wechat` |
| Production Wiki Compiler | ✅ 已合入并已跑完 scale-up 闭环 | PR #44/#46/#47/#54/#56 已 merge；compiler 已支持 raw source -> resolver -> summary + concept/entity pages -> Vault projection -> Mempalace -> lint/audit；2026-04-28 全量小批量生产执行完成，remaining uncompiled = 0 |
| Compiler Canonicalization v2 | ✅ 已合入 | PR #47 已 merge；在 compiler draft 和 DB 写入之间插入 pre-write resolver；新增 bounded candidate retrieval、`wiki_canonical_alias` persisted alias/canonical mapping、small LLM fallback、machine-owned `deferred_resolutions` JSON；低置信度/模糊项不污染 active graph，交给后续 resolver/lint/fixer agent lane |
| Compiler Deferred Resolution Agent | ✅ 已合入并已跑生产 apply | PR #54 已 merge；`compiler-resolve-deferred` 机器-only 后置治理已实现并用于真实 production reports；apply 顺序为 DB -> Vault -> Mempalace -> lint/audit；最新复查 deferred dry-run 为 aliases=0 / creates=0 / changed=0，低置信 `keep_deferred` 保留不污染 active graph |
| Audit Report Hardening | ✅ 已合入 PR #58 | 按 Notion 全方位代码审计报告修复可落地项：MCP 10MiB 输入上限、LLM plan bounds、脱敏扩展、outbox batch transaction、FTS token quoting；剩余审计项已拆成独立 roadmap 条目 |
| Row-level Wiki State Storage | 💤 未开始 | 审计延后项：把 `wiki_state` 单行 JSON blob 迁移到 claims/pages/sources/entities/edges/audits 行级表，保留快照兼容和迁移/回滚故事；需独立存储迁移 PRD/spec |
| CLI Command Modularization | 💤 未开始 | 审计延后项：拆分 `wiki-cli/src/main.rs` 的子命令处理到 `commands/` 模块，降低 main.rs 规模；纯重构，需独立 PRD 和分阶段 review |
| MCP Typed Errors | ✅ 已合入 PR #60 | 审计延后项：`wiki-cli/src/mcp.rs` 已收敛为 typed `McpToolError` / JSON-RPC error mapping，输出稳定 `error.data.kind` |
| MCP API Reference | ✅ 已合入 PR #58 | PR #58 新增 `docs/mcp-api-reference.md`，覆盖统一 MCP Server 工具参数、scope 默认值、写入副作用、输入上限和错误形状 |
| Multi-process Write Guardrails | 💤 部分完成，待 lock/lease | PR #58 已补 README/AGENTS writer safety 警告；后续评估 advisory lock 或 writer lease |
| Contradiction Scan Scaling | 💤 未开始 | 审计延后项：优化 `naive_contradiction_pairs` O(n²) 路径，加入 stale 预过滤、scope 分桶、可选 embedding 预筛或 bounded candidates |
| Reliability Test Matrix | 💤 未开始 | 审计延后项：补并发写入、1w+ claims/pages 大数据量性能回归、MCP malformed/oversized/fuzz、LLM 畸形 JSON、DB lock/failure injection 测试 |
| Automation Integrity Check | ✅ 已合入 PR #58 | PR #58 已把 `PRAGMA integrity_check` 纳入 `automation health` 输出；scheduled report 保留策略仍属于 `Scheduled Vault Reports` |
| Dependency Audit Automation | 💤 未开始 | 审计建议：定期运行 `cargo audit` 或等价供应链检查，并把结果接入报告/CI 低频 job |
| Time Library Unification | 💤 未开始 | 审计低优先项：评估 `chrono` vs `time` 双时间库，长期优先统一到 `time`；若保留 mempalace 独立性，文档化边界 |
| Scheduled Vault Reports | 💤 未开始 | 待 PRD；把 `vault-audit`、`metrics`、`dashboard`、`automation health`、`suggest` 等报告接入定时生成和保留策略 |
| C16A Atomic snapshot + outbox | ✅ 已合入 | PR #25 已 merge；新增 `save_snapshot_and_append_outbox` 单事务持久化路径；CLI/MCP/backfill 写路径已切到原子提交 |
| C16B Embedding ANN index | 💤 未开始 | 仍保留在 [embedding-ann-index](specs/embedding-ann-index/)；可单独规划，不和存储一致性混在一个 PR |

## 当前下一阶段

> 本节是 PR #58 之后的后续路线，不是 PR #58 的完成范围。PR #58 只完成
> `Audit Report Hardening` 已列出的安全/可靠性小补丁。

### P0：Production Compiler scale-up（非新功能）

已完成。已全量执行小批量生产编译+固定检查链路（`batch-ingest`
→ `compiler-resolve-deferred --allow-create --apply` → `consume-to-mempalace` →
`lint` / `audit` → 重复 concept/entity + 断链 + Obsidian 抽查），目标验证已达成。

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

### P4：Reliability / Hardening

1. Multi-process Write Guardrails：PR #58 已写清限制；后续决定 advisory lock / writer lease。
2. Reliability Test Matrix：补并发、大数据、MCP fuzz、LLM/DB 故障注入。
3. Dependency Audit Automation：低频供应链检查，不拖慢 quick CI。

### P5：Embedding / Retrieval 线（同一方向，按阶段实现）

1. Embedding Tx Atomicity：先把 embedding 写入纳入 snapshot/outbox 事务。
2. C16B Embedding ANN index：再做 bounded-work vector search。
3. Contradiction Scan Scaling：优化 contradiction O(n²) 路径。
4. Row-level Wiki State Storage：解决 `wiki_state` blob 全量序列化瓶颈；该项影响存储 schema 和迁移，不能塞进 hardening PR。
5. J14 Semantic Fusion Benchmark：等 J13 报告证明语义不匹配是主因后再启动。
6. Benchmark Reproducibility：`--mode random` 加 `--seed`，可作为本线前置小 PR 或同批第一步。

### P6：DX / Maintainability

1. CLI Command Modularization：拆 `main.rs`，按命令域分模块。
2. MCP Typed Errors：先定义错误类型，再统一 JSON-RPC error mapping。
3. Time Library Unification：评估统一时间库和 crate 独立性边界。

### P7：M12 executor（最后）

M12 operator/executor 属于自动行动层。等 compiler、source 生命周期、outbox 和报告自动化稳定后再规划，避免自动放大错误。

## 审计剩余项 PR 计划

### PR #58 可继续收敛项

以下 3 项已在 PR #58 收敛，原因是改动面小、与 hardening 主题一致，且不会改变核心数据模型：

| 项目 | PR #58 内完成范围 | 边界 |
| --- | --- | --- |
| Automation Integrity Check | 把 `PRAGMA integrity_check` 接入 `automation health` 报告/退出状态。 | 不做 scheduled report 保留策略；那属于 `Scheduled Vault Reports`。 |
| Multi-process Write Guardrails | 补 README / AGENTS 显著警告：不要多个 `LlmWikiEngine` writer 同时写同一 DB。 | 不做 advisory lock / writer lease；那是后续独立 PR。 |
| MCP API Reference | 写一份基于现有 `tools/list` schema 的 MCP 工具 Markdown 参考。 | 不做 typed errors；typed errors 是后续独立 PR。 |

### PR #58 之后的拆分

PR #58 完成 hardening 小补丁后，剩余 12 个未完成/部分完成项按 17 个 PR 完成。原则：先写入一致性和运行安全，再做性能，再做治理/重构。

| 顺序 | PR | 覆盖项 | 原因 |
| --- | --- | --- | --- |
| 1 | MCP Typed Errors | `MCP Typed Errors` | 先把 MCP error 边界类型化，后续 MCP Vault Sync / API 文档可复用错误语义。 |
| 2 | MCP Vault Sync | `MCP Vault Sync` | MCP 写操作后触发 Vault projection；行为边界清晰，但会改变写入副作用，独立 PR。 |
| 3 | Outbox Consumer Cursors schema/API | `Outbox Consumer Cursors` | 先新增 consumer-scoped cursor 表/API，不改变消费行为。 |
| 4 | Outbox Consumer Cursors cutover | `Outbox Consumer Cursors` | 再切 consumer/ack 语义，降低协议迁移风险。 |
| 5 | Embedding Tx Atomicity | `Embedding Tx Atomicity` | 先把 embedding 写入纳入 snapshot/outbox 事务，再谈 ANN。 |
| 6 | Embedding ANN spike / feature gate | `C16B Embedding ANN index` | 先确定 sqlite-vec/ANN 技术、feature gate、CI/release story。 |
| 7 | Embedding ANN implementation | `C16B Embedding ANN index` | 实现 upsert/search/fallback/re-rank；与 spike 分开 review。 |
| 8 | Contradiction Scan Scaling | `Contradiction Scan Scaling` | 优化 contradiction O(n²) 路径，独立做性能/语义回归。 |
| 9 | Multi-process Write Lock / Lease | `Multi-process Write Guardrails` | PR #58 已完成文档警告；后续若需要强制保护，再做 advisory lock / writer lease。 |
| 10 | Reliability Test Matrix | `Reliability Test Matrix` | 补并发、大数据、MCP fuzz、LLM/DB 故障注入测试，为后续迁移兜底。 |
| 11 | Dependency Audit Automation | `Dependency Audit Automation` | 增加低频供应链检查，不拖慢 quick CI。 |
| 12 | Scheduled Vault Reports | `Scheduled Vault Reports` | 定时生成 vault-audit/metrics/dashboard/health/suggest 报告；可包含 automation health 历史保留。 |
| 13 | Notion Archived Source Retirement audit/plan | `Notion Archived Source Retirement` | 先只做 archived source audit/plan，确保 DB-first 退役模型正确。 |
| 14 | Notion Archived Source Retirement apply | `Notion Archived Source Retirement` | 再做 apply/production docs，避免手删 Markdown。 |
| 15 | CLI Command Modularization phase 1 | `CLI Command Modularization` | 先拆低风险命令域，减少 `main.rs` 体积，不改行为。 |
| 16 | CLI Command Modularization phase 2 | `CLI Command Modularization` | 再拆主 dispatcher / shared config，单独 review。 |
| 17 | Row-level Wiki State Storage migration/dual-write | `Row-level Wiki State Storage` | 最大存储迁移项；先加行级 schema、迁移、dual-read/write。 |
| 18 | Row-level Wiki State Storage cutover/cleanup | `Row-level Wiki State Storage` | 再切读写路径并清理兼容层；需完整回滚故事。 |
| 19 | Time Library Unification | `Time Library Unification` | 低优先依赖统一，最后做，避免影响前面功能/迁移。 |

> 备注：表中仍保留 19 个顺序号以稳定引用；其中 `MCP API Reference` 与 `Automation Integrity Check` 已由 PR #58 完成，后续实际剩余为 17 个 PR。

执行计划见 [automation-issue-batch-3.md](automation-issue-batch-3.md)。开发流程见
[dev-workflow.md](dev-workflow.md)，batch-3 PRD 见 [prd/batch-3.md](prd/batch-3.md)。

## 不再重复开发

- `mempalace_*` MCP 工具已经通过 `wiki_mempalace_bridge::make_tools` 访问 bridge。
- outbox ack 已经以 `wiki_outbox_consumer_progress(consumer_tag, acked_up_to_id, acked_at)` 为消费者进度真源。
- `consume-to-mempalace --palace` 的 live bank 已由 `--viewer-scope` 派生。
- `--graph-extras-file` 已按 viewer scope 过滤 wiki doc id，只允许 `mp_drawer:` / `mp_kg:` 外部 id。
- `write_projection` 已清理带合法 page-id frontmatter 的 stale managed page。
