# Handoff: MCP Typed Errors

## Branch

`codex/mcp-typed-errors`

## Completed

- Added private `McpToolError` classification in MCP server.
- Mapped invalid params/tool lookup to JSON-RPC `-32602`.
- Mapped unknown JSON-RPC methods to `-32601`.
- Added `error.data.kind` for parse, method, params, engine, storage, LLM, and mempalace failures.
- Updated MCP API reference with typed error shape.

## Deferred

- MCP write-triggered Vault projection remains `MCP Vault Sync`.
- Broader CLI command modularization remains separate.

## Verification

- `cargo test -p wiki-cli mcp::tests:: -- --nocapture` passed.
