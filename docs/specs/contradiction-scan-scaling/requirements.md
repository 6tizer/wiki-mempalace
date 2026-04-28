# Requirements: Contradiction Scan Scaling

## Functional Requirements

- **R1 stale prefilter**: stale claims must be excluded before pair generation.
- **R2 scope bucket**: contradiction candidates must be generated within the
  same `Scope` bucket only.
- **R3 sparse candidate index**: claims without supported contradiction signals
  must not participate in pair generation.
- **R4 bounded work**: each claim may inspect at most a fixed number of
  candidate claims.
- **R5 compatibility**: `naive_contradiction_pairs` keeps the same public API
  and still validates candidate pairs through `contradicts_heuristic`.

## Acceptance Criteria

- [x] Focused test covers stale prefilter and scope bucket behavior.
- [x] Focused test covers unrelated-claim signal skipping.
- [x] Candidate cap is documented in design.
- [x] PR + CI green.

## Checklist

- [x] Public API unchanged
- [x] Deterministic ordering
- [x] No DB or Vault behavior change
