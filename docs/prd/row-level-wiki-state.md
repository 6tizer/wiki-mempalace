# PRD: Row-level Wiki State Storage

## Goal

Prepare `wiki_state` for row-level storage without breaking existing snapshot
compatibility or rollback.

## Scope

- Add a row-level SQLite table for snapshot collections.
- Switch row-level state to the primary read path after migration tests pass.
- Keep the existing single-row `wiki_state` JSON blob as a fallback until
  production verification proves row-level reads are stable.
- Dual-write row-level state in the same transaction as `wiki_state` and outbox.
- Add fallback load from row-level state when the blob is absent.
- Add migration/rollback tests.

## Non-Goals

- Removing `wiki_state`.
- Adding CLI-visible migration commands.
- Changing `WikiRepository` caller behavior.
- Removing blob rollback before production verification.

## Success Criteria

- Existing DBs still load through `wiki_state` when row-level state is absent.
- Every snapshot save mirrors sources, claims, pages, entities, edges, and audits
  into row-level storage.
- Row-level writes roll back with snapshot/outbox failures.
- Fallback row load can reconstruct a snapshot if the blob is missing.
- Row-level state becomes the primary read path when rows exist.
- A read-only verification API compares row-level state against the blob.

## Status

- **Migration complete PR #76** — row-level mirror, dual-write, fallback load,
  and GitHub quick check completed.
- **Complete PR #76/#77** — row-level mirror, dual-write, row-primary read,
  blob fallback, recovery notes, verification API, and GitHub quick checks
  completed.
- **Blob compatibility retained** — removal waits for production verification.
