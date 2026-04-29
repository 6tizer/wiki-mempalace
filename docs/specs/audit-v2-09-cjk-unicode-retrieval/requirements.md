# Requirements: Audit v2 PR 09 CJK / Unicode Retrieval

## Scope

按 roadmap PR 09 修复 `rust-mempalace` 检索 token 边界：

- FTS query builder 保留 Unicode alphanumeric token。
- 纯标点 / 空 token query 不再替换成固定 `"memory"`。
- CJK query 需要 trigram / LIKE fallback，避免 FTS tokenizer 无召回时直接空结果。
- 英文 token quoting 行为不得退化，`OR` 等操作符必须继续作为普通 quoted token。
- 不修改生产 palace 数据；只通过 temp dir / unit / e2e 测试验证。

## Acceptance

- 中文 query e2e 可召回中文 fixture。
- `build_fts_query("hello OR world")` 仍输出 quoted tokens。
- `build_fts_query("!!!")` 返回 no-query，而不是 `"memory"`。
- CJK fallback 生成 exact token + bigram/trigram patterns。
- sparse embedding / rerank 使用 Unicode-aware terms。
- 既有 bank scope / limit clamp 行为不改。

## Non-goals

- 不引入中文分词第三方库。
- 不改 `wiki-cli` production SearchPorts 默认路；PR 08 已完成。
- 不改 MCP schema / capability。
- 不做 live `/Users/mac-mini/Documents/wiki` 写入验证。
