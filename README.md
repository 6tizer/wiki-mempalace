# wiki-mempalace

本地优先（local-first）的**统一知识底座**——把 `llm-wiki`（知识生命周期内核）与
`rust-mempalace`（记忆宫殿 / FTS5 全文检索 / 时序知识图谱）合并为单一 Cargo workspace，
对 AI Agent 暴露两种入口：

- `wiki-agent`：默认交互入口。native Rust direct-call 调用共享 ToolRegistry、`wiki.db`
  和 `palace.db`，支持 CLI chat、TUI、manager-worker sub agents、web RAG 和持久记忆。
- `wiki-cli mcp`：完全兼容的 22 工具 MCP Server。它也是同一套 ToolRegistry 的
  JSON-RPC adapter，不是第二份工具实现。

两个引擎的合体意味着：知识**可累积、可衰减、可 supersede、可审计**，同时**可
全文检索、可按时间回放、可按实体遍历**。

> 原始的分仓设计见 `docs/blog/article2.md`（历史长文）。本仓用 `git subtree`
> 把 `rust-mempalace` 嫁接为 `crates/rust-mempalace/`，保留双方全部历史。

> Vault 文件树与 frontmatter 的**唯一标准**见 [docs/vault-standards.md](docs/vault-standards.md)。
> 所有 source / summary / concept / entity 文件的目录、命名、frontmatter、正文骨架必须遵守该文档；
> 未对齐的内容必须在写入前修复，不得通过新增"兼容写法"绕过。

> **Writer safety**：写入型 CLI / MCP 入口会先获取 `wiki.db.writer.lock`
> writer lease；拿不到 lease 时 fail fast。SQLite transaction 仍负责单次提交原子性，
> lease 负责避免多个进程各自持有旧内存 snapshot 后互相覆盖。多 agent 共享时仍优先
> 共用同一个 MCP server，或串行执行写命令。

> **当前生产数据（2026-05-05）**：`/Users/mac-mini/Documents/wiki/.wiki/wiki.db`
> 已由三库 Notion 全量导出重建，当前包含 `4765` 个 Wiki page 和 `1526`
> 个 source（X `943`，微信 `583`）。`/Users/mac-mini/Documents/wiki` 是
> Obsidian Vault 投影，`/Users/mac-mini/Documents/wiki/.wiki/palace.db` 是
> Mempalace 投影，二者已随本次重建同步刷新。

---

## 仓库结构

```
wiki-mempalace/
├── Cargo.toml                 # workspace
├── DomainSchema.json          # 知识 Schema 实例（v1.0）
├── AGENTS.md                  # Agent 工作流规范
├── Progress.md                # 开发日志
├── scripts/
│   ├── e2e.sh                 # 端到端回归脚本
│   └── backup.sh              # Dogfood 备份脚本（D4）
├── docs/
│   ├── README.md              # 文档入口：当前事实 / 活跃计划 / 历史归档
│   ├── roadmap.md             # 当前路线图与生产状态
│   ├── dev-workflow.md        # PRD/spec/branch/subagent/review/PR/CI 固定开发流程
│   ├── LESSONS.md             # 每轮合并后的项目级经验
│   ├── automation-issue-batch-3.md # 历史 batch-3 任务规划（当前入口以 roadmap/specs 为准）
│   ├── prd/                   # 当前批次 PRD
│   ├── specs/                 # 每个功能模块的 spec 三件套
│   ├── handovers/             # subagent 模块交接文档
│   ├── templates/             # PRD/spec/subagent/review 模板
│   ├── vault-standards.md     # Vault 目录/命名/frontmatter/正文骨架唯一标准
│   ├── architecture.md        # 架构图 + 业务流转
│   ├── mempalace-linkage.md   # workspace 内 crate 协同契约
│   ├── outbox-and-consumers.md# outbox 事件与消费者契约
│   ├── schema-followup-plan.md# Schema / tag 后续计划
│   ├── longmemeval.md         # 长期记忆评测说明
│   ├── archive/               # 历史计划与已完成批次
│   └── blog/
│       └── article2.md        # 两仓合并前的工程长文
└── crates/
    ├── wiki-ai/               # 共享 LLM profile / embedding / Exa/xAI web search runtime
    ├── wiki-core/             # 领域模型：Claim / Entity / Event / Schema
    ├── wiki-kernel/           # 引擎：ingest / query / lint / promote / crystallize
    ├── wiki-storage/          # SQLite 持久化
    ├── wiki-cli/              # 统一 CLI + MCP Server（22 工具）
    ├── wiki-tools/            # 22 个 MCP/native tools 的唯一 Rust 实现
    ├── wiki-agent/            # CLI chat / TUI / manager-worker / memory entrypoint
    ├── wiki-mempalace-bridge/ # 事件桥 + 搜索 ports（live feature 连 palace）
    ├── wiki-migration-notion/ # Notion Export → 本地 Obsidian vault 一次性迁移工具（含 audit-orphans / fix-orphans）
    └── rust-mempalace/        # 记忆宫殿（lib + bin）；保留独立 README 与 e2e 测试
```

