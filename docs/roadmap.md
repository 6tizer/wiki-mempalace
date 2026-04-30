# Current Roadmap

本文是当前计划源。只放当前状态、下一步候选、完成索引。历史执行细表已归档到
[archive/roadmap-completed-pr-plans-2026-04.md](archive/roadmap-completed-pr-plans-2026-04.md)。

## 当前状态

| 轨道 | 状态 | 当前事实 |
| --- | --- | --- |
| Active implementation | 无 | Audit Report Follow-up v2 已由 PR #83-#93 全部合入；当前没有已确认的新功能批次 |
| Production data ops | 稳定 | 最近生产 backfill、consistency、compiler scale-up 都已闭环；新生产写入仍必须 dry-run first |
| Audit / hardening | 完成一轮 | PR #58 + PR #60-#67 + PR #83-#93 已覆盖当前审计报告拆分项 |
| Docs state | 本页为总入口 | spec 状态见 [specs/README.md](specs/README.md)，经验见 [LESSONS.md](LESSONS.md)，历史计划见 [archive/](archive/README.md) |

## 下一步候选

这些不是已启动任务。启动任一项前仍按 [dev-workflow.md](dev-workflow.md)：
PRD -> 白话架构 -> spec 三件套 -> branch -> Plan/review/PR。

| 候选 | 为什么在池子里 | 启动条件 |
| --- | --- | --- |
| Row-level state production validation / blob fallback retirement | PR #76/#77 已完成 row-level state cutover，但 blob 兼容层仍保留到生产验证后再移除 | 先写验证 PRD/spec；只用 dry-run/backup 证明可退兼容层 |
| Hardening scheduled lane observation | PR #93 新增 scheduled/manual hardening lane；需要观察真实 GitHub schedule 首跑 | schedule 或 manual `all` lane 有结果后，再按失败证据开修复 PR |
| New product/module batch | 当前 M1-M12、J13/J14、compiler、Notion、audit hardening 都已完成本轮闭环 | 用户选定新目标后，新建 PRD 和模块 spec |

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
| MCP / API hardening | ✅ 完成 | Typed errors PR #60、Vault sync PR #61、consumer cursors PR #62/#63、scope/bank/input hardening PR #83-#85 |
| CI / hardening | ✅ 完成 | Reliability matrix PR #65、dependency audit PR #66、required CI PR #89、hardening lane PR #93 |
| DX / maintainability | ✅ 完成 | CLI modularization PR #78/#79、time unification PR #80、MCP API reference PR #58 |

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
