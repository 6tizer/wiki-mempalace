# Current Roadmap

本文是当前计划源。只放当前状态、下一步候选、完成索引。历史执行细表已归档到
[archive/roadmap-completed-pr-plans-2026-04.md](archive/roadmap-completed-pr-plans-2026-04.md)。

## 当前状态

| 轨道 | 状态 | 当前事实 |
| --- | --- | --- |
| Active implementation | 完成 | Notion 三库删档重建导入已由 PR #110 合入，代码、生产替换、文档回填均完成；Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3 已由 PR #101-#107 完成本轮闭环；xAI web search follow-up PR #108 已将默认双搜索从 Exa/Tavily 换成 Exa/xAI |
| Production data ops | 已替换 | `/Users/mac-mini/Documents/wiki` 已由 2026-05-05 三库导出重建：pages `4765`，sources `1526`，Notion index `943+583`，production consistency `palace_missing_page_drawers=0`；备份见本页当前执行记录 |
| Audit / hardening | 处置决策完成 | PR #58 + PR #60-#67 + PR #83-#93 已覆盖上一轮；M-5/L-4/I-8 已由 PR #95/#96/#97 完成；Hardening schedule run `25150878853` 已观察为 green |
| Docs state | 本页为总入口 | spec 状态见 [specs/README.md](specs/README.md)，经验见 [LESSONS.md](LESSONS.md)，历史计划见 [archive/](archive/README.md) |

## 已完成：Notion 三库删档重建导入（2026-05-05）

