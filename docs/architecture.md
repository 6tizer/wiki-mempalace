# wiki-mempalace 架构与业务流

本文描述合并后的统一仓库架构，以及 ingest / query / 事件桥接的端到端业务流。

---

## 1. Crate 依赖拓扑

```
                     ┌────────────────────────────┐
                     │        wiki-cli            │
                     │   binary + MCP server      │
                     │   （22 tools over stdio）  │
                     └────────────────────────────┘
                          │         │         │
                          ▼         ▼         ▼
         ┌────────────────┐   ┌────────────┐   ┌──────────────┐
         │  wiki-kernel   │   │ wiki-memp- │   │rust-mempalace│
         │                │   │ alace-     │   │   (direct,   │
         │ LlmWikiEngine  │   │ bridge     │   │   Phase 6 归 │
         │ hooks / memory │   │ (live)     │   │   bridge)    │
         │ wiki_writer    │   │            │   │              │
         │ search_ports   │   └────────────┘   └──────────────┘
         └────────────────┘        │                   │
           │         │             │                   │
           ▼         ▼             ▼                   │
    ┌───────────┐ ┌────────────┐   │                   │
    │wiki-core  │ │wiki-storage│   │                   │
    │ 领域模型  │ │ SQLite     │   │                   │
    └───────────┘ └────────────┘   │                   │
                                   └──────┬────────────┘
                                          │
                                          ▼
                             （进程内 rust-mempalace lib 调用）
```

`wiki-cli` 依赖所有其他 crate。`wiki-mempalace-bridge` 的 `live` feature 会把
`rust-mempalace` 作为进程内 library 调用。`wiki-cli` 当前直接 `use rust_mempalace::service::*` 以实现 10 个 `mempalace_*` MCP 工具——这条旁路在
Phase 6 会收敛到 bridge。

---

## 2. 数据存储

有两份独立 SQLite：


| 文件                          | 归属             | 用途                                            |
| --------------------------- | -------------- | --------------------------------------------- |
| `wiki.db`                   | wiki-storage   | snapshots / outbox / embeddings / audits      |
| `~/.mempalace-rs/palace.db` | rust-mempalace | drawers / drawer_vectors / kg_facts / tunnels |


两个库**不共享连接池**，靠事件桥（outbox NDJSON）保持最终一致。

---

## 3. Ingest 业务流

```
user CLI --------------> Cmd::Ingest (wiki-cli/main.rs)
                                │
                                ▼
                 LlmWikiEngine::ingest_raw
                    ├─ redact_for_ingest (脱敏)
                    ├─ RawArtifact 入 InMemoryStore
                    ├─ emit WikiEvent::SourceIngested  ──▶ hook.on_event
                    └─ audit 写内存 audit log
                                │
                                ▼
                 engine.save_to_repo(&repo)
                    InMemoryStore → wiki.db.snapshots
                                │
                                ▼
                 engine.flush_outbox_to_repo_with_policy
                    WikiEvent → wiki.db.outbox（带 consumer 游标）
                                │
                     (可选 --sync-wiki)
                                ▼
                 write_projection(wiki_root, store, audits)
                    → wiki/index.md, pages/**/*.md, log.md

─── 异步消费 ───（consume-to-mempalace --last-id N 或 export-outbox-ndjson）

repo.export_outbox_ndjson_from_id(last)
    wiki.db.outbox → NDJSON
                │
                ▼
bridge::consume_outbox_ndjson
    逐行反序列化 WikiEvent，按事件类型分发：
      ├─ ClaimUpserted   → sink.on_claim_upserted（无 resolver/悬挂事件时才回退 on_claim_event）
      ├─ ClaimSuperseded → sink.on_claim_superseded
      ├─ SourceIngested  → sink.on_source_ingested
      └─ 其他 WikiEvent  → retained in outbox + bridge 统计为 ignored
                │
                ▼
LiveMempalaceSink (live feature)
    rust_mempalace::db::open(palace_db)
    insert_drawer（content_hash 去重） → palace.db.drawers
    service::upsert_vector            → palace.db.drawer_vectors
    service::kg_add("supersedes")     → palace.db.kg_facts
    service::kg_invalidate("is_active")
```

