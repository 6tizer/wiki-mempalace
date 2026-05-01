# wiki-mempalace 架构与业务流

本文描述当前统一 workspace 架构，以及 ingest / query / outbox / MCP 的端到端业务流。

## 0. 数据来源层与系统方向性约束

### 整体链条

```text
┌─────────────────────────────────────────────────────┐
│                  原始内容来源层                        │
│                                                     │
│  A. Notion DB (X书签 / 微信文章 DB)                   │
│     └─ 离线导出 zip → wiki-migration-notion 解析      │
│                                                     │
│  B. Agent / 人工 (通过 MCP 或 CLI)                    │
│     └─ wiki_ingest / wiki_file_claim / ingest-llm   │
│                                                     │
│  C. Notion API 增量同步 (已实现 notion-sync)          │
└───────────────────┬─────────────────────────────────┘
                    │ ingest / file-claim / batch-ingest
                    ▼
          ┌─────────────────┐
          │    wiki.db      │  ← 唯一写入中心 (single source of truth)
          │  RawArtifact    │
          │  Claim          │
          │  WikiPage       │
          │  wiki_outbox    │
          └────────┬────────┘
                   │
        ┌──────────┴──────────┐
        │                     │
        ▼                     ▼
  palace.db               Obsidian Vault (.md)
  (Mempalace)         (write_projection / --sync-wiki)
  drawers / kg_facts      pages/ index.md log.md
        │                     │
        │                     └── 你能观察和读取的层
        └── 检索融合 (--palace-db)
```

### 方向性约束（重要）

1. **wiki.db 是唯一写入中心。** Vault（Obsidian .md 文件）和 palace.db 都是派生输出，不是输入。
2. **不要直接修改 Vault 的 .md 文件来影响数据库。** Vault 文件由 `write_projection` 管理，手动修改会在下次 `--sync-wiki` 时被覆盖（对于 engine-managed 的 pages）。
3. **你观察 Vault 后发现的问题，修复路径是：** 在 vault 中定位问题 → 分析对应的 wiki.db 数据 → 修改程序逻辑或通过 CLI/MCP 写入 → 再次运行 `--sync-wiki` 输出到 Vault。
4. **Vault 中的 `sources/` 目录和手动迁移文件不由引擎管理。** `write_projection` 只维护 `pages/{entry_type}/`、`index.md`、`log.md`；`pages/` 文件必须同时带 `managed_by: wiki-mempalace` 与 `managed_kind: wiki-page-projection` marker 才会被 cleanup 视为 engine-managed。

### 当前各来源状态

| 来源 | 接入方式 | 状态 | 备注 |
| --- | --- | --- | --- |
| Notion DB (X书签) | `notion-sync` + 历史离线迁移 | ✅ 已接入 | 支持 cursor 增量同步、已有 source refresh、Vault source projection |
| Notion DB (微信文章) | `notion-sync` + 历史离线迁移 | ✅ 已接入 | 同上 |
| Agent MCP 写入 | `wiki_ingest` / `wiki_file_claim` / `wiki_ingest_llm` | ✅ 实时生效 | 通过 MCP server 实时写 wiki.db |
| CLI 手动写入 | `ingest` / `file-claim` / `batch-ingest` | ✅ 实时生效 | 直接调 CLI |
| Notion API 增量同步 | `wiki-cli notion-sync` / automation `notion-sync` | ✅ 已合入 | PR #36/#38/#42；archived source retirement 已由 PR #68/#69 收敛 |
| Governance / Fixer | `governance scan` / `fixer-plan` / `fixer-apply` / `restore` | ✅ 已接入 | 只以 `wiki.db` 为真源；Fixer apply 写 DB/outbox/projection，不直接改 palace |
| Synthesis | `research-synthesis discover` / `compose` / `run` | ✅ 已接入 | 内部 evidence + 双 provider web evidence + LLM draft/verifier；输出 `entry_type=synthesis` |

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
  -> save_to_repo_and_flush_outbox_with_policy()
  -> save_snapshot_and_append_outbox() transaction
  -> wiki_outbox
  -> wiki_outbox_consumer_progress 按 consumer_tag 记录 ack
  -> 可选 write_projection()
```

写入类 CLI 子命令会自动在同一 SQLite transaction 中保存 snapshot 与本次 outbox，
并在 `--sync-wiki` 开启时写 Markdown projection。`write_projection` 只维护
`pages/{entry_type}/`、`index.md`、`log.md`，只清理带显式 managed marker 且已不在当前 store 中的 projection page；清理结果进入 `.wiki/trash/projection/`。

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
  -> storage-backed SqliteSearchPorts
  -> 有 --palace-db: CompositeSearchPorts(SqliteSearchPorts, MempalaceSearchPorts)
  -> query_ranked_with_ports
  -> RRF + retention 加权
  -> QueryServed event
```

wiki 内部检索默认从 `wiki.db` snapshot 提供 BM25 / vector / graph 三路候选；测试或无 storage fallback 才走 in-memory ports。传入 `--palace-db` 后，
`MempalaceSearchPorts` 会从 `palace.db` 注入 `mp_drawer:*` / `mp_kg:*` 候选，
再由 `CompositeSearchPorts` 去重并进入同一套 RRF 排序。

## 6. Governance / Fixer / Synthesis

```text
governance scan
  -> read wiki.db snapshot
  -> reports/governance/*
  -> fixer-plan
  -> fixer-apply --policy evidence-auto --apply
  -> save_to_repo_and_flush_outbox_with_policy()
  -> optional write_projection()

research-synthesis run --apply
  -> governance scan
  -> synthesis discovery
  -> internal evidence pack
  -> Exa/xAI web evidence
  -> synthesis_writer + synthesis_verifier profiles
  -> WikiPage(entry_type=synthesis, status=in_review, confidence=high)
```

自动化日常链路：

```text
notion-sync
-> batch-ingest
-> governance-scan
-> fixer-plan
-> fixer-apply
-> maintenance
-> consume-to-mempalace
-> vault-reports
```

`synthesis-discover` 和 `synthesis-run` 是 manual automation jobs，不进入 daily
lane。原因是 synthesis 可能发起外部搜索和较长 LLM 写作，应该按研究节奏单独触发。

`--graph-extras-file` 中的 `claim:` / `page:` / `entity:` / `source:` 会按
`--viewer-scope` 过滤；外部 `mp_drawer:` / `mp_kg:` 注入默认拒绝，避免绕过 mempalace bank/scope。

## 7. MCP Server 工具清单

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

## 8. 当前架构债

- workspace 统一使用 `edition = "2021"`；`rust-mempalace` 通过 `edition.workspace = true` 继承，不再独立声明 edition。
- `wiki.db` 与 `palace.db` 仍是最终一致；准实时同步可在未来通过内核 hook 直连 bridge live sink。
- 当前完成状态见 [roadmap.md](roadmap.md)。
