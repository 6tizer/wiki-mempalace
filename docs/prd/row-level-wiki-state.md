# PRD: Row-level Wiki State Storage

## Goal

Prepare `wiki_state` for row-level storage without breaking existing snapshot
compatibility or rollback.

## Scope

- Add a row-level SQLite table for snapshot collections.
- Keep the existing single-row `wiki_state` JSON blob as the primary read path.
- Dual-write row-level state in the same transaction as `wiki_state` and outbox.
- Add fallback load from row-level state when the blob is absent.
- Add migration/rollback tests.

## Non-Goals

- Removing `wiki_state`.
- Making row-level state the primary read path.
- Adding CLI-visible migration commands.
- Changing `WikiRepository` caller behavior.

## Success Criteria

- Existing DBs still load through `wiki_state`.
- Every snapshot save mirrors sources, claims, pages, entities, edges, and audits
  into row-level storage.
- Row-level writes roll back with snapshot/outbox failures.
- Fallback row load can reconstruct a snapshot if the blob is missing.

## Status

- **Migration complete PR #76** — row-level mirror, dual-write, fallback load,
  and GitHub quick check completed.
- **Cutover pending** — primary row read and compatibility cleanup remain in the
  next PR.
