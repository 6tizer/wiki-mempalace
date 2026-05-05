# wiki-agent 08 Docs + E2E Design

## Documentation Model

`wiki-agent` is documented as a new interactive surface, not a replacement for
`wiki-cli mcp`.

- `wiki-agent`: native runtime for chat, TUI, manager-worker, web RAG, memory.
- `wiki-tools::ToolRegistry`: one implementation of the 22 tools.
- `wiki-cli mcp`: JSON-RPC adapter over ToolRegistry.
- `mcp-child`: fallback only.

## E2E Model

The existing `scripts/e2e.sh` keeps DB-first wiki behavior as the base smoke and
adds agent checks at the end:

1. `wiki-agent doctor --tool-backend native` finds 22 tools.
2. `wiki-agent doctor --tool-backend mcp-child` finds the same 22 tools through
   `wiki-cli mcp`.
3. `wiki-agent chat` runs with fake LLM and explicit memory marker.
4. `wiki-agent memory status --json` sees the extracted memory page.
5. `consume-to-mempalace` projects the memory page.
6. `rust-mempalace search --bank cli` finds the memory marker.

The smoke stays offline: no live LLM key or web provider is required.
