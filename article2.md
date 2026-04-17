# 我们用 Rust 把两个 AI 记忆引擎焊在了一起——22 个 MCP 工具，Agent 终于有了"持久大脑"

> 当 llm-wiki 的知识生命周期遇上 rust-mempalace 的 FTS5 全文检索和时序知识图谱，再通过一个统一的 MCP Server 暴露出来——你得到的不是两个工具，而是一个完整的 Agent 记忆底座。

---

## 先说结论

这一轮大迭代，我们完成了以下工作：

- **rust-mempalace 变成了一个可被依赖的 library crate**
- **wiki-mempalace-bridge 从 Noop 壳子变成了真实的双向数据桥**
- **BM25 检索从 stub 升级到了 FTS5 全文索引**
- **统一 MCP Server 暴露 22 个工具，任何 AI Agent 都能直接调用**
- **ingest-llm 现在自动抽取实体和关系，知识图谱终于会自己生长了**
- **新增 wake-up 协议、自动事件钩子、批量维护命令**

两个独立的 Rust 项目，现在是一个有机整体。

---

## 为什么要做这件事

我们之前做了两个项目：

**llm-wiki**——一个知识生命周期内核。它有 claim 置信度、四层记忆巩固（Working → Episodic → Semantic → Procedural）、遗忘曲线衰减、取代链、三路 RRF 融合检索、Obsidian wiki 投影、审计追踪、多 Agent 隔离。知识管理的"大脑皮层"很发达。

**rust-mempalace**——一个本地优先的记忆宫殿系统。它有 FTS5 全文检索、时序知识图谱（SPO 三元组带 valid_from/valid_to）、MCP Server（10 个工具）、wake-up 上下文协议、稀疏向量检索。搜索和即时召回的"小脑"非常强。

问题是：**两个大脑各跑各的**。

llm-wiki 的 BM25 检索是个 token 重叠的 stub，搜索质量堪忧。rust-mempalace 没有知识生命周期，所有信息一视同仁，没有置信度、没有层级、没有衰减。bridge crate 里只有 `NoopMempalaceSink` 和 `NoopMempalaceGraphRanker`——两个空壳，编译能过，但什么也不做。

rohitg00 的 LLM Wiki v2 gist 描述了一个完整的愿景：生命周期 + 知识图谱 + 混合检索 + 事件驱动 + 自动化 hooks + 多 Agent 协作。我们对照了一下，llm-wiki 覆盖了约 70%，但恰恰是剩下的 30%——**真正的搜索质量、Agent 可调用的接口、两个系统的真实打通**——决定了整个系统能不能从"工程 demo"变成"可用底座"。

所以这次迭代的目标很明确：**焊死这两个引擎，对外只暴露一个 MCP 接口。**

---

## Phase 1：让 rust-mempalace 变成 library

这是一切的前提。rust-mempalace 原来是一个纯 binary crate——`src/main.rs` 里 `mod service; mod db; mod mcp;`，所有好东西锁在二进制里，别人没法 `use rust_mempalace::service::search_with_options`。

改动很简单但很关键：

```rust
// 新建 src/lib.rs
pub mod classifier;
pub mod db;
pub mod llm;
pub mod mcp;
pub mod service;
```

```toml
# Cargo.toml 增加
[lib]
name = "rust_mempalace"
path = "src/lib.rs"

[[bin]]
name = "rust-mempalace"
path = "src/main.rs"
```

`main.rs` 从 `mod service` 改成 `use rust_mempalace::service`，`cli.rs` 保持 binary 私有。

改完跑测试：**9 个 e2e 测试全绿**，行为零变化，但整个 service 层、db 层、mcp 层现在都是公开 API 了。

---

## Phase 2：bridge 不再是空壳

这是整个迭代最有分量的部分。

`wiki-mempalace-bridge` 原来定义了两个 trait：

```rust
pub trait MempalaceWikiSink: Send + Sync {
    fn on_claim_upserted(&self, claim: &Claim) -> Result<(), MempalaceError>;
    fn on_claim_superseded(&self, old: ClaimId, new: ClaimId) -> Result<(), MempalaceError>;
    fn on_source_linked(&self, source_id: SourceId, claim_id: ClaimId) -> Result<(), MempalaceError>;
    fn on_source_ingested(&self, source_id: SourceId) -> Result<(), MempalaceError>;
    fn scope_filter(&self, scope: &Scope) -> bool;
}

pub trait MempalaceGraphRanker: Send + Sync {
    fn graph_rank_extras(&self, query: &str, limit: usize) -> Vec<String>;
}
```

