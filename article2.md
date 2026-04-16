# 我们用 Rust 给 LLM 造了一个"持久记忆内核"——四项新能力全揭秘

> 这不是又一个 RAG 教程。这是一次认真的工程实践：用 Rust 把 AI 的"记忆"做成可以长期维护、多 Agent 隔离、向量检索、LLM 自动入库的工业级内核。

---

## 背景：为什么 RAG 不够用？

每次向 LLM 提问时"现查现用"的 RAG，本质上是无状态的。你昨天告诉模型"项目用 Redis"，今天问它，它还是两眼一抹黑。知识没有沉淀，没有版本，没有生命周期，更没有多人协作的隔离边界。

受 Andrej Karpathy 提出的 **LLM Wiki** 概念启发，我们在 Rust 里从零构建了一个"持久知识内核"——`llm-wiki`。它的核心理念很简单：

> **把知识当成数据库里的一等公民，而不是一次性的 prompt 上下文。**

上个版本我们已经实现了基础的 `ingest / query / lint / supersede / crystallize`，以及基于 SQLite 的 outbox 事件流。这篇文章聊的是我们这一轮交付的**四项新能力**，每一项都直指之前的工程短板。

---

## 一、多 Agent 隔离：`Scope` 终于有了牙齿

### 问题

原来的代码里 `Scope::Private` 和 `Scope::Shared` 只是个枚举——写进去是你的，但查出来是所有人的。这在单 Agent 场景没问题，但你一旦想让两个 AI 助手共用同一个 `wiki.db`，它们的私有记忆就会互相泄露。

### 解法

我们在 `wiki-core` 里新增了 `scope_policy.rs`，定义了一条清晰的可见性规则：

```rust
pub fn document_visible_to_viewer(doc_scope: &Scope, viewer: &Scope) -> bool {
    match (doc_scope, viewer) {
        (Scope::Private { agent_id: d }, Scope::Private { agent_id: v }) => d == v,
        (Scope::Shared { team_id: d }, Scope::Shared { team_id: v }) => d == v,
        _ => false, // private 看不到 shared，shared 也看不到 private
    }
}
```

然后把这个过滤器插到检索管道的最前面——BM25 路、向量路、图遍历路，全部在 `InMemorySearchPorts::collect_doc_scores` 里统一卡口。

**效果**：

```bash
# Alice 和 Bob 共用 wiki.db，各自写各自的数据
# Bob 用 Alice 的视角查询——结果为空
cargo run -p wiki-cli -- --db wiki.db \
  --viewer-scope private:alice query "Redis"
```

我们写了专项单测 `query_respects_private_scope_isolation`，并在 e2e 脚本里增加了"intruder 视角应得空结果"的负例验证。

---

## 二、真正的向量检索：SQLite + Cosine，零外部依赖

### 问题

之前的 `vector_ranked_ids` 是个彻头彻尾的 stub——它和 BM25 做的事情完全一样（token 重叠），只是排序顺序稍微不同，纯粹用来测 RRF 管道的 plumbing 是否通畅。

### 解法

我们在 `wiki-storage` 里新增了 `wiki_embedding` 表：

```sql
CREATE TABLE wiki_embedding (
    doc_id TEXT PRIMARY KEY,
    dim    INTEGER NOT NULL,
    vec    BLOB NOT NULL,      -- f32 小端数组
    updated_at TEXT NOT NULL
);
```

Cosine 相似度在 Rust 内存里用 SIMD 友好的方式计算（数据量中等时完全可接受），不依赖任何外部向量数据库。

写入侧：`ingest` / `file-claim` / `supersede` 加上 `--vectors --llm-config` 就会自动调用 OpenAI-compatible `/v1/embeddings` 接口，把 embedding 写进去：

```bash
cargo run -p wiki-cli -- \
  --db wiki.db --vectors --llm-config llm-config.toml \
  ingest "file:///x.md" "Postgres 支持 JSONB 索引" --scope private:cli
```

查询侧：把 query 文本 embed 之后，走 `search_embeddings_cosine` 拿到按余弦排好序的 doc id 列表，作为 RRF 第二路的 `vector_rank_override` 注入，完全绕开 stub：