来源：`/Users/mac-mini/wiki-migration/NotionDB导出` 的三个最新 Notion ZIP。
目标不是增量合并，而是备份旧生产后，用最新导出重建 `wiki.db`、Vault 投影和 `palace.db`。
Spec：[notion-full-reimport-20260505/](specs/notion-full-reimport-20260505/)。
PR：[#110](https://github.com/6tizer/wiki-mempalace/pull/110)。

| 阶段 | 状态 | 验收 |
| --- | --- | --- |
| 导入链路修复 | 完成 | Wiki 只扫 `知识 Wiki/` 直接子页；X 只扫 `X书签文章数据库/` 直接子页；微信只扫 `文章数据库/` 直接有效子页；大小写文件名碰撞不覆盖；backfill 保留标签/置信度/source metadata |
| Staging 重建 | 完成 | dry-run `4765/943/583`；migrate 总对象 `6291`；backfill `sources_seen=1526`、`pages_seen=4765`、`skipped=0`、`warnings=0`；`notion_page_index=1526`；staging consistency `palace_missing_page_drawers=0` |
| Production 替换 | 完成 | 备份 `/Users/mac-mini/Documents/wiki-backups/notion-full-reimport-20260505-20260505-071121/wiki`；旧目录 `/Users/mac-mini/Documents/wiki-old-notion-full-reimport-20260505-20260505-071121`；生产 query、palace、consistency smoke 通过 |

## 下一步 1-3 收口（2026-04-30）

| 项 | 状态 | 证据 | 后续 |
| --- | --- | --- | --- |
| `vault-report-paths` 清理 | 完成 PR #99 | PR #22 已 merge；远端分支 `codex/vault-report-paths` 已删除；spec index 改为 Merged PR #22 | 无 |
| Row-level state production validation | 完成 PR #99，不能退 blob | 新增只读 `verify-row-state`；生产 `/Users/mac-mini/Documents/wiki/.wiki/wiki.db` 返回 `rows=0 blob_present=true matches_blob=n/a` | 保留 blob fallback；未来若要退役，先做受控生产 row-state migration/backfill |
| Hardening scheduled lane observation | 完成 PR #99 | GitHub Actions run [`25150878853`](https://github.com/6tizer/wiki-mempalace/actions/runs/25150878853)：`schedule` / `success`，`perf`、`cjk-retrieval`、`db-corruption`、`mcp-boundary`、`bank-scope` 全绿 | 无修复 PR |

## 已完成 PR 计划（2026-04-30 处置决策）

来源：Notion《wiki-mempalace 全方位代码审计报告》“处置决策（2026-04-30 产品负责人确认）”。
本批 3 个待修复项已按 [dev-workflow.md](dev-workflow.md) 完成 PRD / spec 三件套 / handoff / LESSONS / CI / merge。

| 顺序 | PR 主题 | 状态 | 优先级 | 覆盖项 | 范围边界 | 验收条件 | 建议分支 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | MCP `wiki_query` storage-backed default | Merged PR #95 | P0 | M-5 | MCP `wiki_query` 默认搜索从 `query_pipeline_memory` 切到 SQLite-backed `SqliteSearchPorts`，必要时组合 palace ports；`InMemorySearchPorts` 只保留测试/显式 fallback。 | MCP `wiki_query` 能检索 repo 中已持久化但不在当前 in-memory store 的内容；无 storage 时明确 fallback；write_page 结果仍可写入 page；相关 MCP tests 过。 | `codex/audit-disposition-01-mcp-query-storage-ports` |
| 2 | Docs consistency cleanup | Merged PR #96 | P1 | L-4 | 统一 Rust edition 描述和 Notion 增量同步状态；修 `docs/architecture.md`、`crates/rust-mempalace/README.md` 等明显矛盾。 | active docs 不再命中旧 edition 说法；active docs 不再出现 Notion sync 旧状态；docs-only `git diff --check` 过。 | `codex/audit-disposition-02-doc-consistency` |
| 3 | MCP API reference refresh | Merged PR #97 | P1 | I-8 | 刷新独立 MCP API 文档，覆盖 wiki + mempalace 全工具参数、返回形状、错误类型、副作用、scope/bank 规则；同步 PR #84/#85 后 bank_id 不可由 client 注入的事实。 | 文档列全当前 `tools_list()` 工具；每个工具有 required/optional/returns/writes/notes；`mempalace_*` 不再把 `bank_id` 写成可越权参数；docs index 链接完整。 | `codex/audit-disposition-03-mcp-api-reference` |

## 处置决策记录

| ID | 问题 | 决策 | Roadmap 处理 |
| --- | --- | --- | --- |
| M-2 | LLM Prompt Injection | 接受风险 | 个人使用、无不可信外部输入；已有 UNTRUSTED 标记、redact、validate_bounds。多用户或自动爬外部内容时再重评估。 |
| M-5 | 默认搜索走 InMemorySearchPorts | 已完成 | PR #95：MCP `wiki_query` 默认 storage-backed ports。 |
| L-4 | 文档 edition / 增量同步矛盾 | 已完成 | PR #96：active docs edition / Notion sync 状态一致性。 |
| L-5 | chrono vs time 双时间库 | 暂缓 | 依赖升级或兼容问题出现时再处理。 |
| I-5 | 性能基准测试 | 暂缓 | 数据量超过 1 万或明显变慢时再建立 wiki pipeline benchmark。 |
| I-6 | 并发/故障注入/压力测试 | 暂缓 | 产品闭环跑完、功能稳定后再做。 |
| I-8 | MCP 工具 API 文档 | 已完成 | PR #97：刷新 MCP API reference 并加 docs sync tests。 |

## 后续候选

这些不是已启动任务。启动任一项前仍按 [dev-workflow.md](dev-workflow.md)：
PRD -> 白话架构 -> spec 三件套 -> branch -> Plan/review/PR。

| 候选 | 为什么在池子里 | 启动条件 |
| --- | --- | --- |
| Row-level state production migration/backfill | 只读验证已完成，但生产 DB 还没有 `wiki_state_row`，所以 blob fallback 不能退 | 用户决定要推进 blob fallback 退役时，先做 backup/dry-run，再受控写入 row-level state |
| New product/module batch | 当前 M1-M12、J13/J14、compiler、Notion、audit hardening 都已完成本轮闭环 | 用户选定新目标后，新建 PRD 和模块 spec |

## Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3

来源：Notion 工作流程 / Schema 中维护 Agent 能力盘点，以及本地系统“DB 真源、投影派生、证据驱动自动化”的重新设计。
PRD：[wiki-governance-evidence-synthesis-v3.md](prd/wiki-governance-evidence-synthesis-v3.md)。

| 顺序 | PR 主题 | 状态 | 范围 | 建议分支 |
| --- | --- | --- | --- | --- |
| 1 | AI provider profiles + dual web search runtime | Merged PR #101；xAI follow-up merged PR #108 | `[llm]` 兼容 profile resolver；默认 Exa/xAI 双 provider search；Tavily 旧配置仍兼容；smoke commands | `codex/ai-provider-profiles` / `codex/xai-web-search-provider` |
| 2 | Governance scan | Merged PR #102 | 状态、引用、lint、重复、标签、删除候选、synthesis 内部信号 | `codex/wiki-governance-scan` |
| 3 | Evidence Fixer plan | Merged PR #103 | 规则 + LLM + 双搜索证据生成 typed fix plan | `codex/evidence-fixer-plan` |
| 4 | Evidence Fixer apply + restore | Merged PR #104 | 证据阈值自动 apply；合并/删除/语义 patch 可恢复 | `codex/evidence-fixer-apply-restore` |
| 5 | Synthesis discovery | Merged PR #105 | 单/双/三/四标签候选发现、排序、去重 | `codex/synthesis-discovery` |
| 6 | Web-backed Synthesis composer | Merged PR #106 | 内部 evidence pack + 双搜索 evidence + LLM 写作/校验 | `codex/web-backed-synthesis-composer` |
| 7 | Automation + docs | Merged PR #107 | daily/manual lanes、MCP、Notion 对照矩阵、文档收口 | `codex/governance-automation-docs` |

## 已完成能力总账

| 领域 | 状态 | 证据 |
| --- | --- | --- |
| Automation core | ✅ 完成 | M1-M8 已落地：调度、状态/心跳、health、outbox matrix、restore、gap、fix、消费 page contract |
| Ops reporting / strategy | ✅ 完成 | M10 metrics PR #12、M11 dashboard PR #14、M12 suggest PR #16、M12 executor PR #81/#82 |
| Schema / governance | ✅ 完成 | tag governance PR #13、B5 orphan governance PR #28/#30、DB/Vault/Palace consistency PR #32/#33 |
| Vault / Mempalace bootstrap | ✅ 完成并跑过生产 | Vault Backfill + Palace Init PR #23；生产 `/Users/mac-mini/Documents/wiki` 已完成 backfill + palace init |
| Notion lifecycle | ✅ 完成并跑过生产 apply | Notion Incremental Sync PR #36/#38/#42；Archived Source Retirement PR #68/#69；Source Vault Projection PR #42 |
| Production compiler | ✅ 完成并跑完 scale-up | Compiler PR #44/#46/#47/#54/#56；生产执行后 `remaining uncompiled = 0` |
| Retrieval / benchmark | ✅ 完成 | M9 query fusion、J13 LongMemEval PR #19、J14 PR #75、C16B ANN PR #72/#73、SearchPorts PR #90、CJK PR #91 |
| Storage / reliability | ✅ 完成 | C16A PR #25、row-level state PR #76/#77、embedding tx PR #71、outbox reliability PR #92 |
| MCP / API hardening | ✅ 完成 | Typed errors PR #60、Vault sync PR #61、consumer cursors PR #62/#63、scope/bank/input hardening PR #83-#85、MCP query/API docs PR #95/#97 |
| CI / hardening | ✅ 完成 | Reliability matrix PR #65、dependency audit PR #66、required CI PR #89、hardening lane PR #93 |
| DX / maintainability | ✅ 完成 | CLI modularization PR #78/#79、time unification PR #80、MCP API reference PR #58/#97 |

## Wiki Agent Native Runtime（2026-05-05）

目标：新增 `wiki-agent`，默认通过 native Rust ToolRegistry 直接调用 wiki/mempalace 工具；`wiki-cli mcp` 保持兼容入口，MCP child-process 只作 fallback。
PRD：[wiki-agent.md](prd/wiki-agent.md)。

| 顺序 | PR 主题 | 状态 | 范围 | 分支 |
| --- | --- | --- | --- | --- |
| 1 | Shared AI core | Merged PR #113 | 抽出 `wiki-ai`，共享 LLM profile / embedding / web search runtime | `codex/wiki-agent-shared-ai-core` |
| 2 | Native ToolRegistry + MCP adapter | Merged PR #114 | 新增 `wiki-tools`；`wiki-cli mcp` 转调共享实现；新增 `wiki-agent doctor` native / mcp-child discovery | `codex/wiki-agent-native-tool-registry` |
| 3 | Pure CLI chat | Merged PR #115 | `wiki-agent chat` REPL、profile、slash commands、session history | `codex/wiki-agent-cli-chat` |
| 4 | Local + Web RAG | Merged PR #116 | 本地 wiki/palace 检索 + Exa/xAI web evidence 综合回答 | `codex/wiki-agent-web-rag` |
| 5 | Manager-worker sub agents | Merged PR #117 | Lint/Governance/Fixer/Synthesis/Search/Memory workers | `codex/wiki-agent-manager-workers` |
| 6 | Persistent memory + skills | Merged PR #118 | `.wiki/wiki-agent.db` 会话记忆，验证后写入 `wiki.db`/`palace.db` | `codex/wiki-agent-memory-skills` |
| 7 | TUI | Merged PR #119 | ratatui + crossterm 分屏、状态栏、流式输出、键盘导航 | `codex/wiki-agent-tui` |
| 8 | Docs + E2E | Merged PR #120 | README、架构、MCP/API 对照、Notion 能力矩阵、端到端 smoke | `codex/wiki-agent-docs-e2e` |

## Audit Report Follow-up v2 Closeout

来源：Notion《wiki-mempalace 全方位代码审计报告》2026-04-29 复核。
执行结果：11 个 PR 全部合入。

| PR | 主题 | 结果 |
| --- | --- | --- |
| #83 | MCP input boundary quick fix | limit clamp + lint report path guard |
| #84 | MCP scope capability hardening | write scope capability + supersede visibility guard |
| #85 | Mempalace bank capability | MCP bank 由 viewer 派生；KG/search 按 bank 隔离 |
| #86 | Cross-bank drawer dedupe migration | drawer dedupe 改为 `(bank_id, content_hash)` |
| #87 | QueryServed privacy/schema | 不再存 raw query；hash/scope/schema 兼容旧事件 |
| #88 | wiki-cli LLM governance | `api_key_env`、untrusted payload、limits、redaction、provider allowlist |
| #89 | CI required hardening | required quick CI 加 clippy + cargo-deny policy checks |
| #90 | Production SearchPorts default | query/explain 默认 storage-backed ports |
| #91 | CJK / Unicode retrieval | Unicode token + CJK fallback |
| #92 | Outbox + SQLite reliability | per-consumer ack + busy/transaction robustness |
| #93 | Vault projection safety + docs/test cleanup | managed marker、quarantine、write guard、docs consistency、hardening lane |

## 不再重复开发

- `mempalace_*` MCP 工具已经通过 `wiki_mempalace_bridge::make_tools` 访问 bridge。
- outbox ack 已经以 `wiki_outbox_consumer_progress(consumer_tag, acked_up_to_id, acked_at)` 为消费者进度真源。
- `consume-to-mempalace --palace` 的 live bank 已由 `--viewer-scope` 派生。
- `--graph-extras-file` 已按 viewer scope 过滤 wiki doc id，并拒绝外部注入 `mp_drawer:` / `mp_kg:` id。
- `write_projection` 只清理带显式 managed marker 的 stale projection page，并将其移入 `.wiki/trash/projection/`。
