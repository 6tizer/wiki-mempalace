# Tasks: MCP Vault Sync

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/mcp-vault-sync`
- [x] Implementation
- [x] Focused tests
- [x] Workspace tests
- [x] Clippy
- [x] Handoff
- [ ] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Gate MCP projection on `--sync-wiki` | Agent | Main | `crates/wiki-cli/src/main.rs` | Complete |
| Project after MCP writes | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Return typed projection errors | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Document runtime rule | Script | Main | `docs/mcp-api-reference.md`, docs indexes | Complete |

## Verification

- `cargo test -p wiki-cli mcp::tests:: -- --nocapture`
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