接口设计得很漂亮，但只有 `Noop` 实现。现在我们有了 `LiveMempalaceSink` 和 `LiveMempalaceGraphRanker`。

**写路径（Sink）的核心逻辑：**

- `on_claim_upserted` → 把 claim 文本作为 drawer 插入 mempalace（wing=`wiki_claims`），同时生成稀疏向量
- `on_claim_superseded` → 在 mempalace 的时序知识图谱里插入 `(new_claim, supersedes, old_claim)` 三元组
- `on_source_linked` → 插入 `(source, supports, claim)` 三元组
- `scope_filter` → 把 wiki 的 `Scope::Private{agent_id}` 映射到 mempalace 的 `bank_id`

**读路径（Ranker）的核心逻辑：**

- `graph_rank_extras` → 用 query 在 mempalace 做 FTS5 搜索 + 知识图谱查询，返回排好序的 doc id 列表

全部用 `#[cfg(feature = "live")]` feature-gate，不启用时编译开销为零：

```toml
[features]
default = []
live = ["dep:rust-mempalace", "dep:rusqlite", "dep:sha2", "dep:chrono"]
```

一个细节：`rusqlite::Connection` 不是 `Sync` 的（内部有 `RefCell`），但 trait 要求 `Send + Sync`。我们用 `Mutex<Connection>` 包装，并封装了一个 `with_conn` 闭包模式来简化加锁：

```rust
fn with_conn<F, R>(&self, f: F) -> Result<R, MempalaceError>
where
    F: FnOnce(&Connection) -> Result<R, MempalaceError>,
{
    let conn = self.conn.lock()
        .map_err(|e| MempalaceError::Backend(format!("lock: {e}")))?;
    f(&conn)
}
```

---

## Phase 3：BM25 从玩具变成真家伙

llm-wiki 原来的 `InMemorySearchPorts` 做的是什么？把 query tokenize 成小写 token，然后看 claim 文本里包含几个 token，按数量排序。这不是 BM25，这是 `grep | wc -l`。

现在我们有了 `MempalaceSearchPorts`，它背后是 SQLite FTS5 的真正 BM25 排序：

```sql
SELECT d.id, snippet(drawers_fts, 0, '[', ']', ' ... ', 20)
FROM drawers_fts
JOIN drawers d ON d.id = drawers_fts.rowid
WHERE drawers_fts MATCH ?1
ORDER BY bm25(drawers_fts)
```

FTS5 做了词频统计、逆文档频率、文档长度归一化——这是信息检索领域几十年验证过的经典算法，不是 stub 能比的。

为了让 bridge crate 能实现 `SearchPorts` trait 而不产生循环依赖（bridge → kernel → bridge），我们把 trait 定义从 `wiki-kernel` 搬到了 `wiki-core`：

```
wiki-core  ← 定义 SearchPorts trait
    ↑
wiki-kernel ← 提供 InMemorySearchPorts (stub 实现)
    ↑
wiki-mempalace-bridge ← 提供 MempalaceSearchPorts (FTS5 实现)
```

干净的依赖方向，没有环。

---

## Phase 4：统一 MCP Server——22 个工具，一个入口

这是对外的门面。现在你只需要启动一个进程：

```bash
cargo run -p wiki-cli -- mcp
```

它在 stdin/stdout 上跑 JSON-RPC 2.0（MCP 协议），Agent 一个 `tools/list` 就能拿到全部能力。

**12 个 Wiki 原生工具：**

| 工具 | 能力 |
|---|---|
| `wiki_status` | 知识库统计 |
| `wiki_ingest` | 原始文本入库（自动脱敏） |
| `wiki_ingest_llm` | LLM 驱动的结构化入库 |
| `wiki_file_claim` | 创建知识断言 |
| `wiki_supersede_claim` | 取代旧断言 |
| `wiki_query` | 三路 RRF 混合检索 |
| `wiki_promote_claim` | 断言晋级（Working→Semantic） |
| `wiki_crystallize` | 会话结晶为 wiki 页面 |
| `wiki_lint` | 健康检查 |
| `wiki_wake_up` | Agent 唤醒上下文（L2 语义知识 + L3 活跃上下文） |
| `wiki_maintenance` | 批量维护：衰减 + 检查 + 晋级 |
| `wiki_export_graph_dot` | 导出知识图谱 DOT 格式 |

