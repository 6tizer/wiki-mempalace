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
| J14 Semantic Fusion Benchmark | ✅ PR #75 | 在 J13 LongMemEval lane 上增加 query baseline vs semantic fusion 对照；低分不阻断 CI |
| Vault Backfill + Palace Init | ✅ 已合入并已跑生产初始化 | PR #23 已 merge；`vault-audit`、`vault-backfill`、`palace-init`、MCP `shared:wiki` runtime defaults 已实现；2026-04-25 已对 `/Users/mac-mini/Documents/wiki` 完成生产 backfill + palace init |
| B5 Orphan Governance | ✅ 已合入并已跑生产 apply | PR #28 / PR #30 已 merge；`vault-audit` timestamped 报告、LLM plan、中文报告、白名单 apply 已实现；生产 vault 已真实跑过 |
| DB/Vault/Palace Consistency Governance | ✅ 已合入并已跑生产 apply | PR #32 已 merge；已真实 apply 到 `/Users/mac-mini/Documents/wiki`，最终 plan 可执行动作 0，Vault 无新 pages 文件，Mempalace 缺失 page drawer 0 |
| CR-01 Code Review Fixes | ✅ 已合入 | PR #34 已 merge；修复快照序列化确定性、SourceIngested unresolved 语义、flush_outbox drain 精度、save_snapshot 事务、notion_uuid 锚定提取、url_index 重复 URL、benchmark hits 真实存储；4 项延后 follow-up 已登记 roadmap |
| MCP Vault Sync | ✅ 已合入 PR #61 | CR-01 延后项：MCP 写操作在 `--sync-wiki` 启用时自动触发 `write_projection`；projection 失败返回 typed `storage_error` |
| Outbox Consumer Cursors | ✅ 已合入 PR #62/#63 | CR-01 延后项：consumer-scoped export API 已补；`export-outbox-ndjson-from` / `consume-to-mempalace` 已切到 consumer cursor 默认语义，`--last-id` 保留为 legacy/manual floor |
| Embedding Tx Atomicity | ✅ PR #71 | CR-01 延后项：`upsert_embedding` 纳入 snapshot+outbox 同一 SQLite transaction |
| Benchmark Reproducibility | ✅ PR #70 | CR-01 延后项：`rust-mempalace bench --mode random` 添加 `--seed` 参数并存入 `benchmark_runs`，使跨次 recall 可比 |
| Notion Archived Source Retirement | ✅ 已合入 PR #68/#69 | PR #68 完成 audit/plan；PR #69 完成 guarded apply：只处理 `apply_safe=true`，DB source + `notion_page_index` 同事务退役，再按 frontmatter 身份删除 Vault source 文件 |
| Notion Incremental Sync | ✅ 已合入 | PR #36 / PR #38 / PR #42；`wiki-cli notion-sync`、automation `notion-sync` daily job 已实现；增量游标 `notion_sync_cursors`/`notion_page_index`；速率限制 350ms + 429 重试；`--refresh-existing` 刷新已有 source body/tags；`--writeback-notion` 接口完整默认关闭；PRD: `docs/prd/notion-incremental-sync.md` |
| Notion Source Vault Projection | ✅ 已合入并已跑生产 apply | PR #42 已 merge；`notion-sync` 已支持 Notion block 正文抓取、`--refresh-existing`、Obsidian-safe tag projection 和 automation 默认刷新；生产 refresh 覆盖 X 782 / WeChat 485 个窗口内页面，刷新 161 个已有 source，最终 `notion-source-vault-sync --dry-run --refresh-existing --repair-tags` 为 planned=0 / tags_rewritten=0；176 个 DB-backed Notion source 已投影到 `sources/x` / `sources/wechat` |
| Production Wiki Compiler | ✅ 已合入并已跑完 scale-up 闭环 | PR #44/#46/#47/#54/#56 已 merge；compiler 已支持 raw source -> resolver -> summary + concept/entity pages -> Vault projection -> Mempalace -> lint/audit；2026-04-28 全量小批量生产执行完成，remaining uncompiled = 0 |
| Compiler Canonicalization v2 | ✅ 已合入 | PR #47 已 merge；在 compiler draft 和 DB 写入之间插入 pre-write resolver；新增 bounded candidate retrieval、`wiki_canonical_alias` persisted alias/canonical mapping、small LLM fallback、machine-owned `deferred_resolutions` JSON；低置信度/模糊项不污染 active graph，交给后续 resolver/lint/fixer agent lane |
| Compiler Deferred Resolution Agent | ✅ 已合入并已跑生产 apply | PR #54 已 merge；`compiler-resolve-deferred` 机器-only 后置治理已实现并用于真实 production reports；apply 顺序为 DB -> Vault -> Mempalace -> lint/audit；最新复查 deferred dry-run 为 aliases=0 / creates=0 / changed=0，低置信 `keep_deferred` 保留不污染 active graph |
| Audit Report Hardening | ✅ 已合入 PR #58 | 按 Notion 全方位代码审计报告修复可落地项：MCP 10MiB 输入上限、LLM plan bounds、脱敏扩展、outbox batch transaction、FTS token quoting；剩余审计项已拆成独立 roadmap 条目 |
| Row-level Wiki State Storage | ✅ PR #76/#77 | 审计延后项：已增加 `wiki_state_row` row-level mirror、snapshot 双写、row-primary 读路径、blob fallback 和验证/恢复说明；blob 兼容层保留到生产验证后再移除 |
| CLI Command Modularization | ✅ PR #78/#79 | 审计延后项：PR #78 先抽 `schema-validate`、`llm-smoke`、outbox export/ack 到 `commands/`；PR #79 拆 no-engine dispatcher/shared runtime setup 并补 CLI smoke |
| MCP Typed Errors | ✅ 已合入 PR #60 | 审计延后项：`wiki-cli/src/mcp.rs` 已收敛为 typed `McpToolError` / JSON-RPC error mapping，输出稳定 `error.data.kind` |
| MCP API Reference | ✅ 已合入 PR #58 | PR #58 新增 `docs/mcp-api-reference.md`，覆盖统一 MCP Server 工具参数、scope 默认值、写入副作用、输入上限和错误形状 |
| Multi-process Write Guardrails | ✅ 已合入 PR #64 | PR #58 已补 writer safety 警告；PR #64 已增加 `wiki.db.writer.lock` writer lease，写入型 CLI/MCP 入口拿不到 lease 时 fail fast |
| Contradiction Scan Scaling | ✅ PR #74 | 已优化 `naive_contradiction_pairs`：stale 预过滤、scope 分桶、signal index、bounded candidates |
| Reliability Test Matrix | ✅ 已合入 PR #65 | 审计延后项：已补 DB batch rollback failure injection、大 snapshot smoke、MCP malformed parse、LLM malformed JSON extraction；writer lease busy / MCP oversized 已在相邻测试覆盖 |
| Automation Integrity Check | ✅ 已合入 PR #58 | PR #58 已把 `PRAGMA integrity_check` 纳入 `automation health` 输出；scheduled report 保留策略仍属于 `Scheduled Vault Reports` |
| Dependency Audit Automation | ✅ 已合入 PR #66 | 审计建议：已新增 scheduled/manual `cargo audit` workflow，上传 JSON/stderr artifact；quick CI 只做 wrapper 语法检查 |
| Time Library Unification | ✅ PR #80 | 审计低优先项：PR #80 统一剩余 `chrono::Utc` 当前时间格式化到 `time::OffsetDateTime`；不改变 RFC3339 字符串格式 |
| Scheduled Vault Reports | ✅ 已合入 PR #67 | 已新增 automation `vault-reports` job：timestamped bundle、`latest.json/latest.md` 指针、`WIKI_SCHEDULED_REPORT_KEEP` 保留策略 |
| C16A Atomic snapshot + outbox | ✅ 已合入 | PR #25 已 merge；新增 `save_snapshot_and_append_outbox` 单事务持久化路径；CLI/MCP/backfill 写路径已切到原子提交 |
| C16B Embedding ANN index | ✅ PR #72/#73 | `ann-embed` feature gate、locality-bucket bounded search、fallback full scan、ranking 回归已完成 |
| M12 Executor | ✅ PR #81/#82 | `suggest --executor-plan` dry-run planner 与 `suggest-executor-apply` guarded apply 已完成；默认不执行写入，apply 需 plan + allowlist + explicit flag |
| Audit Report Follow-up v2 | 🧭 已拆解 | 2026-04-29 对 Notion《wiki-mempalace 全方位代码审计报告》复核后，仍有安全、检索、outbox、Vault projection、CI/test/docs 未完成项；建议按 11 个独立 PR 完成，见下方“2026-04-29 审计复核新增剩余项” |

