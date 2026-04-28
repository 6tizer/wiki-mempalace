# PRD: Multi-process Writer Lease

## Status

Implemented in PR #64.

## Goal

Prevent two independent writer processes from mutating the same `wiki.db` snapshot/outbox state at the same time.

## Scope

- Add a repository-adjacent writer lease file for `wiki.db`.
- Writer commands acquire the lease before loading `LlmWikiEngine`.
- Lease acquisition fails fast when another unexpired writer owns it.
- Read-only commands continue without a lease.
- Expired leases may be reclaimed to recover from crashed processes.

## Out of Scope

- Row-level state migration.
- Embedding transaction atomicity.
- Cross-host distributed locks.
- Rewriting command behavior beyond adding the pre-write guard.

## Success Criteria

- Acquiring a second lease for the same DB fails while the first is alive.
- Dropping a lease releases it.
- Expired lease files can be replaced.
- CLI writer classification covers known DB write commands and keeps read-only commands unblocked.
