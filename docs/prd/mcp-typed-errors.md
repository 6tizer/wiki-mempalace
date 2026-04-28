# PRD: MCP Typed Errors

## Status

Implemented in PR #TBD.

## Goal

把 MCP server 的字符串错误收敛成稳定 JSON-RPC error mapping，让调用方可以按错误分类处理参数错误、未知方法/工具、内部执行错误。

## Scope

- 为 `wiki-cli/src/mcp.rs` 增加 `McpToolError` 分类。
- `handle_request` 输出稳定 `error.code` 和 `error.data.kind`。
- 缺失参数、非法 UUID、非法 tier/tag 等输入错误返回 `invalid_params`。
- 未知 JSON-RPC method 返回 `method_not_found`。
- 未知 MCP tool 返回 `tool_not_found`。
- engine/storage/LLM/mempalace 失败保留原 message，并在 `data.kind` 中标出来源。

## Out of Scope

- 不改变 MCP tool 成功响应 shape。
- 不改变 Vault projection 行为；`MCP Vault Sync` 单独实现。
- 不重构 `wiki-cli/src/main.rs`。

## Success Criteria

- MCP 错误响应包含 JSON-RPC `code`、`message`、`data.kind`。
- 现有 MCP 工具成功路径保持兼容。
- Focused MCP tests、workspace tests、fmt、clippy 通过。
