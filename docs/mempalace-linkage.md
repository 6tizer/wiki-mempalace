# wiki-mempalace 内部联动契约（workspace）

本文描述 `wiki-*` 系列 crate 与 `rust-mempalace` crate 在 **workspace 内**的协同
契约——从原来的"跨仓库事件契约"演化为"workspace 内 crate 边界契约"。

## 1) 概念映射

| wiki-core                    | rust-mempalace          | 说明                                  |
| ---------------------------- | ----------------------- | ----------------------------------- |
| `RawArtifact`                | `drawers` 行             | 原始资料正文进入 drawer content             |
| `Claim`                      | `kg_facts`              | 可映射为 `(subject, predicate, object)` |
| `Claim.supersedes` / `stale` | `kg_facts.valid_to`     | 新结论写入后，旧事实 `kg_invalidate`          |
| `WikiEvent::SourceIngested`  | `mine_path` / 入库流程触发   | 写侧事件驱动                              |
| `WikiEvent::QueryServed`     | benchmark / telemetry   | 可用于检索效果观测                           |
| `Entity` / `TypedEdge`       | `kg_query` + `traverse` | 图路召回来源                              |

## 2) 当前联动形态：进程内（bridge live feature）

合并后两者同属 workspace，联动走**进程内函数调用**：

- `wiki-mempalace-bridge` 的 `live` feature 打开后，依赖 `rust-mempalace = { path = "../rust-mempalace" }`
- 提供三个实现类：
  - `LiveMempalaceSink`：消费 outbox NDJSON，写入 palace.db 的 drawers / kg_facts / drawer_vectors
  - `LiveMempalaceGraphRanker`：query 的第三路图召回来源
  - `MempalaceSearchPorts`：实现 `wiki-core::SearchPorts`，给三路 RRF 提供 BM25/向量/图候选

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

事件格式：每行一个 JSON 对象，`type` 字段区分类型（`source_ingested` /
`claim_upserted` / `claim_superseded` / `page_written` / `query_served` /
`session_crystallized` / `graph_expanded` / `lint_run_finished`）。

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
   `ClaimSuperseded` / `SourceIngested`
3. 在 mempalace 侧写 `drawers` / `kg_facts`，必要时执行 `kg_invalidate`
4. 读侧查询继续由各自系统负责，`MempalaceSearchPorts` 把 mempalace 的候选注入 RRF

## 6) 未来收敛方向（Phase 6）

当前 `wiki-cli/src/mcp.rs` 的 10 个 `mempalace_*` MCP 工具**绕开 bridge**直接 `use
rust_mempalace::service::*;`，违反本契约的抽象边界。Phase 6 会把这些工具的实现
挪到 bridge（新增一个 `MempalaceTools` trait 或扩展现有 trait），让 `wiki-cli`
不再直接依赖 `rust-mempalace`，bridge 成为唯一的对 mempalace 访问层。