## 快速开始

### 构建

```bash
cargo build --workspace --release
```

### 最小冒烟

> **参数位置约定**：`--db` / `--wiki-dir` / `--sync-wiki` / `--viewer-scope` /
> `--vectors` / `--llm-config` / `--schema` / `--graph-extras-file` / `--palace` 都是**顶层 global**
> 参数，必须放在子命令**之前**。所有写入类子命令在完成后会自动持久化并 flush outbox，
> 无需手动调用 `save_snapshot` / `flush_outbox`。

### 生产 vault-local 启动

#### wiki-agent 交互入口

```bash
cargo run -p wiki-agent -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  --viewer-scope shared:wiki \
  --palace /Users/mac-mini/Documents/wiki/.wiki/palace.db \
  chat --profile agent_manager
```

TUI：

```bash
cargo run -p wiki-agent -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  --viewer-scope shared:wiki \
  --palace /Users/mac-mini/Documents/wiki/.wiki/palace.db \
  tui --profile agent_manager
```

`wiki-agent` 默认 `--tool-backend native`，直接调用 `wiki-tools::ToolRegistry`。
`--tool-backend mcp-child` 只用于兼容外部 MCP 子进程 fallback。

#### MCP Server

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  --palace /Users/mac-mini/Documents/wiki/.wiki/palace.db \
  mcp
```

上面这条会启动当前生产知识库的统一 MCP Server。`wiki.db` 是写入真源；
Obsidian Vault 和 `palace.db` 都是投影层。

### 本地最小冒烟示例

```bash
# 1) ingest 一条原文（脱敏 + 落 SQLite + 投影 Markdown）
cargo run -p wiki-cli -- \
  --db wiki.db --wiki-dir wiki --sync-wiki \
  ingest "file:///notes/a.md" "项目使用 Redis 作缓存" \
  --scope private:cli

# 1b) 扫描 vault 中 `compiled_to_wiki: false` 的 source，逐条走 LLM 抽取 + 落库
cargo run -p wiki-cli -- --db wiki.db --wiki-dir ~/Documents/wiki --sync-wiki \
  batch-ingest --vault ~/Documents/wiki --delay-secs 1

# 2) query 混合三路（BM25 + 向量 + 图）
cargo run -p wiki-cli -- --db wiki.db query "Redis 缓存"

# 3) lint 基线检查（完整度 + 孤儿页 + claim 过期）
cargo run -p wiki-cli -- --db wiki.db --wiki-dir wiki --sync-wiki lint

# 4) Claim 生命周期：录入 → supersede → 手动 promote
cargo run -p wiki-cli -- --db wiki.db \
  file-claim "项目使用 Redis" --scope private:cli --tier semantic
cargo run -p wiki-cli -- --db wiki.db \
  supersede-claim <old_claim_id> "项目改用 DragonflyDB" --scope private:cli --tier semantic
cargo run -p wiki-cli -- --db wiki.db promote <claim_id>

# 5) 页面生命周期推进（Draft → InReview → Approved）
cargo run -p wiki-cli -- --db wiki.db promote-page <page_id>            # 自动下一跳
cargo run -p wiki-cli -- --db wiki.db promote-page <page_id> --to Approved --force

