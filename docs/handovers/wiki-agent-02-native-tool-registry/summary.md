# Handoff: wiki-agent 02 Native ToolRegistry

## Scope

Branch: `codex/wiki-agent-native-tool-registry`.

Implemented PR2 of the wiki-agent batch:

- Added `crates/wiki-tools` with `ToolRegistry`, `ToolDefinition`, `ToolContext`, and the shared implementation for all 22 MCP-compatible tools.
- Moved existing MCP compatibility tests into `wiki-tools`.
- Replaced `crates/wiki-cli/src/mcp.rs` with a thin compatibility wrapper around `wiki_tools::run_mcp`.
- Added initial `crates/wiki-agent` binary with `doctor --tool-backend native|mcp-child|auto`.
- Added mcp-child discovery by sending `tools/list` to a child `wiki-cli mcp --once`.

## Verification

- `target/debug/wiki-agent doctor --tool-backend native` reports 22 tools.
- `target/debug/wiki-agent --db <temp>/wiki.db doctor --tool-backend mcp-child --wiki-cli target/debug/wiki-cli` reports 22 tools.
- `cargo fmt --all -- --check`
- `cargo test -p wiki-tools`
- `cargo test -p wiki-agent`
- `cargo test -p wiki-cli`
- `cargo test -p wiki-ai`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
- `git diff --check`

## Notes

- PR2 does not implement chat, TUI, sub-agent routing, or memory extraction.
- Native direct-call is the default path for future `wiki-agent`; mcp-child is compatibility fallback only.
- `wiki-cli mcp` remains externally compatible because the JSON-RPC server still exposes the same schemas and error shape.
