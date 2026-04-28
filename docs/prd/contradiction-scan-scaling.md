# PRD: Contradiction Scan Scaling

## Goal

Reduce `naive_contradiction_pairs` from visible all-pairs scanning to a bounded
candidate path that remains deterministic and scope-safe.

## Scope

- `wiki-kernel` contradiction hint scanning.
- Stale claim prefilter.
- Scope buckets.
- Sparse contradiction-signal index for `是` / `不是` and `can` / `cannot`.
- Per-claim candidate cap.

## Non-Goals

- LLM contradiction judgment.
- Embedding semantic contradiction discovery.
- Changing `ContradictionHint` shape or public caller API.

## Success Criteria

- Stale and invisible claims are filtered before candidate pairing.
- Claims from different scopes are not paired.
- Claims without contradiction signals are skipped.
- Each claim checks a bounded number of candidates.
- Existing heuristic output remains deterministic for covered pairs.

## Status

- **Complete PR #74** — implementation uses scope buckets, sparse signal index,
  candidate cap, and focused kernel tests.
