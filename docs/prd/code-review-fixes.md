# Code Review Fixes — Batch CR-01

## Summary

- Goal: 修复全库代码审查发现的 Critical / High / Medium / Low 问题，消除数据不一致、事件丢失和测试误导风险。
- User value: 保证 DB-Vault-Mempalace 三层数据一致性，避免 outbox 事件丢失，benchmark 结果可信，notion 迁移链接正确。
- Success criteria: `cargo test --workspace` 全绿；`cargo clippy --workspace --all-targets -- -D warnings` 无新警告；文档与代码对齐。

## Plain-Language Product Scope

- User can: 运行 `consume-to-mempalace` 后 Mempalace 与 wiki 数据一致；重复 save_snapshot 产生相同序列化字节；notion 链接解析不误匹配；benchmark 报告 hits 真实。
- User cannot: 本批不改动 MCP 协议接口定义、不改变 CLI 参数设计、不修改 Notion 导出格式。
- User decision needed: MCP `_wiki_dir` 是否启用自动 vault projection（见 Module 4，已标 out-of-scope 本批仅加注释）。

## Scope

In:

- `crates/wiki-kernel/src/memory.rs` — 快照排序确定性
- `crates/wiki-storage/src/lib.rs` — `save_snapshot` 事务一致性（已委托 inner，无需改）
- `crates/wiki-mempalace-bridge/src/lib.rs` — `SourceIngested` unresolved 计入 unresolved 而非 filtered
- `crates/wiki-kernel/src/engine.rs` — `flush_outbox_to_repo_with_policy` drain 修复；expect 替换
- `crates/wiki-cli/src/consistency.rs` — `notion_uuid_from_target` 锚定规则
- `crates/wiki-migration-notion/src/writer.rs` — `url_index` 重复 URL 检测
- `crates/rust-mempalace/src/service.rs` — benchmark hits 列；samples 改为 actual total；random 模式注释
- `crates/wiki-kernel/src/wiki_writer.rs` — `cleanup_stale_managed_pages` 保护注释（逻辑已正确，补文档）

Out:

- MCP `_wiki_dir` 实际 projection 集成（需独立 PRD — 参见 roadmap "MCP Vault Sync"）
- embedding 写入纳入快照事务（需独立存储迁移 PRD — 参见 roadmap "C16B-embedding-tx"）
- outbox 消费者游标 / at-exactly-once 语义（需独立 outbox-v2 PRD — 参见 roadmap "Outbox Consumer Cursors"）
- longmemeval benchmark random 模式可重复性种子（需独立配置 PRD — 参见 roadmap "Benchmark Reproducibility"）

## Modules


| Module | Goal | Owner area | Status |
| --- | --- | --- | --- |
| M1-snapshot-sort | 快照序列化排序确定性 | memory.rs | ✅ Completed |
| M2-source-unresolved | SourceIngested unresolved 正确计数 | mempalace-bridge/lib.rs | ✅ Completed |
| M3-outbox-drain | flush_outbox drain 修复 + expect 替换 | engine.rs | ✅ Completed |
| M4-save-snapshot-tx | save_snapshot 事务包装 | wiki-storage/lib.rs | ✅ Completed |
| M5-notion-uuid | notion_uuid_from_target 锚定 | consistency.rs | ✅ Completed |
| M6-url-index | url_index 重复 URL 检测 | writer.rs | ✅ Completed |
| M7-benchmark | benchmark hits/samples 字段修复 | service.rs + db.rs | ✅ Completed |
| M8-stale-pages-doc | cleanup_stale_managed_pages 保护注释 | wiki_writer.rs | ✅ Completed |


## Acceptance

- M1: 同一逻辑 store 两次 `to_snapshot` 产生完全相同 JSON（新测试通过）。
- M2: `SourceIngested` + 无法解析 scope 时 `stats.unresolved` 计数 +1，不计入 `filtered`（已有测试或新测试覆盖）。
- M3: `flush_outbox_to_repo_with_policy` 失败时不丢失未被追加的事件（drain 范围修正）；`last_err.expect` 替换为 `EngineError`。
- M4: `notion_uuid_from_target` 仅从文件名末尾 32 hex 段或 Notion URL 路径末尾提取，不匹配中间子串。
- M5: 重复 URL 产生 `WriteStats.duplicate_urls` 计数（或 tracing warn），不丢失任何一条。
- M6: `latest_benchmark` 返回真实 `hits`；`benchmark_run` 存入 `out.total` 而非 `samples` 参数；DB schema 迁移兼容旧行。

## Risks

- M2 改动可能使现有测试断言值变化（unresolved vs filtered 计数）—— 需逐一检查。
- M6 `benchmark_runs` 需 schema 迁移添加 `hits` 列；旧行 `hits = 0` 为历史值，不影响功能。

## Rollout

- Branch strategy: `cursor/code-review-fixes-6950` 单分支承载所有 6 个模块。
- PR sequence: 单 PR，一次 review。
- Merge gate: CI green + reviewer sign-off。

## Status

- [x] PRD approved
- [x] Plain architecture approved
- [x] Specs created
- [x] Modules implemented
- [x] CI green (local: fmt ✓ clippy ✓ test ✓)
- [ ] Merged (PR #34 draft，待 review sign-off)
- [ ] Roadmap updated