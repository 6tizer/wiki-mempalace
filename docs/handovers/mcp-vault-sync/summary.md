# Handoff: MCP Vault Sync

## Branch

`codex/mcp-vault-sync`

## Completed

- MCP runtime now receives `wiki_dir` only when `--sync-wiki` is enabled.
- MCP write paths persist DB/outbox, then run `write_projection` when `wiki_dir` is present.
- Projection failures return typed MCP `storage_error`.
- MCP success payloads stay unchanged.

## Deferred

- Mempalace outbox consumption remains separate.
- Multi-process writer lock/lease remains separate.

## Verification

- `cargo test -p wiki-cli mcp::tests:: -- --nocapture` passed.
- `cargo test --workspace` passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
