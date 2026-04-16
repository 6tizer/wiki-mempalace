# 用 Rust 给 AI Agent 造一座「记忆宫殿」

> 如果你的 AI 助手每次对话都从零开始，那它只是一个没有记忆的工具。我们能不能让它真正「记住」你？

---

## 一、起点：AI 记忆是个伪命题吗？

用 ChatGPT、Cursor 或者任何 LLM 工具的时候，你一定踩过这个坑：

- **今天**跟它讨论了架构决策，**明天**它已经忘得一干二净。
- **上周**你解释过为什么选 Rust 而不是 Go，这周它又在推荐你用 Go。
- 你把同一段背景信息复制粘贴了几十遍，仍然感觉像在跟一个失忆患者打交道。

这不是模型不够聪明，而是结构性问题：**大模型没有持久记忆层**。

市面上的解法大多是「把历史塞进 Context」——但 context 有限，而且质量参差不齐，真正的决策、原则、项目背景经常淹没在噪音里。

[MemPalace](https://github.com/MemPalace/mempalace) 提出了一个更清醒的思路：与其让 AI 自己总结压缩，不如**原文存储，让检索来决定什么被唤醒**。

我们沿着这个思路，用 Rust 从头实现了一套更强的本地记忆系统——`rust-mempalace`。

---

## 二、核心理念：记忆宫殿不是数据库

「记忆宫殿」是古希腊演说家用来记忆长篇演讲的空间记忆法：把要记的内容放在熟悉的空间里的不同位置，想起来的时候「漫步」其中。

我们把这个比喻直接映射到数据模型上：

```
Wing（翼楼）→ Hall（大厅）→ Room（房间）→ Drawer（抽屉）
```

每一条记忆是一个 **Drawer**，里面装的是**原文**，不做任何 AI 摘要。分类规则完全透明、可编辑，由 `classifier_rules.json` 驱动，不存在黑箱。

不同翼楼之间可以打通 **Tunnel（隧道）**，让跨域的知识显式关联——比如把「架构决策」和「具体项目」连起来。

这套空间结构让记忆**可导航**，而不只是可检索。

---

## 三、技术选型：为什么是 Rust？

原版 MemPalace 是 Python 实现。我们选择用 Rust 重写，有几个实在的理由：

| 维度 | Python 版 | Rust 版（本项目）|
|------|-----------|----------------|
| 部署 | 需要 Python 运行时 + 依赖 | 单一静态二进制，无额外依赖 |
| 检索引擎 | 外部向量数据库 | 内嵌 SQLite FTS5，零配置 |
| 分类透明度 | 模型推断 | 规则文件，完全可审计 |
| Agent 接入 | 需要额外适配 | 原生 MCP stdio JSON-RPC |
| 跨平台 | 环境差异多 | 一次编译，到处运行 |

**最关键的一点**：我们想要一个可以扔进任何 CI、任何容器、任何开发者机器上就能跑的工具。Rust 的单二进制特性让这件事变得极其简单。

---

## 四、混合检索：不只是关键词匹配

记忆系统最核心的能力是**召回**——你需要的时候，它能把对的东西找出来。

我们实现了三层召回管线：

```
FTS5 全文检索
    ↓ (miss fallback)
LIKE 模糊匹配
    ↓
词法得分 × weight + trigram 语义得分 × weight → Rerank
```

权重可以在 `~/.mempalace-rs/config.json` 里实时调整：

```json
{
  "retrieval": {
    "lexical_weight": 1.0,
    "vector_weight": 1.3
  }
}
```

搜索结果还支持 `explain` 模式，每一条结果都能告诉你**为什么它被召回、得分是怎么来的**——这对调试检索质量非常重要。

---

## 五、知识图谱层：让记忆有时态

光存文本还不够。现实世界的知识是**随时间变化**的：

- 「我们用 Postgres」——这是什么时候的决策？还有效吗？
- 「张三负责认证模块」——他后来换岗了吗？

我们在 drawers 之上加了一层 `kg_facts`（知识图谱事实），支持时态 triple：

```
subject --predicate--> object  [valid_from .. valid_to]
```

配套的命令包括：

```bash
# 添加一条时态事实
mempalace-rs kg-add --subject "auth-service" --predicate "uses_db" --object "Postgres"

# 查某个主体的时间线
mempalace-rs kg-timeline --subject "auth-service"

# 检测矛盾：同一主体同一谓词有多个并发 object
mempalace-rs kg-conflicts

# 让一条事实失效（比如换库了）
mempalace-rs kg-invalidate --subject "auth-service" --predicate "uses_db" --object "Postgres"
```

每一条 fact 还可以通过 `source_drawer_id` 回链到原始文本——**知识的来源永远可以追溯**。

---

## 六、MCP 接入：让 AI Agent 直接调用记忆

这是整个系统最有意思的部分。

[MCP（Model Context Protocol）](https://modelcontextprotocol.io/) 是 Anthropic 提出的 AI 工具调用标准，Cursor、Claude Desktop 等工具都已经原生支持。

我们给 `rust-mempalace` 实现了完整的 MCP stdio JSON-RPC server，提供以下工具：

```
mempalace_status      → 查看记忆库状态
mempalace_search      → 混合检索记忆
mempalace_wake_up     → 生成 L0+L1 唤醒上下文
mempalace_taxonomy    → 查看 wing/hall/room 分类树
mempalace_traverse    → 遍历隧道图
mempalace_kg_query    → 查询知识图谱
mempalace_kg_timeline → 查看时态变化
mempalace_kg_stats    → 统计概览
```

在 Cursor 里，只需要在 `.cursor/mcp.json` 里注册一行：

```json
{
  "mcpServers": {
    "mempalace": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "/path/to/rust-mempalace/Cargo.toml", "--", "mcp", "--quiet"],
      "env": {}
    }
  }
}
```

之后 Cursor 里的 AI 就可以直接调用你的本地记忆库。Agent 对话的时候，它能主动查「我们之前关于认证的决策是什么」，而不是等你手动粘贴。

---

## 七、端到端测试：从 CLI 到 Agent 调用链

「能跑」和「真的可靠」之间的距离，靠测试来填。

我们写了 7 个端到端集成测试，每个测试都在独立临时目录下启动真实二进制：

```
e2e_cli_text_mode           → CLI 文本输出完整链路
e2e_cli_json_mode           → CLI JSON 输出机器可读
e2e_mcp_suite               → MCP tools/list + tools/call
e2e_agent_uses_mcp_toolchain → Agent 连续调用 search→wake_up→kg_query
e2e_kg_conflict_timeline    → KG 冲突检测与时间线
e2e_bench_fixed_vs_random   → 基准测试两种模式
```

其中 `e2e_agent_uses_mcp_toolchain` 是我们最在意的一个：它模拟了真实 Agent 的调用序列——`tools/list → mempalace_search → mempalace_wake_up → mempalace_kg_query`——完整走一遍，不 mock，不造假。

---

## 八、CI 分层：快速反馈 + 全量门禁

测试本身是成本，我们把它分成两层：

**CI Quick**（每次 PR / push main 触发）
- `cargo fmt --check` + `cargo test --bin`
- 目标：30 秒内给出反馈，不让 PR 循环变慢

**CI E2E**（push main + 定时每日）
- `cargo test`（含全部 7 个端到端用例）
- 目标：保证 Agent + MCP + KG + Bench 全链路始终可用

两层 CI 分开触发，快的通道保住开发体验，慢的通道守住质量底线。

---

## 九、Benchmark：检索质量说数据，不说感觉

系统好不好用，不能只靠主观感受。我们内置了 benchmark 命令：

```bash
# 随机采样 30 条，测 top-5 召回
mempalace-rs bench --samples 30 --top-k 5 --mode random

# 固定 case 集回归测试
mempalace-rs bench --samples 20 --top-k 5 --mode fixed --report bench.json
```

输出包含：recall@k、平均延迟（ms）、吞吐量（queries/s）。

配合 `--report` 参数可以输出 JSON 格式报告，方便接入 CI 做回归对比——**调了权重之后，检索质量有没有退步，一目了然**。

---

## 十、「唤醒」：每次对话开始前的热身

`wake-up` 是整个系统里最有 MemPalace 气质的功能。

```bash
mempalace-rs wake-up
```

它会从记忆库里提取：
- **L0**：你设定的 identity（你是谁、你的工作原则）
- **L1**：最近活跃的上下文摘要

把这段文本粘贴到每次对话开头（或者让 Agent 自动调用 `mempalace_wake_up`），相当于给 AI 做了一次「记忆唤醒」，让它从当下状态出发，而不是每次冷启动。

---

## 总结：一套工具，三层价值

| 层次 | 解决的问题 |
|------|-----------|
| **存储层**（drawers + taxonomy）| 原文入库，结构清晰，去重可靠 |
| **检索层**（FTS5 + rerank + bench）| 混合召回，可解释，可量化 |
| **知识层**（kg_facts + timeline）| 时态关联，冲突检测，来源追溯 |

加上 MCP 接入，整个系统既是一个 CLI 工具，也是 AI Agent 可以直接调用的本地记忆服务。

你的项目决策、架构选型、对话历史——不再随着 context 窗口消失，而是沉淀在本地，随时可召回，随时可追溯。

---

## 快速上手

```bash
# 克隆并构建
git clone https://github.com/your-org/rust-mempalace
cd rust-mempalace
cargo build --release

# 初始化记忆库
cargo run -- init --identity "你是我的编码助手，请保留所有架构决策。"

# 把项目代码 / 对话记录挖进去
cargo run -- mine ~/Projects/my-app
cargo run -- mine ~/chat-exports --mode convos

# 搜索
cargo run -- search "为什么选 Postgres"

# 唤醒上下文
cargo run -- wake-up

# 启动 MCP server（供 Cursor / Claude Desktop 调用）
cargo run -- mcp --quiet
```

项目地址：[github.com/your-org/rust-mempalace](https://github.com/your-org/rust-mempalace)

---

*如果你也在探索 AI 记忆、知识图谱、或者 MCP 工具链，欢迎交流。*
