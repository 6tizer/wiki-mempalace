# wiki-mempalace 架构与业务流

本文描述当前统一 workspace 架构，以及 ingest / query / outbox / MCP 的端到端业务流。

## 1. Crate 依赖拓扑

```text
wiki-cli
  ├─ wiki-kernel
  │   ├─ wiki-core
  │   └─ wiki-storage
  ├─ wiki-mempalace-bridge
  │   └─ rust-mempalace (live feature)
  └─ MCP server (22 tools)
```

`wiki-cli` 是统一 CLI 与 MCP 入口。wiki 侧能力通过 `wiki-kernel` / `wiki-storage`
完成；mempalace 侧能力通过 `wiki-mempalace-bridge` 完成。10 个 `mempalace_*`
MCP 工具通过 `wiki_mempalace_bridge::make_tools` 进入 `MempalaceTools` 抽象，
不由 `wiki-cli` 直接调用 `rust_mempalace::service`。

## 2. 数据存储

| 文件 | 归属 | 用途 |
| --- | --- | --- |
| `wiki.db` | `wiki-storage` | snapshots / outbox / embeddings / automation run state |
| `palace.db` | `rust-mempalace` | drawers / drawer_vectors / kg_facts / tunnels |

两份 SQLite 不共享连接池。`wiki.db` 是主写入中心；`palace.db` 是 outbox 消费后的派生记忆层。

## 3. Ingest 与 outbox

```text
CLI ingest / ingest-llm / batch-ingest
  -> LlmWikiEngine 写 RawArtifact / Claim / WikiPage
  -> save_to_repo()
  -> flush_outbox_to_repo_with_policy()
  -> wiki_outbox
  -> wiki_outbox_consumer_progress 按 consumer_tag 记录 ack
  -> 可选 write_projection()
```

写入类 CLI 子命令会自动保存 snapshot、flush outbox，并在 `--sync-wiki` 开启时写
Markdown projection。`write_projection` 只维护 `pages/{entry_type}/`、`index.md`、
`log.md`，并清理带合法 page-id frontmatter 但已不在当前 store 中的 managed page。

## 4. mempalace 消费

```text
consume-to-mempalace
  -> export_outbox_ndjson_from_id(progress)
  -> bridge consume_outbox_ndjson_with_resolver_and_stats
  -> MempalaceWikiSink
  -> ack wiki_outbox_consumer_progress
```

当前正式消费到 mempalace 的事件：

- `PageWritten`
- `ClaimUpserted`
- `ClaimSuperseded`

`SourceIngested` 仍可进入 outbox，但 live sink 默认 no-op；历史 backfill 只把 source 写入
`wiki.db`，不默认把 source 原文塞进 palace。`PageWritten` 只把 summary / concept /
entity / synthesis / qa 等高质量页面写入 `wiki_pages` drawer。

其他 `WikiEvent` 保留在 outbox 中，bridge 统计为 `ignored`，不派发到 mempalace。

`consume-to-mempalace --palace <db>` 使用 `LiveMempalaceSink` 写真实 palace。live bank
由 `--viewer-scope` 派生，例如 `private:cli -> cli`、`shared:team1 -> team1`。

## 5. Query 与融合检索

```text
query / explain
  -> run_fusion_query
  -> 无 --palace-db: InMemorySearchPorts
  -> 有 --palace-db: CompositeSearchPorts(InMemorySearchPorts, MempalaceSearchPorts)
  -> query_ranked_with_ports
  -> RRF + retention 加权
  -> QueryServed event
```

wiki 内部检索提供 BM25 / vector / graph 三路候选。传入 `--palace-db` 后，
`MempalaceSearchPorts` 会从 `palace.db` 注入 `mp_drawer:*` / `mp_kg:*` 候选，
再由 `CompositeSearchPorts` 去重并进入同一套 RRF 排序。

`--graph-extras-file` 中的 `claim:` / `page:` / `entity:` / `source:` 会按
`--viewer-scope` 过滤；`mp_drawer:` / `mp_kg:` 作为外部 mempalace id 保留。

## 6. MCP Server 工具清单

| 前缀 | 工具 | 实现路径 |
| --- | --- | --- |
| `wiki_*` | status, ingest, file_claim, supersede_claim, query, promote_claim, crystallize, lint, wake_up, maintenance, export_graph_dot, ingest_llm | `wiki-kernel::LlmWikiEngine` |
| `mempalace_*` | status, search, wake_up, taxonomy, traverse, kg_query, kg_timeline, kg_stats, reflect, extract | `wiki-mempalace-bridge::MempalaceTools` |

启动方式：

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  --viewer-scope shared:wiki \
  --palace /Users/mac-mini/Documents/wiki/.wiki/palace.db \
  mcp
```

## 7. 当前架构债

- workspace `edition = "2021"`，`rust-mempalace` 独立 `edition = "2024"`；整体升级时再统一。
- `wiki.db` 与 `palace.db` 仍是最终一致；准实时同步可在未来通过内核 hook 直连 bridge live sink。
- M10 metrics、M11 dashboard 与 M12 strategy 已合入；当前状态见 [roadmap.md](roadmap.md)。