## 当前下一阶段

> 本节是 PR #58 之后的后续路线，不是 PR #58 的完成范围。PR #58 只完成
> `Audit Report Hardening` 已列出的安全/可靠性小补丁。

## 2026-04-29 审计复核新增剩余项

来源：Notion《wiki-mempalace 全方位代码审计报告》2026-04-29 状态表 + 当前代码复核。
已在本 roadmap 标完成且代码证据匹配的项不重复登记；以下只拆仍未完成、部分完成或文档未确认的项。

建议按 **11 个 PR** 推进。P0 先做 1-4；P1 做 5-7；P2 做 8-10；P3 做 11。

| 顺序 | PR 主题 | 状态 | 优先级 | 覆盖审计项 | 范围边界 | 验收条件 | 依赖 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | MCP input boundary quick fix | ✅ PR #83 | P0 | H-5 / H-3 | MCP `limit` / `per_stream_limit` / `search_limit` clamp；`write_lint_report` 路径遍历拒绝。 | schema + handler 双层限幅；checked conversion；`../evil.md`、绝对路径、reports symlink、file symlink、超大 limit 测试通过。 | 无 |
| 2 | MCP scope capability hardening | ✅ PR #84 | P0 | H-1 / I-3 | `resolve_write_scope` 不再允许任意覆盖 server viewer；`supersede` 加 viewer 可见性校验。 | 写工具只能用 server-side viewer/capability 或合法子 scope；跨 scope supersede、write-page/crystallize mismatch、maintenance hidden-claim mutation 拒绝/隔离测试通过。 | PR 1 |
| 3 | Mempalace bank capability | ✅ PR #85 | P0 | H-2 / I-3 | MCP mempalace 工具禁止 client 传 `bank_id`；由 `viewer_scope -> bank_id` 派生；KG query/timeline/stats 加 bank filter。 | 搜索、wake_up、taxonomy、traverse、reflect、KG 查询均按 bank 隔离；越权测试通过。 | PR 2 |
| 4 | Cross-bank drawer dedupe migration | ✅ PR #86 | P0 | M-7 | `drawers(content_hash)` 全局唯一改为 `(bank_id, content_hash)`；live sink / mine path 查重同步改造。 | migration 可重复执行；相同内容不同 bank 可共存；同 bank 仍去重；bridge/live tests 通过。 | PR 3 |
| 5 | QueryServed privacy/schema | 🚧 Active branch | P1 | M-3 | `QueryServed` 不再保存原始 query；增加 hash/scope/schema version；适配 M12/suggest 读取。 | salted hash + 可选 redacted preview；旧事件兼容；query history 不跨 scope 泄漏；相关策略测试通过。 | PR 2 |
| 6 | wiki-cli LLM governance | Planned | P1 | M-2 / I-4 | `wiki-cli` LLM config 支持 `api_key_env`；prompt 中 source body 明确作为 untrusted payload；补 max input/output、redaction hook、provider allowlist、structured validation。 | inline key 兼容但非首选；adversarial prompt / oversized input / invalid output 测试通过。 | PR 1 |
| 7 | CI required hardening | Planned | P1 | I-1 | Required CI 加 clippy；新增 cargo-deny/advisory/license/yanked/duplicate 检查；保留 heavy audit scheduled/manual 边界。 | PR gate 明确；`cargo clippy --workspace --all-targets -- -D warnings` 进 required lane；deny 配置和 smoke 通过。 | 无 |
| 8 | Production SearchPorts default | Planned | P2 | M-5 | 生产 query 默认使用 storage-backed BM25/vector/graph ports；`InMemorySearchPorts` 限定测试/fallback。 | 无 palace/storage 时行为有明确 fallback；query truth table 文档同步；CLI/MCP query tests 通过。 | PR 5 |
| 9 | CJK / Unicode retrieval | Planned | P2 | M-6 | `rust-mempalace` FTS query 保留 Unicode token；CJK 走 trigram/LIKE fallback；空 token 不再固定成 `"memory"`。 | 中文 query e2e 通过；英文 FTS quote 回归不退化；搜索 limit 仍受 PR 1 clamp 保护。 | PR 1 |
| 10 | Outbox + SQLite reliability | Planned | P2 | M-1 / M-8 | per-consumer ack 计数不再依赖全局 `processed_at`；统一 `busy_timeout` / transaction wrapper / retry 或 backoff 指标。 | 第二 consumer ack 计数正确；legacy `processed_at` 不破坏 cursor 语义；locked/busy 测试通过。 | PR 1 |
| 11 | Vault projection safety + docs/test cleanup | Planned | P3 | L-1 / L-2 / L-3 / L-4 / I-5 / I-6 | 空 slug fallback、YAML/frontmatter escape、managed marker/trash/quarantine/dry-run；修 architecture/README 状态矛盾；新增慢速 hardening/perf/fuzz lane。 | projection 不生成空 basename、不误删手写 UUID 页；docs 状态一致；nightly/full lane 覆盖 perf、MCP fuzz、DB corruption、CJK、bank/scope 矩阵。 | PR 3 / PR 9 / PR 10 |

