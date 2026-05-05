# PRD: wiki-agent

## Goal

Build a local-first `wiki-agent` that gives the wiki system a first-class chat
and TUI interface while preserving the existing `wiki-cli mcp` server.

The agent should combine:

- Rig-style Rust-native agent orchestration.
- Claude Code-like conversational tool use.
- Hermes-style persistent memory, self-improvement loop, and modern terminal UI.

## Product Outcome

- `wiki-agent chat` is the default command-line conversation surface.
- `wiki-agent tui` and `wiki-agent chat --tui` provide an optional ratatui UI.
- The manager agent can delegate work to sub agents for lint, governance, fixer,
  synthesis, search, and memory curation.
- Tool execution defaults to native Rust direct calls through a shared
  ToolRegistry. `rmcp` child-process MCP access remains only as fallback.
- `wiki.db` remains the source of truth; Vault and `palace.db` remain projections.
- Existing `wiki-cli mcp` behavior and schemas remain compatible.

## Scope

This batch is split into eight PRs:

1. Shared AI core. Merged PR #113.
2. Native ToolRegistry plus MCP adapter. In local implementation.
3. CLI chat.
4. Local + web RAG answering.
5. Manager-worker sub agents.
6. Persistent memory + skill loop.
7. TUI.
8. Docs and E2E.

## Non-Goals

- Do not replace or remove `wiki-cli mcp`.
- Do not make MCP child process the default tool path.
- Do not make Vault or `palace.db` primary write sources.
- Do not store raw chat transcripts in `wiki.db`; only verified memories/skills
  may be promoted there.

## Acceptance

- Existing `wiki-cli` commands and MCP server remain compatible.
- `wiki-agent doctor` can validate native tools and fallback MCP discovery.
- `wiki-agent chat` supports profiles, tool calls, local/wiki evidence, web
  evidence, and session history.
- Sub agents execute through native Rust tool/core calls when available.
- TUI shares the same runtime and policies as CLI chat.
