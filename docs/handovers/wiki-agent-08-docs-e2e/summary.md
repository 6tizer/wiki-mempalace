# Handoff: wiki-agent 08 Docs + E2E

## Scope

Branch: `codex/wiki-agent-docs-e2e`.

Implemented:

- Root README updated for `wiki-agent` chat/TUI/native ToolRegistry and MCP compatibility.
- Architecture updated for `wiki-agent`, `wiki-ai`, `wiki-tools`, native workers, and memory flow.
- MCP reference clarifies ToolRegistry is shared and `mcp-child` is fallback only.
- Notion workflow matrix now includes chat, TUI, manager-worker, and memory/skills.
- Dev workflow includes `wiki-agent` operating rules.
- E2E script now smokes native doctor, mcp-child doctor, fake chat, memory extraction, and palace search.

## Verification

- `bash -n scripts/e2e.sh`
- `./scripts/e2e.sh` (LLM smoke skipped as optional because provider credits were unavailable)
- `cargo fmt --all -- --check`
- `cargo test -p wiki-agent`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
- `git diff --check`
