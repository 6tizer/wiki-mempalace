# Requirements: Multi-process Writer Lease

## Functional Requirements

- Storage MUST expose a `SqliteWriterLease` for a concrete DB path.
- Lease acquire MUST use atomic file creation.
- A live, unexpired lease MUST fail fast with a typed storage error.
- Drop MUST release only the lease owned by the current process owner.
- Expired lease files MAY be removed and replaced.
- CLI write commands MUST acquire the lease before loading engine state.
- MCP runtime MUST acquire the lease before serving requests.
- Read-only/report commands MUST NOT require the lease.

## Non-Functional Requirements

- No new third-party dependency.
- No SQLite schema change.
- Lock file path must be deterministic and next to the DB.
- Tests must avoid production DB/Vault paths.

## Acceptance

- Storage tests cover acquire, busy, release, stale replacement, and owner-safe drop.
- CLI unit tests cover representative write/read command classification.
- Existing workspace tests remain green.
