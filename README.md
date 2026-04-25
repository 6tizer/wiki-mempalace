# wiki-mempalace

本地优先（local-first）的**统一知识底座**——把 `llm-wiki`（知识生命周期内核）与
`rust-mempalace`（记忆宫殿 / FTS5 全文检索 / 时序知识图谱）合并为单一 Cargo workspace，
对 AI Agent 暴露一个 **22 工具的统一 MCP Server**：12 个 `wiki_`* + 10 个 `mempalace_`*。

两个引擎的合体意味着：知识**可累积、可衰减、可 supersede、可审计**，同时**可
全文检索、可按时间回放、可按实体遍历**。

> 原始的分仓设计见 `docs/blog/article2.md`（历史长文）。本仓用 `git subtree`
> 把 `rust-mempalace` 嫁接为 `crates/rust-mempalace/`，保留双方全部历史。

> Vault 文件树与 frontmatter 的**唯一标准**见 [docs/vault-standards.md](docs/vault-standards.md)。
> 所有 source / summary / concept / entity 文件的目录、命名、frontmatter、正文骨架必须遵守该文档；
> 未对齐的内容必须在写入前修复，不得通过新增"兼容写法"绕过。

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
│   ├── roadmap.md             # 当前路线图（M12 / LongMemEval 为主要剩余项）
│   ├── dev-workflow.md        # PRD/spec/branch/subagent/review/PR/CI 固定开发流程
│   ├── LESSONS.md             # 每轮合并后的项目级经验
│   ├── automation-issue-batch-3.md # 当前 batch-3 任务规划
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
    ├── wiki-core/             # 领域模型：Claim / Entity / Event / Schema
    ├── wiki-kernel/           # 引擎：ingest / query / lint / promote / crystallize
    ├── wiki-storage/          # SQLite 持久化
    ├── wiki-cli/              # 统一 CLI + MCP Server（22 工具）
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
> `--vectors` / `--llm-config` / `--schema` / `--graph-extras-file` 都是**顶层 global**
> 参数，必须放在子命令**之前**。所有写入类子命令在完成后会自动持久化并 flush outbox，
> 无需手动调用 `save_snapshot` / `flush_outbox`。

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
cargo run -p wiki-cli -- --db wiki.db mcp --palace ~/.mempalace-rs

# 10) 查看统一 metrics（默认只读；可写 JSON 或 Markdown 报告）
cargo run -p wiki-cli -- --db wiki.db metrics --json --report wiki/reports/metrics.md
```

完整子命令列表见 [AGENTS.md](AGENTS.md)（含 `ingest-llm`、`export-outbox-ndjson[-from]`、
`ack-outbox`、`consume-to-mempalace`、`llm-smoke` 等）。

### 端到端回归

```bash
./scripts/e2e.sh
```

覆盖：ingest → file-claim → supersede → query write-page → lint → outbox export/ack →
mempalace consumer → viewer-scope 隔离 → llm-smoke（可选）。

### 测试

```bash
# 全量（workspace 100+ 个测试）
cargo test --workspace

# rust-mempalace crate 级 e2e（8 个 e2e_core 用例，子进程级）
cargo test -p rust-mempalace --test e2e_core
```

---

## 架构概览

```
wiki-cli (binary)
  └─ MCP Server（22 tools）
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
- [docs/archive/README.md](docs/archive/README.md)：历史计划归档
- [Progress.md](Progress.md)：每轮工作日志
- [crates/rust-mempalace/README.md](crates/rust-mempalace/README.md)：
`rust-mempalace` 作为独立 crate 的原始说明（保留）

## 许可

`MIT OR Apache-2.0`
