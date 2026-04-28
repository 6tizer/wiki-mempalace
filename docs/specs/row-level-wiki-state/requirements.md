# Requirements: Row-level Wiki State migration/dual-write

## Functional Requirements

- **R1 schema**: SQLite creates a row-level state table keyed by collection and
  item key.
- **R2 dual-write**: Every snapshot save writes both `wiki_state` and row-level
  state in the same transaction.
- **R3 exact mirror**: Row-level state removes stale rows from prior snapshots.
- **R4 fallback read**: `load_snapshot` reads the blob first, then reconstructs
  from rows only when the blob is absent.
- **R5 rollback**: Any row-level write failure rolls back snapshot, outbox,
  embeddings, aliases, and Notion index changes in that transaction.
- **R6 compatibility**: Existing public repository methods and CLI behavior do
  not change.

## Acceptance Criteria

- [x] Storage tests prove dual-write and fallback reconstruction.
- [x] Storage tests prove stale row cleanup.
- [x] Storage tests prove transaction rollback on row-level failure.
- [x] PR + CI green.

## Checklist

- [x] Preserve `wiki_state` as rollback-compatible source.
- [x] No production apply required.
- [x] Cutover/cleanup deferred to next PR.
