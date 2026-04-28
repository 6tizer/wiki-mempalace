# Design: Contradiction Scan Scaling

## Summary

Replace visible all-pairs scanning with a deterministic bounded candidate scan:

1. Filter invisible and stale claims before indexing.
2. Drop claims without contradiction signals.
3. Bucket remaining claims by exact `Scope`.
4. Build a small in-memory index for opposing signals:
   - `不是` ↔ `是`
   - `cannot` ↔ `can `
5. For each claim, inspect at most
   `CONTRADICTION_MAX_CANDIDATES_PER_CLAIM` candidates.
6. Preserve existing final judgment through `contradicts_heuristic`.

## Bounds

Current cap: `256` candidates per claim.

This changes the worst-case scan from all visible pairs to
`active_signal_claims * 256` heuristic checks per scope bucket.

## Compatibility

- Function signature remains unchanged.
- Return type remains `Vec<ContradictionHint>`.
- Existing heuristic remains the final filter.
- Cross-scope pairing is intentionally removed to match scope isolation rules.

## Future Work

- Add embedding/semantic candidate providers after J14 fusion evidence exists.
- Add metrics for skipped/capped candidates if this becomes operator-visible.