# 6) 维护类：置信度衰减 + lint + 批量 promote
cargo run -p wiki-cli -- --db wiki.db --wiki-dir wiki --sync-wiki maintenance

# 7) 会话结晶为一张页面
cargo run -p wiki-cli -- --db wiki.db \
  crystallize "如何选型向量库？" \
  --finding "pgvector 足够" --file "docs/roadmap.md" --lesson "先量后换"

# 8) Schema 快速校验（不需要 DB）
cargo run -p wiki-cli -- schema-validate DomainSchema.json

# 9) 启动统一 MCP Server（stdio JSON-RPC）
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki --sync-wiki \
  --viewer-scope private:cli \
  mcp

# 10) wiki-agent native doctor / chat
cargo run -p wiki-agent -- --db wiki.db doctor --tool-backend native
cargo run -p wiki-agent -- --db wiki.db chat "Redis 缓存怎么查？" --web off

# 11) 查看统一 metrics（默认只读；可写 JSON 或 Markdown 报告）
cargo run -p wiki-cli -- --db wiki.db metrics --json --report wiki/reports/metrics.md
```

完整子命令列表见 [AGENTS.md](AGENTS.md)（含 `ingest-llm`、`export-outbox-ndjson[-from]`、
`ack-outbox`、`consume-to-mempalace`、`llm-smoke` 等）。

### 端到端回归

```bash
./scripts/e2e.sh
```

覆盖：ingest → file-claim → supersede → query write-page → lint → outbox export/ack →
mempalace consumer → viewer-scope 隔离 → wiki-agent native/mcp-child doctor →
wiki-agent fake chat + memory extraction → llm-smoke（可选）。

### 测试

```bash
# 全量 workspace 测试
cargo test --workspace

# rust-mempalace crate 级 e2e（8 个 e2e_core 用例，子进程级）
cargo test -p rust-mempalace --test e2e_core
```

---

## 架构概览

```
wiki-cli (binary)
  └─ MCP adapter → wiki-tools::ToolRegistry（22 tools）

wiki-agent (binary)
  └─ native ToolRegistry + chat/TUI + manager-worker + memory
       ├─ wiki_*  (12) → wiki-kernel → wiki-core / wiki-storage
       └─ mempalace_* (10) → wiki-mempalace-bridge → rust-mempalace::service

wiki-kernel emit WikiEvent → outbox → wiki-mempalace-bridge (live) → rust-mempalace
                                                                          │
                                                                 palace SQLite
                                                                   drawers / kg_facts
                                                                   drawer_vectors
```

详见 [docs/architecture.md](docs/architecture.md)。

---

## 工作流规范

Agent 或 CLI 使用者请优先阅读 [AGENTS.md](AGENTS.md)，其中定义了 6 步稳定流程：
`ingest → query / write-page → lint → outbox export / ack → supersede → llm ingest`。

## 文档索引

- [AGENTS.md](AGENTS.md)：面向 Agent 的稳定工作流
- [DomainSchema.json](DomainSchema.json)：领域 Schema v1.0 实例
- [docs/README.md](docs/README.md)：文档入口与分类
- [docs/roadmap.md](docs/roadmap.md)：当前路线图
- [docs/vault-standards.md](docs/vault-standards.md)：**Vault 目录/命名/frontmatter/正文骨架唯一标准**
- [docs/architecture.md](docs/architecture.md)：架构图与业务流
- [docs/mempalace-linkage.md](docs/mempalace-linkage.md)：bridge 契约与数据映射
- [docs/mcp-api-reference.md](docs/mcp-api-reference.md)：统一 MCP Server 工具参数、scope 和副作用参考
- [docs/archive/README.md](docs/archive/README.md)：历史计划归档
- [Progress.md](Progress.md)：每轮工作日志
- [crates/rust-mempalace/README.md](crates/rust-mempalace/README.md)：
`rust-mempalace` 作为独立 crate 的原始说明（保留）

## 许可

`MIT OR Apache-2.0`
