# rust-mempalace

面向本地优先工作流的 **MemPalace 风格记忆库**：用单一 Rust 可执行文件管理「原文存储、结构化归档、检索与唤醒」，可选接入 [Model Context Protocol (MCP)](https://modelcontextprotocol.io/)，供编辑器与 Agent 通过 stdio JSON-RPC 调用。

设计参考上游思路 [MemPalace/mempalace](https://github.com/MemPalace/mempalace)，本仓库为独立实现，栈与交付形态不同（无 Python 运行时依赖，数据默认落在本地 SQLite）。

---

## 简介

**rust-mempalace** 解决的是：在长期使用 AI 辅助编程或文档协作时，**决策、对话与代码上下文**需要可检索、可导航、可追溯的落盘形态，而不是仅依赖模型上下文窗口。

- **写入**：以 verbatim 为主，不在入库阶段做模型摘要。
- **组织**：`wing / hall / room` 分层；跨域关系通过显式 `tunnel` 与遍历规则表达。
- **读取**：基于 SQLite FTS5 的检索管线，并包含混合打分与 rerank；支持 `explain` 便于调参。
- **扩展**：可选时态知识图谱表 `kg_facts`（时间线、统计、冲突检测、来源 drawer 回链）；内置基准命令用于回归对比。

---

## 功能概览


| 领域  | 能力                                                                                    |
| --- | ------------------------------------------------------------------------------------- |
| 入库  | `mine`（项目目录 / 对话导出）、`split` 大文件、路径与关键词驱动的分类、内容哈希去重                                    |
| 检索  | FTS5、LIKE 回退、词法 + trigram 混合、可配置权重                                                    |
| 导航  | `taxonomy`、`traverse`、`link`（tunnel）                                                  |
| 唤醒  | `wake-up`（L0 identity + L1 上下文摘要）                                                     |
| 知识层 | `kg-add` / `kg-query` / `kg-timeline` / `kg-stats` / `kg-conflicts` / `kg-invalidate` |
| 集成  | MCP stdio 服务；`--output json` / `--quiet` 便于脚本与 CI                                     |
| 质量  | `e2e_core` 子进程级端到端测试；分层 GitHub Actions                                                |


---

## 环境要求

- **Rust**：建议使用当前 **stable** 工具链（本仓库 `Cargo.toml` 使用 `edition = "2024"`，需工具链支持该版本）。
- **系统**：无额外守护进程；依赖通过 `rusqlite` 的 `bundled` 特性内嵌 SQLite。

---

## 安装

将本仓库克隆到本地后，在项目根目录执行：

```bash
cargo build --release
```

可执行文件默认路径：`target/release/rust-mempalace`（与 Cargo 包名一致）。开发时可用 `cargo run -- <子命令>`。

---

## 快速开始

```bash
cargo run -- init --identity "You are my coding copilot. Preserve architecture decisions."
cargo run -- mine /path/to/repo
cargo run -- mine /path/to/chat-exports --mode convos
# 多租户 / 隔离：入库时指定 bank（默认 `default`）；检索可加 `--bank`
cargo run -- mine ./notes --bank team-alpha
cargo run -- search "why did we choose postgres" --bank team-alpha
cargo run -- wake-up
cargo run -- status
cargo run -- mcp --quiet
```

默认 palace 根目录：`~/.mempalace-rs`。多环境或测试可使用 `--palace <目录>` 指向独立数据目录。

完整子命令与参数说明：

```bash
cargo run -- --help
# 或
./target/release/rust-mempalace --help
```

---

## 数据目录与配置


| 路径（相对 palace 根）         | 说明                                       |
| ----------------------- | ---------------------------------------- |
| `config.json`           | 检索权重（含 RRF）、`mcp`、`llm`（可选 OpenAI 兼容端点）等 |
| `classifier_rules.json` | wing / hall 路由规则（可编辑、可审计）                |
| `identity.txt`          | L0 唤醒用身份描述                               |


`config.json` 示例：

```json
{
  "retrieval": {
    "lexical_weight": 1.0,
    "vector_weight": 1.3,
    "rrf_k": 60.0,
    "rrf_weight": 18.0
  },
  "mcp": {
    "quiet_default": true
  },
  "llm": {
    "enabled": false,
    "base_url": "https://api.openai.com/v1",
    "api_key_env": "MEMPALACE_LLM_API_KEY",
    "model": "gpt-4.1-mini",
    "timeout_secs": 120
  }
}
```

启用后可用 CLI `reflect` / `extract` 或 MCP `mempalace_reflect` / `mempalace_extract`。密钥优先读 `api_key_env` 指向的环境变量；也可配置 `api_key`（不推荐提交到版本库）。请求发往 `{base_url}/chat/completions`（OpenAI 兼容）。

LLM 启用判定（四项需同时满足）：

- `llm.enabled = true`
- `llm.base_url` 非空
- `llm.model` 非空
- 可解析到 API key（先读 `api_key_env` 指向的环境变量，读不到再尝试 `api_key`）

因此，`api_key` 留空并不总是等价于禁用：如果 `api_key_env` 对应环境变量有值，LLM 仍会启用。若你希望明确关闭，建议直接设置 `llm.enabled = false`。

---

## 数据模型（概念）

- **drawers**：原文片段及 `wing` / `hall` / `room`、`bank_id`（记忆库/租户隔离）、`source_path`、内容哈希等元数据。
- **drawers_fts**：FTS5 虚拟表，服务检索。
- **tunnels**：跨 wing 的显式链接。
- **kg_facts**：带 `valid_from` / `valid_to` 的 SPO 事实；可与 `source_drawer_id` 关联。
- **traverse**：除显式 tunnel 外，不同 wing 下同名 `room` 可作为隐式连通边参与遍历。

---

## MCP

以 stdio 承载 JSON-RPC；适合在 Cursor、Claude Desktop 等客户端中注册为本地命令。

- 启动：`rust-mempalace mcp [--once] [--quiet]`
- 单次探测示例：`echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | rust-mempalace mcp --once`

当前暴露的工具名：`mempalace_status`、`mempalace_search`（支持 `bank_id`）、`mempalace_wake_up`、`mempalace_taxonomy`、`mempalace_traverse`、`mempalace_kg_query`、`mempalace_kg_timeline`、`mempalace_kg_stats`、`mempalace_reflect`、`mempalace_extract`。与 CLI 子命令并非一一对应（例如部分 `kg-`* 仅 CLI 提供）。

集成异常时，优先确认子进程 **stdout 仅输出 JSON-RPC**（使用 `mcp --quiet` 并避免其它进程污染管道）。

---

## 开发与测试

**单元与二进制测试（快速门禁，与 CI Quick 对齐）：**

```bash
cargo fmt --all -- --check
cargo test --bin rust-mempalace
```

**端到端集成测试**（真实二进制、临时 `--palace`）：

```bash
cargo test --test e2e_core
```

`tests/e2e_core.rs` 当前包含 8 个用例，覆盖 CLI 文本/JSON、MCP suite、KG 冲突与时间线、bench 固定/随机模式、Agent 工具链调用及错误路径、`bank_id` 检索隔离等。

**CI**（见 `.github/workflows/`）：


| Workflow       | 触发                                     | 作用                                        |
| -------------- | -------------------------------------- | ----------------------------------------- |
| `ci-quick.yml` | `pull_request`、`push` 至 `main`         | `fmt` + `cargo test --bin rust-mempalace` |
| `ci-e2e.yml`   | `push` 至 `main`、`workflow_dispatch`、定时 | 全量 `cargo test`（含 e2e）                    |


---

## 仓库结构（摘要）

```
src/
  main.rs        # CLI 入口与命令分发
  cli.rs         # 参数与输出格式
  db.rs          # SQLite schema 与访问
  service.rs     # 业务编排（mine、search、wake-up、kg、bench 等）
  llm.rs         # 可选 OpenAI 兼容 Chat Completions 客户端
  mcp.rs         # MCP stdio JSON-RPC
  classifier.rs  # 分类与规则
docs/
  longmemeval.md # LongMemEval 与 CI 策略说明
tests/
  e2e_core.rs    # 端到端用例
```

---

## 与同类系统对比（如 Hindsight）

[Hindsight](https://github.com/vectorize-io/hindsight) 面向「Agent 随时间学习」：写入侧用 LLM 抽取实体/关系/时序并归一化，`recall` 并行语义、BM25、图与时序检索，再用 RRF 与 cross-encoder 等重排；提供 `reflect`、Docker/Web UI、多语言 SDK 与云端形态，并在 LongMemEval 等基准上强调召回质量。

**rust-mempalace** 侧重 **本地优先、原文可审计、单二进制**：默认不在入库时调用模型；检索以 SQLite FTS5 与本地稀疏向量混合为主，并在此基础上逐步增强（如 RRF 融合、`bank_id` 隔离、可选 OpenAI 兼容的 `extract` / `reflect`）。若你的首要约束是 **离线、可复现、数据不出境**，本仓库更合适；若首要约束是 **开箱即用的高阶记忆加工与企业集成**，可优先评估 Hindsight 等产品。

更细的差异与路线图见仓库内规划（不随 README 展开论文级细节）。LongMemEval 与 CI 策略见 [docs/longmemeval.md](docs/longmemeval.md)。

---

## 其它说明

- 分类为**确定性**规则匹配（路径与文本关键词），便于复现与审计。
- 检索质量与数据分布强相关；可调 `config.json` 后使用 `bench` 做对比。
- CLI 横幅使用彩色 ASCII；自动化或终端兼容性差时可设 `NO_COLOR=1`。

---

## 致谢

概念与产品方向受 [MemPalace](https://github.com/MemPalace/mempalace) 启发；本实现为独立代码库，API 与数据格式与上游不一定兼容。