### P0：Production Compiler scale-up（非新功能）

已完成。已全量执行小批量生产编译+固定检查链路（`batch-ingest`
→ `compiler-resolve-deferred --allow-create --apply` → `consume-to-mempalace` →
`lint` / `audit` → 重复 concept/entity + 断链 + Obsidian 抽查），目标验证已达成。

### P1：Source 生命周期治理（同一批规划，分 PR 实现）

1. Notion Archived Source Retirement audit/plan：PR #68 已完成 dry-run plan，不改 DB/Vault/Palace。
2. Notion Archived Source Retirement apply：PR #69 已完成 guarded apply，DB-first 退役，再清理匹配的 Vault source 文件。
3. MCP Vault Sync：PR #61 已完成 MCP 写操作后自动触发 Vault projection。

### P2：报告自动化（低风险独立批次）

Scheduled Vault Reports：PR #67 已把 `vault-audit`、`metrics`、`dashboard`、
`automation health`、`suggest` 接入 automation `vault-reports`，支持 timestamped bundle、
latest 指针和历史保留/清理策略。

### P3：Outbox 消费语义（独立批次）

Outbox Consumer Cursors：新增 consumer-scoped cursor / at-exactly-once 语义，
减少重复派发。该项触碰 outbox 协议和 Mempalace 消费边界，不和 reporting 或 compiler
scale-up 混做。

