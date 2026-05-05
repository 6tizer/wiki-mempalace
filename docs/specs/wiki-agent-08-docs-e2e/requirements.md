# wiki-agent 08 Docs + E2E Requirements

## Scope

- Refresh user-facing docs for current `wiki-agent` behavior.
- Make root README describe native direct-call first, MCP compatibility, TUI, memory, and sub-agent workers.
- Keep MCP reference clear that `wiki-cli mcp` remains compatible and shares `wiki-tools::ToolRegistry`.
- Update Notion workflow capability matrix for chat, TUI, sub-agent orchestration, memory, and skills.
- Extend `scripts/e2e.sh` with wiki-agent native / mcp-child / memory / palace smoke.

## Acceptance

- Docs no longer imply Governance/Fixer/Synthesis are only manually operated from CLI.
- README and architecture describe `wiki-agent` as the interactive entrypoint and `wiki-cli mcp` as compatibility server.
- E2E covers `doctor native`, `doctor mcp-child`, fake chat turn, memory extraction, and palace search.
- Existing MCP API schema and tool count remain unchanged.
