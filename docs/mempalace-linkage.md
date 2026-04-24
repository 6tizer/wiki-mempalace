# wiki-mempalace 内部联动契约（workspace）

本文描述 `wiki-*` 系列 crate 与 `rust-mempalace` crate 在 **workspace 内**的协同
契约——从原来的"跨仓库事件契约"演化为"workspace 内 crate 边界契约"。

## 1) 概念映射


| wiki-core                    | rust-mempalace          | 说明                                  |
| ---------------------------- | ----------------------- | ----------------------------------- |
| `RawArtifact`                | `drawers` 行             | 原始资料正文进入 drawer content             |
| `Claim`                      | `kg_facts`              | 可映射为 `(subject, predicate, object)` |
| `Claim.supersedes` / `stale` | `kg_facts.valid_to`     | 新结论写入后，旧事实 `kg_invalidate`          |
| `WikiEvent::SourceIngested`  | `mine_path` / 入库流程触发    | 写侧事件驱动                              |
| `WikiEvent::QueryServed`     | benchmark / telemetry   | 可用于检索效果观测                           |
| `Entity` / `TypedEdge`       | `kg_query` + `traverse` | 图路召回来源                              |


## 2) 当前联动形态：进程内（bridge live feature）

合并后两者同属 workspace，联动走**进程内函数调用**：

- `wiki-mempalace-bridge` 的 `live` feature 打开后，依赖 `rust-mempalace = { path = "../rust-mempalace" }`
- 提供三个实现类：
  - `LiveMempalaceSink`：消费 outbox NDJSON，写入 palace.db 的 drawers / kg_facts / drawer_vectors
  - `LiveMempalaceGraphRanker`：query 的第三路图召回来源
  - `MempalaceSearchPorts`：实现 `wiki-core::SearchPorts`，给三路 RRF 提供 BM25/向量/图候选
  - `LiveMempalaceTools`：实现 10 个 `mempalace_*` MCP 工具，供 `wiki-cli` 通过 bridge 调用

优点：无跨进程序列化开销；结构清晰；测试可内联。
代价：bridge 与 rust-mempalace 的内部 API（`service::search_with_options` /
`service::kg_add` 等）**强耦合**。rust-mempalace 重构 service 层时，bridge 必须同步调整。

## 3) 降级联动形态：进程外（outbox 消费）

即使不启用 `live` feature，`wiki-cli` 也可以把 outbox 导出为 NDJSON，交给任意外部
consumer（哪怕是别的语言写的）消费：

```bash
cargo run -p wiki-cli -- --db wiki.db export-outbox-ndjson-from --last-id 0 > events.ndjson
cargo run -p wiki-cli -- --db wiki.db ack-outbox --up-to-id 999 --consumer-tag my-consumer
```

事件格式：每行一个 JSON 对象，`type` 字段区分类型。完整事件集合、生产者与消费策略见
[docs/outbox-event-matrix.md](outbox-event-matrix.md)。

当前 mempalace 正式消费的只有 3 类事件：

- `source_ingested`
- `claim_upserted`
- `claim_superseded`

其余事件会继续保留在 outbox 中，bridge 只做 `ignored` 统计，不派发到 mempalace。

## 4) 字段映射建议（Claim → kg_facts）

- `subject`: 项目 / 实体主语（由上层规则或 LLM 提取）
- `predicate`: 关系类型（`uses` / `depends_on` / `fixed` 等）
- `object`: 结论对象（库名、版本、配置值）
- `valid_from`: claim 创建或强化时间
- `valid_to`: supersede 时回填
- `source_drawer_id`: 可选，从 `RawArtifact` 联动后回填

## 5) 最小落地流程

1. `wiki-cli` ingest → `wiki-kernel` 写入 sources / claims / pages → flush outbox
2. bridge live sink（或外部 consumer）按 outbox 顺序处理 `ClaimUpserted` /
  `ClaimSuperseded` / `SourceIngested`；其中 `ClaimUpserted` 先还原完整 claim 并走
  `on_claim_upserted`，只有无 resolver 或悬挂事件时才兼容回退到 `on_claim_event`
3. 非 mempalace 消费事件继续保留在 outbox，中间层只把它们计为 `ignored`
4. 在 mempalace 侧写 `drawers` / `kg_facts`，必要时执行 `kg_invalidate`
5. 读侧在 `query/explain --palace-db` 开启时使用 `CompositeSearchPorts`，由
  `MempalaceSearchPorts` 把 mempalace 的候选注入 RRF

## 6) 当前边界

`wiki-cli/src/mcp.rs` 的 10 个 `mempalace_*` MCP 工具已通过
`wiki_mempalace_bridge::make_tools` 调用 `MempalaceTools`。默认 feature 下使用
`NoopMempalaceTools`，`live` feature 下使用 `LiveMempalaceTools` 连接真实 palace。

剩余耦合在 bridge 内部：`live_sink`、`live_search`、`live_ranker`、`live_tools`
仍直接依赖 `rust_mempalace::service` / `db`。这是有意的 crate 边界：替换
mempalace 后端时应主要修改 bridge，而不是 wiki-cli。