---

## 4. Query 业务流（三路 RRF）

```
user query "Redis 缓存"
        │
        ▼
LlmWikiEngine::query_pipeline_memory
    │
    ├─ 路 1：BM25 / 词法（InMemoryStore 内文本重叠）
    │
    ├─ 路 2：向量余弦（wiki.db.embedding，需 --vectors + [embed]）
    │
    └─ 路 3：图召回
           ├─ 内核自身：walk_entities (wiki-core/graph.rs)
           └─ MempalaceGraphRanker 追加外部候选
                    │
                    ▼
              LiveMempalaceGraphRanker
                 rust_mempalace::service::search_with_options
                     → "mp_drawer:<id>"
                 rust_mempalace::service::kg_query
                     → "mp_kg:<subject>:<predicate>"
        │
        ▼
merge_graph_rankings (wiki-kernel/search_ports.rs)
   三路 doc_id 用 RRF (reciprocal rank fusion) 融合
        │
        ▼
rank_fused_with_retention
   按 claim.retention_strength 做半衰期衰减加权
        │
        ▼
WikiEvent::QueryServed  (emit outbox)
        │
        ▼
Vec<(doc_id, score)>
        │
        ▼ （可选 --write-page）
WikiPage::new → engine.file_page → store.pages → write_projection
```

---

## 5. MCP Server 工具清单（22 tools）


| 前缀            | 工具                                                                                                                                       | 实现路径                                                      |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| `wiki_*`      | status, ingest, file_claim, supersede_claim, query, promote_claim, crystallize, lint, wake_up, maintenance, export_graph_dot, ingest_llm | 走 `wiki-kernel::LlmWikiEngine`                            |
| `mempalace_*` | status, search, wake_up, taxonomy, traverse, kg_query, kg_timeline, kg_stats, reflect, extract                                           | 当前直接 `use rust_mempalace::service::*`（Phase 6 归一到 bridge） |


启动方式：`cargo run -p wiki-cli -- --db wiki.db mcp --palace ~/.mempalace-rs`。

---

## 6. 数据模型映射（wiki ↔ mempalace）


| wiki-core                    | rust-mempalace          | 说明                                     |
| ---------------------------- | ----------------------- | -------------------------------------- |
| `RawArtifact`                | `drawers` 行             | 原始资料正文进入 drawer content                |
| `Claim`                      | `kg_facts`              | `(subject, predicate, object)` SPO 三元组 |
| `Claim.supersedes` / `stale` | `kg_facts.valid_to`     | 新结论写入后 `kg_invalidate` 旧事实             |
| `WikiEvent::SourceIngested`  | `mine_path` 入库事件        | 桥接触发 drawer 写入                         |
| `WikiEvent::ClaimUpserted`   | `drawers` / `drawer_vectors` 写入 | 事件驱动                           |
| `Entity` / `TypedEdge`       | `kg_query` + `traverse` | 图路召回来源                                 |


映射细节见 [docs/mempalace-linkage.md](mempalace-linkage.md)。

完整 outbox 事件矩阵、生产者和当前消费策略见
[docs/outbox-event-matrix.md](outbox-event-matrix.md)。当前 mempalace 只正式消费
`SourceIngested`、`ClaimUpserted`、`ClaimSuperseded`，其他事件仍写入 outbox，但 bridge
不会派发到 mempalace。

---

## 7. 已知架构债（Phase 6 修复）

1. **旁路依赖**：`wiki-cli/src/mcp.rs` 绕开 `wiki-mempalace-bridge` 直接 `use
  rust_mempalace::service::`*。应让所有外部依赖收敛到 bridge，
   未来替换 mempalace 后端改一处即可。
2. **Edition 版本不齐**：workspace `edition = "2021"`，`rust-mempalace` 独立
  `edition = "2024"`。等 workspace 整体升 2024 再统一。
3. **两份 SQLite 最终一致**：依赖 outbox 消费器手动触发。未来可考虑内核 hook 直连
  bridge live sink，实现准实时一致。
