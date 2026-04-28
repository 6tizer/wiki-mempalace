# Handoff: Embedding Tx Atomicity

## Status

Implementation merged in PR #71 on 2026-04-28.

## Scope

Vector-enabled source/claim writes now commit DB state, outbox, and embedding rows in one SQLite transaction.

## Changed

- Added `EmbeddingWrite`.
- Added `SqliteRepository::save_snapshot_and_append_outbox_with_embeddings`.
- CLI vector write paths compute embeddings before commit.
- MCP vector write paths no longer best-effort silently; embedding failure aborts the write.
- Compiler vector source/claim writes use the same atomic commit helper.
- Storage tests prove success and rollback on forced embedding insert failure.

## Verification

- Focused storage tests passed.
- `cargo test -p wiki-cli --quiet` passed.
- Full workspace gate passed locally.

## Next

After merge, continue roadmap item 13: `C16B Embedding ANN spike / feature gate`.
