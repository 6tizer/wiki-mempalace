# PRD: Audit Report Hardening

## Goal

把 Notion「wiki-mempalace 全方位代码审计报告」中能安全落地的安全、可靠性、DX 问题收敛为一轮小补丁；架构迁移项继续留在已有专门 PRD/spec。

## Scope

- MCP stdio 输入边界：限制单行 JSON-RPC 请求大小，避免无界 `read_line` 分配。
- LLM ingest 后置校验：LLM JSON 解析后、写入引擎前校验 version、字段长度、数组数量。
- ingest 脱敏：扩展 token / email / phone / credit-card-like 规则，并提取占位符常量。
- outbox flush：独立 flush 路径改为批量事务，避免逐条 autocommit 下的部分写入。
- `rust-mempalace` FTS query：把用户 token quote 后传给 FTS5 `MATCH`，降低语法/操作符风险。
- automation health：把 `PRAGMA integrity_check` 纳入日常 health 输出。
- multi-process write guardrails：补 README / AGENTS writer safety 警告。
- MCP API reference：新增 MCP 工具参数、scope、副作用和错误形状参考。

## Out of Scope

- `wiki_state` 单行 JSON blob 行级迁移：已有 C16 后续方向，需独立存储迁移设计。
- `wiki_embedding` ANN：已有 [embedding-ann-index](../specs/embedding-ann-index/requirements.md)。
- `wiki-cli/src/main.rs` 大拆分：纯重构，需独立 PRD，避免和安全修复混做。
- MCP typed error mapping、multi-process writer lock/lease、contradiction scan scaling、reliability/fuzz/perf tests、dependency audit、time library unification：均已登记到 roadmap，按独立批次处理。

## Success Criteria

- MCP 超大单行请求在解析前失败。
- `ingest-llm` / `wiki_ingest_llm` 对超大 LLM plan 拒绝写入。
- 新增敏感样本被脱敏测试覆盖。
- outbox 独立 flush 使用 batch transaction API。
- `automation health` 输出 DB integrity 状态。
- README / AGENTS 写清单 writer 约束。
- MCP API reference 进入 docs index。
- `cargo test --workspace` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过。
