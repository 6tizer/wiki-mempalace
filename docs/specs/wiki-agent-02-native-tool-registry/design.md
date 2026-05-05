# wiki-agent 02 Native ToolRegistry Design

## Design

The tool implementation moves into `wiki-tools`.

- `ToolRegistry::list()` returns typed tool definitions parsed from the existing MCP schema.
- `ToolRegistry::call()` executes the same handler used by the MCP adapter.
- `ToolContext` carries the loaded `LlmWikiEngine`, `SqliteRepository`, viewer scope, wiki projection path, palace path, vector flag, and LLM config path.
- `wiki-cli mcp` remains the compatibility stdio JSON-RPC entrypoint and delegates to `wiki_tools::run_mcp`.
- `wiki-agent` uses native discovery by default and has an `mcp-child` fallback that sends `tools/list` to a child `wiki-cli mcp --once`.

## Compatibility

MCP behavior remains source-compatible:

- Same 22 tool names.
- Same JSON schema shape.
- Same typed error object shape.
- Same scope and bank capability rules.
- Same storage-backed `wiki_query` behavior.

The fallback path is compatibility-only. It is not the default runtime for
future `wiki-agent` tool calls.