**10 个 MemPalace 穿透工具：**

| 工具 | 能力 |
|---|---|
| `mempalace_search` | FTS5 混合检索 |
| `mempalace_status` | Palace 概览 |
| `mempalace_wake_up` | L0 身份 + L1 关键事实 |
| `mempalace_taxonomy` | Wing/Hall/Room 分类树 |
| `mempalace_traverse` | 跟随记忆宫殿隧道 |
| `mempalace_kg_query` | 时序知识图谱查询 |
| `mempalace_kg_timeline` | 实体时间线 |
| `mempalace_kg_stats` | 知识图谱统计 |
| `mempalace_reflect` | RAG：搜索 + LLM 综合 |
| `mempalace_extract` | LLM 三元组抽取 |

一个实际调用的例子：

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call",
 "params":{"name":"wiki_status","arguments":{}}}

→ {"claims":0,"pages":0,"entities":0,"sources":0,"audit_records":0}
```

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call",
 "params":{"name":"mempalace_status","arguments":{}}}

→ {"drawers":14,"wings":3,"tunnels":0,"kg_facts":3}
```

两个引擎，一个接口。Agent 不需要知道数据在哪个数据库里。

---

## Phase 5：知识图谱自动生长

之前 `ingest-llm` 只让 LLM 抽取 claims（原子事实断言）。这次我们扩展了 JSON 计划的 schema：

```json
{
  "version": 1,
  "summary_title": "Redis 缓存架构",
  "claims": [
    { "text": "项目使用 Redis 做 L2 缓存", "tier": "semantic" }
  ],
  "entities": [
    { "label": "Redis", "kind": "library" },
    { "label": "项目 X", "kind": "project" }
  ],
  "relationships": [
    { "from_label": "项目 X", "relation": "uses", "to_label": "Redis" }
  ]
}
```

LLM 一次调用，同时产出事实断言 + 实体节点 + 类型化关系边。这些会被写入 wiki engine 的实体图谱，如果 bridge 是 live 的，还会同步推送到 mempalace 的时序知识图谱。

为此我们在 `wiki-core` 的 `EntityKind` 和 `RelationKind` 上加了 `parse()` 方法：

```rust
impl EntityKind {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "person" => Self::Person,
            "project" => Self::Project,
            "library" => Self::Library,
            // ...
            other => Self::Other(other.to_string()),
        }
    }
}
```

用 `#[serde(default)]` 保证向后兼容——旧的 JSON 计划（没有 `entities` 和 `relationships` 字段）照样解析，零迁移成本。

---

## Phase 6 & 7：唤醒协议和自动钩子

**Enhanced Wake-up**

Agent 启动时调用 `wiki_wake_up`，拿到的不是一段泛泛的"欢迎回来"，而是结构化的上下文：

```markdown
# L2 Active Semantic Knowledge
- [conf=0.92, Semantic] 项目使用 Redis 做 L2 缓存，TTL=30min
- [conf=0.88, Procedural] 部署流程：先灰度 10%，观察 15 分钟后全量

# L3 Active Context
## Recent Pages
- Redis 缓存架构分析
- Q1 部署复盘

## Knowledge Graph: 12 entities, 8 edges
```

如果同时调用 `mempalace_wake_up`，还能拿到 L0（身份）和 L1（关键事实）。四层唤醒上下文，从身份认知到具体事实，层层递进。

**AutoWikiHook**

新增的 `AutoWikiHook` 实现了 `WikiHook` trait，在事件发生时自动做记账：

- `ClaimUpserted` → 记录新 claim，后续可用于自动矛盾检测
- `QueryServed` → 追踪查询频率，为 claim 强化提供信号

这是事件驱动自动化的起点。后续可以在此基础上实现"写入时自动矛盾检查""查询后自动强化""定时批量衰减"等全自动的知识库维护。

同步新增了 `maintenance` 命令：

```bash
cargo run -p wiki-cli -- --db wiki.db maintenance
# decay=applied lint_findings=3 promoted=1
```

一行命令完成：全库置信度衰减 → 健康检查 → 符合条件的 claim 自动晋级。

---

## 架构全景

完成这次迭代后，整个系统的数据流如下：

