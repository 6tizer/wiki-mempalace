# Requirements: MCP Typed Errors

## Functional Requirements

- MCP MUST return JSON-RPC `-32601` for unknown JSON-RPC methods.
- MCP MUST return JSON-RPC `-32602` for missing or invalid tool arguments.
- MCP MUST include `error.data.kind` for parse, method, tool, parameter, engine, storage, LLM, and mempalace errors.
- MCP MUST preserve successful `result` payloads for existing tools.
- MCP tool error messages SHOULD remain human-readable and close to current text.

## Non-Functional Requirements

- No new runtime dependencies.
- No network calls in tests.
- Error mapping must be implemented inside the MCP layer, not by changing core engine/storage error types.

## Acceptance

- Tests cover invalid params and unknown method mapping.
- `docs/mcp-api-reference.md` documents the typed error shape.
