# Outbox Event Matrix

本文是 `wiki.db -> wiki_outbox -> consume-to-mempalace` 的源码事实表。目标是把
`WikiEvent` 的生产者、消费者、当前动作和测试覆盖固定下来，避免后续开发者靠读源码猜
"这个事件现在谁在用"。

约定：

- `active`：当前有稳定生产者，且有正式消费者。
- `retained-no-consumer`：当前会进入 outbox，但 bridge 只保留，不派发到 mempalace。
- `defined-not-emitted`：事件类型已定义，但当前没有稳定生产者。
- mempalace 目前正式消费 `PageWritten`、`ClaimUpserted`、`ClaimSuperseded`。
  `SourceIngested` 保留派发面，但 live sink 默认 no-op，避免历史 source 正文噪音进入 palace。
- 其余事件会留在 outbox 中，bridge 统计为 `ignored`，供未来自动化消费链使用。

| Event | Status | Producer | Trigger | Payload keys | Current consumer | Mempalace consumes | Current action | Why not consumed / notes | Test coverage |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SourceIngested` | active | `wiki-kernel::LlmWikiEngine::ingest_raw` | CLI `ingest` / `ingest-llm` 的 source 入库 | `source_id`, `redacted`, `at` | `wiki-cli consume-to-mempalace` -> `wiki-mempalace-bridge` -> `MempalaceWikiSink::on_source_ingested` | no by default | live sink no-op | source 仍进 `wiki.db`；palace v1 不默认吃原文全文 | `crates/wiki-kernel/src/engine.rs` `ingest_and_file_claim_flow`；`crates/wiki-mempalace-bridge/src/lib.rs` `consumes_source_ingested`；`crates/wiki-mempalace-bridge/tests/live_sink.rs` `source_ingested_does_not_create_source_drawer` |
| `ClaimUpserted` | active | `wiki-kernel::LlmWikiEngine::file_claim` | CLI `file-claim` / `supersede-claim` 创建新 claim | `claim_id`, `at` | `wiki-cli consume-to-mempalace` -> resolver -> `MempalaceWikiSink::on_claim_upserted` | yes | 还原完整 claim 后派发；resolver 缺失时回退 `on_claim_event` 并计为 `unresolved` | 使用 resolver + scope filter，避免共享 outbox 跨 scope 泄漏 | `crates/wiki-kernel/src/engine.rs` `ingest_and_file_claim_flow`、`supersede_chain`；`crates/wiki-mempalace-bridge/src/lib.rs` `resolver_path_materializes_claim_and_enforces_scope`、`stats_mark_unresolved_and_ignored_events`；`crates/wiki-cli/tests/automation_run_daily.rs` `consume_to_mempalace_dispatches_active_events_from_cli_flow` |
| `ClaimSuperseded` | active | `wiki-kernel::LlmWikiEngine::supersede` | CLI `supersede-claim` 替换旧结论 | `old`, `new`, `at` | `wiki-cli consume-to-mempalace` -> `MempalaceWikiSink::on_claim_superseded` | yes | 给 mempalace 写 supersede / invalidate 边 | resolver 路径必须能解析 new claim scope；否则计 `unresolved` 且不应 ack | `crates/wiki-kernel/src/engine.rs` `supersede_chain`；`crates/wiki-mempalace-bridge/src/lib.rs` `consumes_ndjson_and_dispatches_claim_events`、`unresolved_supersede_scope_is_not_dispatched` |
| `PageWritten` | active | `wiki-cli vault-backfill` / page write flows | 页面写入或历史 page 回填 | `page_id`, `at` | `wiki-cli consume-to-mempalace` / `palace-init` -> resolver -> `MempalaceWikiSink::on_page_written` | yes | eligible page 写入 `wiki_pages` drawer | live sink 只接收 summary / concept / entity / synthesis / qa；index / lint_report 不进 palace | `crates/wiki-mempalace-bridge/src/lib.rs` `resolver_path_materializes_page_written_and_enforces_scope`；`crates/wiki-mempalace-bridge/tests/live_sink.rs` `page_written_creates_drawer_and_rerun_does_not_duplicate`、`ineligible_pages_do_not_create_drawers` |
| `QueryServed` | retained-no-consumer | `wiki-kernel::LlmWikiEngine::record_query` / `query_pipeline_memory` | CLI `query` | `query_fingerprint`, `top_doc_ids`, `at` | none | no | bridge 统计为 `ignored` | 这类事件目前用于审计和未来检索观测，不属于 mempalace 写侧 | `crates/wiki-kernel/src/engine.rs` `persist_and_reload_snapshot_and_outbox`、`record_query_emits_query_served_event`；`crates/wiki-mempalace-bridge/src/lib.rs` `stats_mark_unresolved_and_ignored_events`；`crates/wiki-cli/tests/automation_run_daily.rs` `consume_to_mempalace_ignores_query_crystallize_and_lint_events` |
| `SessionCrystallized` | retained-no-consumer | `wiki-kernel::LlmWikiEngine::crystallize` | CLI `crystallize` | `page_id`, `at` | none | no | bridge 统计为 `ignored` | 当前是沉淀事件，不需要写入 mempalace drawer/vector | `crates/wiki-kernel/src/engine.rs` `crystallize_emits_session_crystallized_event`；`crates/wiki-cli/tests/automation_run_daily.rs` `consume_to_mempalace_ignores_query_crystallize_and_lint_events` |
| `GraphExpanded` | defined-not-emitted | none | none | `seeds`, `visited`, `at` | none | no | 保留事件定义，不派发 | 当前 `expand_graph` 只返回结果，不 emit 事件；保留给未来图观测 | `crates/wiki-mempalace-bridge/src/lib.rs` `event_matrix_doc_stays_in_sync_with_wiki_event_variants` |
| `LintRunFinished` | retained-no-consumer | `wiki-kernel::LlmWikiEngine::run_basic_lint` | CLI `lint` / `maintenance` 内 lint 阶段 | `findings`, `at` | none | no | bridge 统计为 `ignored` | 这类事件目前只服务 lint 审计和未来维护流水线 | `crates/wiki-kernel/src/engine.rs` `run_basic_lint_emits_lint_run_finished_event`；`crates/wiki-mempalace-bridge/src/lib.rs` `stats_mark_unresolved_and_ignored_events`；`crates/wiki-cli/tests/automation_run_daily.rs` `consume_to_mempalace_ignores_query_crystallize_and_lint_events` |
| `PageStatusChanged` | retained-no-consumer | `wiki-kernel::LlmWikiEngine::promote_page` / `mark_stale_pages` | 页面晋升、过期标记 | `page_id`, `from`, `to`, `actor`, `at` | none | no | bridge 统计为 `ignored` | 当前是页面生命周期事件，不参与 mempalace 知识图写侧 | `crates/wiki-kernel/src/engine.rs` `promote_needs_update_to_approved_works`、`mark_stale_emits_page_status_changed_event` |
| `PageDeleted` | retained-no-consumer | `wiki-kernel::LlmWikiEngine::cleanup_expired_pages` | 自动清理过期页面 | `page_id`, `at` | none | no | bridge 统计为 `ignored` | 当前只是页面生命周期收尾事件，供未来维护自动化使用 | `crates/wiki-kernel/src/engine.rs` `cleanup_expired_pages_emits_page_deleted_event` |

## 当前 bridge 派发规则

- `ClaimUpserted`
  - resolver 成功且通过 `scope_filter`：`dispatched += 1`
  - resolver 失败：回退 `on_claim_event`，但统计为 `unresolved += 1`
  - scope 不通过：`filtered += 1`
- `ClaimSuperseded`
  - scope 通过：`dispatched += 1`
  - scope 不通过：`filtered += 1`
  - resolver 路径无法解析 new claim scope：`unresolved += 1`
- `SourceIngested`
  - scope 通过：`dispatched += 1`
  - scope 不通过：`filtered += 1`
- `PageWritten`
  - resolver 成功且 scope 通过：`dispatched += 1`
  - resolver 失败：`unresolved += 1`
  - scope 不通过：`filtered += 1`
- 其他事件
  - 不派发到 mempalace sink
  - 统一计入 `ignored += 1`

## 文档漂移约束

`crates/wiki-mempalace-bridge/src/lib.rs` 中有 `event_matrix_doc_stays_in_sync_with_wiki_event_variants`
测试，会把本文件列出的事件集合与 `crates/wiki-core/src/events.rs` 的 `WikiEvent` 变体做
比对。新增事件时，必须同时更新这里。