```
                    ┌──────────────────────────────┐
                    │   Unified MCP Server (22)     │
                    │   wiki-cli mcp                │
                    └──────┬───────────┬────────────┘
                           │           │
              Wiki Tools (12)    MemPalace Tools (10)
                           │           │
                    ┌──────▼──┐   ┌────▼──────────┐
                    │wiki.db  │   │palace.db       │
                    │ claims  │──▶│ drawers (FTS5) │
                    │ pages   │   │ kg_facts       │
                    │ entities│   │ drawer_vectors  │
                    │ outbox  │   │ tunnels        │
                    └─────────┘   └────────────────┘
                         │              ▲
                         │   LiveMempalaceSink
                         └──────────────┘

检索管道：
  Query ──┬── BM25 (FTS5 via MemPalace) ──┐
          ├── Vector (sparse/dense)  ──────┤── RRF Fusion ── retention_strength ── Top-K
          └── Graph (KG traverse)    ──────┘
```

两个 SQLite 数据库，各管各的 schema，通过 `LiveMempalaceSink` 单向同步。搜索时通过 `MempalaceSearchPorts` 和 `LiveMempalaceGraphRanker` 反向拉取。两个项目可以独立运行、独立测试，联动时能力叠加。

---

## 验收

```
✅ cargo test --workspace          → 24 tests passed
✅ cargo test (rust-mempalace)     → 9 tests passed  
✅ bash scripts/e2e.sh             → E2E PASS
✅ MCP tools/list                  → 22 tools returned
✅ wiki_status / mempalace_status  → 两个引擎都能通过 MCP 响应
```

---

## 和 LLM Wiki v2 Gist 的对照

| Gist 描述的能力 | 我们的实现状态 |
|---|---|
| 置信度评分 | ✅ 多源凹合并 + 时间衰减 |
| 取代链 | ✅ 链式追溯，旧 claim 保留 |
| 遗忘曲线 | ✅ Ebbinghaus 指数衰减 + 访问强化 |
| 四层巩固 | ✅ W→E→S→P，schema 驱动的晋级阈值 |
| 知识图谱 | ✅ 7 种实体 + 7 种关系 + BFS 遍历 |
| 类型化关系 | ✅ Uses/DependsOn/Contradicts/... |
| 混合检索 (BM25+Vector+Graph) | ✅ 三路 RRF，FTS5 真实 BM25 |
| 自动化 hooks | ✅ AutoWikiHook + maintenance 命令 |
| 质量检查 / lint | ✅ 断链/孤页/过时 claim/缺失交叉引用 |
| 矛盾检测 | ✅ 启发式对检测 |
| 多 Agent 隔离 | ✅ Private/Shared scope 严格过滤 |
| 隐私脱敏 | ✅ Bearer/AWS Key/RSA 自动擦除 |
| 审计追踪 | ✅ UUID + actor + operation + timestamp |
| 结晶化 | ✅ 会话 → wiki 页面 + 候选 claim |
| LLM 自动入库 | ✅ claim + entity + relationship 一次提取 |
| MCP Agent 接口 | ✅ 22 工具统一 MCP Server |
| 唤醒协议 | ✅ L0 身份 + L1 事实 + L2 语义 + L3 上下文 |
| Schema 驱动 | ✅ JSON 可配置的领域 schema |

Karpathy 描述了一个"不断复利的知识 wiki"的构想。rohitg00 补充了生命周期、图谱、自动化的工程细节。**我们把这些全部用 Rust 落了地，而且做了 gist 里没提到的东西**：事件 outbox、MCP 协议、双引擎联动、时序知识图谱。

---

## 写在最后

这次迭代最让我们兴奋的不是某个单独的 feature，而是**两个项目咬合在一起时产生的化学反应**：

- llm-wiki 提供知识的"生命"——生老病死、层级晋升、矛盾取代。
- rust-mempalace 提供知识的"触达"——FTS5 全文检索、时序图谱、MCP 即用接口。
- bridge 是纽带——feature-gate 隔离，开启后数据自动流转，关闭后两边各自安好。

这正是 Rust 在系统编程上的魅力：**trait 定义边界，feature flag 控制耦合度，类型系统保证两边不会在运行时莫名其妙地崩掉**。

如果你也在做 Agent 记忆、知识管理、或者任何需要"LLM 记住东西"的系统——欢迎来看代码，欢迎提 Issue，更欢迎一起把这个底座做厚。

---

*作者：brzhang | 技术栈：Rust · SQLite · FTS5 · RRF · MCP · OpenAI-compatible API · Outbox Pattern*

*项目：llm-wiki + rust-mempalace | 测试覆盖：33 tests + E2E + MCP 集成验证*
