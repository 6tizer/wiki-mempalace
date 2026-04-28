# Tasks: MCP Typed Errors

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/mcp-typed-errors`
- [x] Implementation
- [x] Focused tests
- [x] Workspace tests
- [x] Clippy
- [x] Handoff
- [x] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Define MCP error enum | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Map JSON-RPC error codes/kinds | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Convert tool errors at MCP edge | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Document error shape | Script | Main | `docs/mcp-api-reference.md` | Complete |

## Verification

- `cargo test -p wiki-cli mcp::tests:: -- --nocapture`
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Merge

- PR #60 merged on 2026-04-28.
