# Requirements: Row-level Wiki State cutover/cleanup

## Functional Requirements

- **R1 row-primary read**: `load_snapshot` reads row-level state first when rows
  are present.
- **R2 blob fallback**: Existing `wiki_state` blob remains a fallback when row
  state is absent.
- **R3 dual-write retained**: Snapshot saves still write both rows and blob for
  rollback.
- **R4 verification API**: Storage exposes a read-only verification method that
  compares row-level state with the blob.
- **R5 recovery docs**: Handoff documents how to recover by clearing rows and
  falling back to the blob.
- **R6 no CLI behavior drift**: Existing repository callers keep the same
  public trait contract.

## Acceptance Criteria

- [x] Tests prove row-level state is primary when present.
- [x] Tests prove blob fallback when rows are absent.
- [x] Tests prove row/blob verification detects match and mismatch.
- [x] PR + CI green.

## Checklist

- [x] Keep `wiki_state` rollback-compatible.
- [x] No production apply in this PR.
- [x] Compatibility removal deferred until production verification exists.