### P4：Reliability / Hardening

1. Multi-process Write Guardrails：PR #64 已用 repository-adjacent writer lease 强制单 writer。
2. Reliability Test Matrix：PR #65 已补核心回归矩阵；后续可在独立慢速 lane 扩展 fuzz/perf。
3. Dependency Audit Automation：PR #66 已新增 scheduled/manual `cargo audit` lane，不拖慢 quick CI。

### P5：Embedding / Retrieval 线（同一方向，按阶段实现）

1. Benchmark Reproducibility：`rust-mempalace benchmark --mode random` 增加 `--seed`，并写入 `benchmark_runs`。
2. Embedding Tx Atomicity：把 embedding 写入纳入 snapshot/outbox 同一事务边界。
3. C16B Embedding ANN spike / feature gate：PR #72 已确定 feature gate、fallback 和 CI story。
4. C16B Embedding ANN implementation：PR #73 已实现 locality-bucket bounded search、fallback full scan、ranking 回归测试。
5. Contradiction Scan Scaling：PR #74 已优化 contradiction O(n²) 路径。
6. J14 Semantic Fusion Benchmark：PR #75 已在 J13 基础上增加 semantic/query fusion 对照 benchmark。
7. Row-level Wiki State Storage：PR #76/#77 已完成 migration/dual-write 与 cutover/cleanup；blob 兼容层保留到生产验证后再移除。