```bash
cargo run -p wiki-cli -- \
  --db wiki.db --vectors --llm-config llm-config.toml \
  query "数据库索引优化"
```

这一路和 BM25 路、图路最终在 RRF（倒数排名融合）里汇合，给出综合排序。

**验收**：单测 `embedding_cosine_ranking` 写入三条不同方向 embedding，验证 cosine 最近邻排序正确，且重载 DB 后向量仍在。

---

## 三、LLM 自动 ingest：一行命令，结构化知识入库

### 问题

之前 ingest 完全靠人工：你写一段话、手动 `file-claim`、手动加实体……工程效率极低。

### 解法

新增 `ingest-llm` 子命令。核心流程：

```
原始文本 → 调用 LLM（chat completion）→ 严格 JSON → 校验 → 批量写入引擎
```

我们在 `wiki-core` 里定义了 DTO `LlmIngestPlanV1`：

```rust
pub struct LlmIngestPlanV1 {
    pub version: u32,
    pub summary_title: String,
    pub summary_markdown: String,
    pub claims: Vec<LlmClaimDraft>,
}

pub struct LlmClaimDraft {
    pub text: String,
    pub tier: String, // working | episodic | semantic | procedural
}
```

LLM 只需要输出这样一段 JSON：

```json
{
  "version": 1,
  "summary_title": "Redis 缓存策略",
  "summary_markdown": "## 摘要\n项目使用 Redis 作为 L2 缓存。",
  "claims": [
    { "text": "Redis 用于 session 缓存，TTL=30min", "tier": "semantic" },
    { "text": "缓存 key 格式：user:{id}:session", "tier": "procedural" }
  ]
}
```

解析成功后，引擎自动调用 `ingest_raw` + 批量 `file_claim` + 可选 summary wiki 页写入 + `--vectors` 时顺带 embed。

失败保险：**LLM 输出解析失败时不写任何 claim**，只可选写一个错误说明页，保证数据库干净。

```bash
# --dry-run 只打印模型 JSON，不落库，用于调试 prompt
cargo run -p wiki-cli -- --db wiki.db --llm-config llm-config.toml \
  ingest-llm "file:///x.md" "正文内容……" --scope private:cli --dry-run
```

**验收**：`tests/fixtures/ingest_llm_ok.json` 是一份录制的 fixture，`parses_fixture_file` 单测用 `include_str!` 做完全离线的解析与引擎写入验证，无需联网。

---

## 四、MemPalace 深度联动：图路 RRF 打通，写路事件补全

### 问题

之前 `wiki-mempalace-bridge` 只能"消费 outbox 事件往 MemPalace 写"，没有反向的"从 MemPalace 图遍历读回候选 doc id"；而且 `SourceIngested` 事件被 outbox consumer 直接忽略了。

### 解法

**读路径**——`merge_graph_rankings`：

在 `wiki-kernel` 里新增了一个干净的工具函数，把内核图路和外部（MemPalace traverse）候选按**轮次交织**合并：

```
primary:   [e1, e2, e3]
secondary: [e1, x1]
merged:    [e1, e2, x1, e3]   ← 去重，内核优先，外部补充
```

CLI 侧增加了 `--graph-extras-file`：

```bash
# 把 MemPalace 图遍历结果写成文件，每行一个 doc id
echo "entity:abc123" > /tmp/mp_extras.txt
echo "claim:def456" >> /tmp/mp_extras.txt

cargo run -p wiki-cli -- --db wiki.db \
  --graph-extras-file /tmp/mp_extras.txt \
  query "Redis 架构"
```

这样就把 MemPalace 的图知识无缝并入了 RRF 第三路，没有任何强依赖。

**写路径**——`SourceIngested` 事件：

`MempalaceWikiSink` 增加了 `on_source_ingested` 方法（默认空实现，不破坏现有代码）；`consume_outbox_ndjson` 现在会正确分发 `WikiEvent::SourceIngested`。e2e 输出里能看到：

```
mempalace source_ingested bff1974b-...
mempalace claim_upserted 50818609-...
mempalace claim_superseded a60a0f84-... -> 662cb92d-...
consumed=4  ← 从 3 变成了 4
```

