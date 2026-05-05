# wiki-agent 02 Native ToolRegistry Requirements

## Scope

PR2 adds the shared native tool layer used by both `wiki-cli mcp` and the new
`wiki-agent` entrypoint.

## Functional Requirements

- Provide a `wiki-tools` crate with a single registry for the current 22 MCP tools.
- Keep `wiki-cli mcp` protocol, schemas, typed errors, scope checks, and mempalace bank derivation compatible.
- Add a minimal `wiki-agent doctor` command that can discover native tools.
- Add an `mcp-child` fallback discovery path for compatibility with an external `wiki-cli mcp` process.
- Do not duplicate tool business logic between `wiki-cli` and `wiki-agent`.

## Non-Goals

- No chat loop, TUI, manager-worker routing, memory extraction, or web RAG in this PR.
- No new production data writes beyond existing tool behavior.

## Acceptance

- `wiki-cli mcp tools/list` still exposes 22 tools.
- `wiki-agent doctor --tool-backend native` reports 22 native tools.
- `wiki-agent doctor --tool-backend mcp-child` can discover tools through a child `wiki-cli mcp` when the binary is available.
- Existing MCP tests pass from the shared crate.
