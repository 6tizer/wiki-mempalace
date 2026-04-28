# Design: MCP Typed Errors

## Error Type

Add a private `McpToolError` enum in `wiki-cli/src/mcp.rs`. Each variant owns a human-readable message and maps to:

- `invalid_params` / `tool_not_found`: JSON-RPC `-32602`
- `method_not_found`: JSON-RPC `-32601`
- `engine_error`, `storage_error`, `llm_error`, `mempalace_error`: JSON-RPC `-32000`

## Response Shape

`handle_request` serializes errors as:

```json
{"code":-32602,"message":"missing query","data":{"kind":"invalid_params"}}
```

Parse errors keep JSON-RPC `-32700` and add `data.kind=parse_error`.

## Boundaries

Core crates keep their existing error types. MCP maps errors at the edge so later MCP Vault Sync and API clients can rely on stable JSON-RPC categories without broad engine/storage refactors.