**trait 设计**——`MempalaceGraphRanker`：

```rust
pub trait MempalaceGraphRanker: Send + Sync {
    fn graph_rank_extras(&self, query: &str, limit: usize) -> Vec<String>;
}
```

默认实现 `NoopMempalaceGraphRanker` 返回空列表，CI 永远绿。若你本机有 `rust-mempalace`，实现这个 trait 即可接入真实图遍历，完全不改内核代码。

---

## 架构全景：四路汇合 RRF

完成这四项工作后，查询管道的全貌如下：

```
用户 query
    │
    ├─ BM25（token 重叠 stub，后续换 tantivy）
    │
    ├─ Vector（embed query → cosine search → viewer_scope 过滤）
    │       ↑
    │   wiki_embedding 表（SQLite, BLOB f32）
    │
    └─ Graph（InMemorySearchPorts::graph_ranked_ids
    │         + merge_graph_rankings
    │         + --graph-extras-file / MempalaceGraphRanker）
    │
    └──────── RRF Fusion ──────── 保留强度加权 ──── Top-K 结果
                                      ↑
                               claim 的 tier half-life 衰减
```

写入侧：

```
ingest / ingest-llm / file-claim / supersede
    │
    ├─ SQLite 状态（claims / pages / entities / sources）
    ├─ wiki_embedding（可选 --vectors）
    └─ outbox events → MempalaceWikiSink
           ├─ on_claim_upserted
           ├─ on_claim_superseded
           └─ on_source_ingested  ← 新增
```

---

## 工程细节值得一提

**1. scope 过滤的层次**

不是在查询结果上"事后过滤"，而是在 `collect_doc_scores` 入口就卡掉不可见的文档，避免无谓的打分计算。

**2. raw string 陷阱**

Rust 的 `r#"..."#` 原始字符串在 JSON 里出现 `"#` 时会提前截断。我们在写 ingest_plan 单测时踩了这个坑，最终用 `r###"..."###` 绕过。对于 JSON 内联测试，还是推荐 `serde_json::json!` 宏或 fixture 文件。

**3. e2e 负例验证**

好的 e2e 不只测"正路通"，还要测"隔离墙真的挡住了"。我们在 `scripts/e2e.sh` 里加了一步：

```bash
ranked_wrong="$(... --viewer-scope private:intruder query "Redis" | grep '^[0-9]' || true)"
if [[ -n "$ranked_wrong" ]]; then
  echo "scope 隔离失败" >&2; exit 1
fi
```

**4. outbox 事件驱动的扩展性**

整个 MemPalace 集成的核心思想是：**内核不依赖外部系统，外部系统通过消费 outbox 事件感知内核变化**。这让 `llm-wiki` 可以独立运行、独立测试，而不被任何外部依赖绑架。

---

## 下一步

这四项工作完成后，整个系统的"骨架"已经稳固。接下来计划的方向：

- **BM25 升级**：把 token stub 换成 `tantivy` 内存索引，真正利用 TF-IDF 统计。
- **实体与关系提取**：在 `ingest-llm` 的 JSON 计划里增加 `entities` / `edges` 字段，让知识图谱真正生长。
- **MemPalace native 集成**：在本机开启 `mempalace-native` feature，让 `graph_rank_extras` 走真实的图遍历。
- **Web UI**：把 wiki projection 的 Markdown 接一个轻量浏览器，不再只靠 Obsidian。

---

## 写在最后

这个项目最让我们满意的地方，不是某个单独的功能，而是**整个系统的可组合性**：

- 向量路和 BM25 路可以独立开关，互不干扰。
- Scope 过滤是一个纯函数，在任何地方调用结果都一致。
- MemPalace 集成完全通过 trait 抽象，零侵入内核。
- 所有写路径都走 outbox，任何外部系统都可以"订阅"知识变化。

这就是 Rust 在构建长期演化的工程系统时给我们的底气：**类型系统和所有权模型，迫使我们在代码层面就把边界想清楚。**

代码在 GitHub，欢迎 Star、Issue 和 PR。

---

*作者：llm-wiki team | 技术栈：Rust · SQLite · RRF · OpenAI-compatible embeddings · outbox pattern*
