# PRD: Embedding Tx Atomicity

## Status

Implemented in PR #71.

## Goal

Eliminate states where a source or claim is committed to `wiki_state` / `wiki_outbox` but its requested embedding write fails afterward.

## Scope

- Add a SQLite repository API that commits snapshot, outbox events, and embedding rows in one transaction.
- Update vector-enabled CLI write paths for ingest, ingest-llm, file-claim, and supersede-claim.
- Update vector-enabled MCP write paths for ingest, ingest-llm, file-claim, and supersede-claim.
- Update compiler batch ingest vector paths.
- Add rollback tests for embedding failure after snapshot/outbox writes.

## Out of Scope

- ANN/vector index implementation.
- Changing embedding model calls or retry behavior.
- Making Vault projection part of the SQLite transaction.
- Retrofitting historical rows missing embeddings.

## Success Criteria

- If embedding insertion fails, the same transaction rolls back snapshot and outbox writes.
- If embedding generation fails before commit, no DB write is attempted for that logical operation.
- Existing non-vector write paths behave as before.
- Tests prove success and rollback behavior.