### P6：DX / Maintainability

1. CLI Command Modularization phase 1：PR #78 已拆低风险命令域，减少 `main.rs` 体积。
2. CLI Command Modularization phase 2：PR #79 已拆 no-engine dispatcher/shared runtime setup，并补命令 smoke。
3. MCP Typed Errors：PR #60 已完成 typed JSON-RPC error mapping。
4. Time Library Unification：PR #80 已统一剩余 `chrono` 使用到 `time`。

### P7：M12 executor（最后，已完成）

1. M12 executor dry-run planner：PR #81 已完成 `suggest --executor-plan` action plan，只产出计划。
2. M12 executor guarded apply：PR #82 已完成 allowlist、dry-run-first、explicit apply flag，只执行低风险可审计动作。

## 审计剩余项 PR 计划

### PR #58 可继续收敛项

以下 3 项已在 PR #58 收敛，原因是改动面小、与 hardening 主题一致，且不会改变核心数据模型：

| 项目 | PR #58 内完成范围 | 边界 |
| --- | --- | --- |
| Automation Integrity Check | 把 `PRAGMA integrity_check` 接入 `automation health` 报告/退出状态。 | 不做 scheduled report 保留策略；那属于 `Scheduled Vault Reports`。 |
| Multi-process Write Guardrails | 补 README / AGENTS 显著警告：不要多个 `LlmWikiEngine` writer 同时写同一 DB。 | 不做 advisory lock / writer lease；那是后续独立 PR。 |
| MCP API Reference | 写一份基于现有 `tools/list` schema 的 MCP 工具 Markdown 参考。 | 不做 typed errors；typed errors 是后续独立 PR。 |

### PR #58 之后的拆分

用户确认后的完成顺序固定为 23 个 PR。原则：先安全/写入语义，再 source/reporting，再 retrieval/storage，再 DX，最后 executor。

