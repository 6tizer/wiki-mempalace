# Archived Roadmap Execution Tables 2026-04

这些表从 `docs/roadmap.md` 移出。它们是已完成执行历史，不再作为当前路线源。

## Audit Report Follow-up v2: PR #83-#93

来源：Notion《wiki-mempalace 全方位代码审计报告》2026-04-29 状态表 + 当前代码复核。

| 顺序 | PR 主题 | 状态 | 优先级 | 覆盖审计项 | 范围边界 | 验收条件 | 依赖 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | MCP input boundary quick fix | ✅ PR #83 | P0 | H-5 / H-3 | MCP `limit` / `per_stream_limit` / `search_limit` clamp；`write_lint_report` 路径遍历拒绝。 | schema + handler 双层限幅；checked conversion；`../evil.md`、绝对路径、reports symlink、file symlink、超大 limit 测试通过。 | 无 |
| 2 | MCP scope capability hardening | ✅ PR #84 | P0 | H-1 / I-3 | `resolve_write_scope` 不再允许任意覆盖 server viewer；`supersede` 加 viewer 可见性校验。 | 写工具只能用 server-side viewer/capability 或合法子 scope；跨 scope supersede、write-page/crystallize mismatch、maintenance hidden-claim mutation 拒绝/隔离测试通过。 | PR 1 |
| 3 | Mempalace bank capability | ✅ PR #85 | P0 | H-2 / I-3 | MCP mempalace 工具禁止 client 传 `bank_id`；由 `viewer_scope -> bank_id` 派生；KG query/timeline/stats 加 bank filter。 | 搜索、wake_up、taxonomy、traverse、reflect、KG 查询均按 bank 隔离；越权测试通过。 | PR 2 |
| 4 | Cross-bank drawer dedupe migration | ✅ PR #86 | P0 | M-7 | `drawers(content_hash)` 全局唯一改为 `(bank_id, content_hash)`；live sink / mine path 查重同步改造。 | migration 可重复执行；相同内容不同 bank 可共存；同 bank 仍去重；bridge/live tests 通过。 | PR 3 |
| 5 | QueryServed privacy/schema | ✅ PR #87 | P1 | M-3 | `QueryServed` 不再保存原始 query；增加 hash/scope/schema version；适配 M12/suggest 读取。 | salted hash + 可选 redacted preview；旧事件兼容；query history 不跨 scope 泄漏；相关策略测试通过。 | PR 2 |
| 6 | wiki-cli LLM governance | ✅ PR #88 | P1 | M-2 / I-4 | `wiki-cli` LLM config 支持 `api_key_env`；prompt 中 source body 明确作为 untrusted payload；补 max input/output、redaction hook、provider allowlist、structured validation。 | inline key 兼容但非首选；adversarial prompt / oversized input / invalid output 测试通过。 | PR 1 |
| 7 | CI required hardening | ✅ PR #89 | P1 | I-1 | Required CI 加 clippy；新增 cargo-deny/advisory/license/yanked/duplicate 检查；保留 heavy audit scheduled/manual 边界。 | PR gate 明确；`cargo clippy --workspace --all-targets -- -D warnings` 进 required lane；deny 配置和 smoke 通过。 | 无 |
| 8 | Production SearchPorts default | ✅ PR #90 | P2 | M-5 | 生产 query 默认使用 storage-backed BM25/vector/graph ports；`InMemorySearchPorts` 限定测试/fallback。 | 无 palace/storage 时行为有明确 fallback；query truth table 文档同步；CLI/MCP query tests 通过。 | PR 5 |
| 9 | CJK / Unicode retrieval | ✅ PR #91 | P2 | M-6 | `rust-mempalace` FTS query 保留 Unicode token；CJK 走 trigram/LIKE fallback；空 token 不再固定成 `"memory"`。 | 中文 query e2e 通过；英文 FTS quote 回归不退化；搜索 limit 仍受 PR 1 clamp 保护。 | PR 1 |
| 10 | Outbox + SQLite reliability | ✅ PR #92 | P2 | M-1 / M-8 | per-consumer ack 计数不再依赖全局 `processed_at`；统一 `busy_timeout` / transaction wrapper / retry 或 backoff 指标。 | 第二 consumer ack 计数正确；legacy `processed_at` 不破坏 cursor 语义；locked/busy 测试通过。 | PR 1 |
| 11 | Vault projection safety + docs/test cleanup | ✅ PR #93 | P3 | L-1 / L-2 / L-3 / L-4 / I-5 / I-6 | 空 slug fallback、YAML/frontmatter escape、managed marker/trash/quarantine/write guard；修 architecture/README 状态矛盾；新增慢速 hardening smoke lane。 | projection 不生成空 basename、不误删/覆盖手写 UUID 页；docs 状态一致；nightly/full lane 覆盖 perf smoke、MCP malformed/boundary、DB corruption、CJK、bank/scope 矩阵。 | PR 3 / PR 9 / PR 10 |

## Audit Report Hardening Split After PR #58: PR #60-#82

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

## Former Phase Buckets

这些阶段均已完成，当前状态以 `docs/roadmap.md` 的完成能力总账为准。

- P0 Production Compiler scale-up：已全量执行小批量生产编译和固定检查链，`remaining uncompiled = 0`。
- P1 Source 生命周期治理：Notion archived retirement、MCP Vault Sync 已完成。
- P2 报告自动化：Scheduled Vault Reports 已完成。
- P3 Outbox 消费语义：Outbox Consumer Cursors 已完成。
- P4 Reliability / Hardening：writer lease、reliability matrix、dependency audit 已完成。
- P5 Embedding / Retrieval：benchmark seed、embedding tx、ANN、contradiction scan、J14、row-level state 已完成。
- P6 DX / Maintainability：CLI modularization、typed errors、time unification 已完成。
- P7 M12 executor：dry-run planner + guarded apply 已完成。
