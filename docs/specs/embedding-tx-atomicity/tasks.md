# Tasks: Embedding Tx Atomicity

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/embedding-tx-atomicity`
- [x] Storage transaction API
- [x] CLI vector write paths updated
- [x] MCP vector write paths updated
- [x] Compiler vector write paths updated
- [x] Rollback tests
- [x] Handoff
- [x] Local gate
- [x] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Storage atomic API | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| CLI vectors commit boundary | Agent | Main | `crates/wiki-cli/src/main.rs` | Complete |
| MCP vectors commit boundary | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Compiler vectors commit boundary | Agent | Main | `crates/wiki-cli/src/wiki_compiler.rs` | Complete |
| Docs/roadmap/handoff | Script | Main | `docs/` | Complete |

## Verification

- `cargo test -p wiki-storage snapshot_outbox_and_embeddings_commit_together -- --nocapture`
- `cargo test -p wiki-storage snapshot_and_outbox_roll_back_when_embedding_insert_fails -- --nocapture`
- `cargo test -p wiki-cli --quiet`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Merge

- PR #71 merged on 2026-04-28.