| 顺序 | PR | 覆盖项 | 状态 | 原因 |
| --- | --- | --- | --- | --- |
| 1 | MCP Typed Errors | `MCP Typed Errors` | ✅ PR #60 | 稳定 MCP error 分类。 |
| 2 | MCP Vault Sync | `MCP Vault Sync` | ✅ PR #61 | MCP 写操作启用 sync 时刷新 Vault projection。 |
| 3 | Outbox Consumer Cursors schema/API | `Outbox Consumer Cursors` | ✅ PR #62 | 新增 consumer-scoped cursor API，不改默认行为。 |
| 4 | Outbox Consumer Cursors cutover | `Outbox Consumer Cursors` | ✅ PR #63 | CLI/consumer 切到 cursor 默认语义，`--last-id` 保留 legacy override。 |
| 5 | Multi-process Write Lock / Lease | `Multi-process Write Guardrails` | ✅ PR #64 | 单 DB writer lease，拿不到锁 fail fast。 |
| 6 | Reliability Test Matrix | `Reliability Test Matrix` | ✅ PR #65 | 补并发/失败/坏输入/大数据 smoke 回归。 |
| 7 | Dependency Audit Automation | `Dependency Audit Automation` | ✅ PR #66 | scheduled/manual `cargo audit`，不拖慢 quick。 |
| 8 | Scheduled Vault Reports | `Scheduled Vault Reports` | ✅ PR #67 | 定时报告 bundle、latest 指针、保留策略。 |
| 9 | Notion Archived Source Retirement audit/plan | `Notion Archived Source Retirement` | ✅ PR #68 | 先做 Notion archived dry-run plan，不改 DB/Vault。 |
| 10 | Notion Archived Source Retirement apply | `Notion Archived Source Retirement` | ✅ PR #69 | DB-first apply，避免手删 Markdown。 |
| 11 | Benchmark Reproducibility | `Benchmark Reproducibility` | ✅ PR #70 | `--mode random` 加 `--seed` 并记录到 `benchmark_runs`。 |
| 12 | Embedding Tx Atomicity | `Embedding Tx Atomicity` | ✅ PR #71 | embedding 写入纳入 snapshot/outbox 同事务。 |
| 13 | C16B Embedding ANN spike / feature gate | `C16B Embedding ANN index` | ✅ PR #72 | 先确定 ANN 技术路径、fallback 和 CI story。 |
| 14 | C16B Embedding ANN implementation | `C16B Embedding ANN index` | ✅ PR #73 | bounded vector search、fallback full scan、ranking 回归。 |
| 15 | Contradiction Scan Scaling | `Contradiction Scan Scaling` | ✅ PR #74 | 降低 `naive_contradiction_pairs` O(n²) 爆炸风险。 |
| 16 | J14 Semantic Fusion Benchmark | `J14 Semantic Fusion Benchmark` | ✅ PR #75 | semantic/query fusion 对照评估 lane。 |
| 17 | Row-level Wiki State Storage migration/dual-write | `Row-level Wiki State Storage` | ✅ PR #76 | 加行级表与迁移路径，保留快照兼容。 |
| 18 | Row-level Wiki State Storage cutover/cleanup | `Row-level Wiki State Storage` | ✅ PR #77 | 主路径切到行级 state，保留验证和恢复说明。 |
| 19 | CLI Command Modularization phase 1 | `CLI Command Modularization` | ✅ PR #78 | 先拆低风险命令域，不改 CLI 行为。 |
| 20 | CLI Command Modularization phase 2 | `CLI Command Modularization` | ✅ PR #79 | 再拆 dispatcher/shared config，补 smoke。 |
| 21 | Time Library Unification | `Time Library Unification` | ✅ PR #80 | 统一或文档化 `chrono` / `time` 边界。 |
| 22 | M12 executor dry-run planner | `M12 Executor` | ✅ PR #81 | 只产出 action plan，不执行写入。 |
| 23 | M12 executor guarded apply | `M12 Executor` | ✅ PR #82 | allowlist + dry-run-first + explicit apply flag。 |

执行计划见 [automation-issue-batch-3.md](automation-issue-batch-3.md)。开发流程见
[dev-workflow.md](dev-workflow.md)，batch-3 PRD 见 [prd/batch-3.md](prd/batch-3.md)。

## 不再重复开发

- `mempalace_*` MCP 工具已经通过 `wiki_mempalace_bridge::make_tools` 访问 bridge。
- outbox ack 已经以 `wiki_outbox_consumer_progress(consumer_tag, acked_up_to_id, acked_at)` 为消费者进度真源。
- `consume-to-mempalace --palace` 的 live bank 已由 `--viewer-scope` 派生。
- `--graph-extras-file` 已按 viewer scope 过滤 wiki doc id，只允许 `mp_drawer:` / `mp_kg:` 外部 id。
- `write_projection` 已清理带合法 page-id frontmatter 的 stale managed page。
