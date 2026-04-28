# PRD: Benchmark Reproducibility

## Status

Implemented in PR #70.

## Goal

Make `rust-mempalace bench --mode random` reproducible across runs by accepting an explicit seed and recording that seed with each benchmark run.

## Scope

- Add `--seed <u64>` to `rust-mempalace bench`.
- Allow `--seed` only with `--mode random`.
- Store the seed in `benchmark_runs`.
- Include the seed in CLI JSON output, benchmark reports, and latest benchmark reads.
- Preserve current behavior when no seed is provided.

## Out of Scope

- Changing retrieval ranking or benchmark scoring.
- Adding semantic/query fusion benchmarks; that remains J14.
- Making low recall fail CI.
- Changing LongMemEval scheduled workflow cadence.

## Success Criteria

- Same DB + same `--mode random --seed N` uses the same random sample order.
- `benchmark_runs.seed` records `N` for seeded random runs.
- Unseeded random runs remain randomized.
- `--mode fixed --seed N` fails fast instead of silently ignoring the seed.
- Focused unit and e2e tests cover deterministic selection, CLI output, and DB persistence.
