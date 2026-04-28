# Handoff: Benchmark Reproducibility

## Status

Implementation merged in PR #70 on 2026-04-28.

## Scope

`rust-mempalace bench --mode random` now supports an explicit seed so recall runs can be compared across time against the same sampled drawer set.

## Changed

- CLI adds `bench --seed <u64>`.
- `--seed` is rejected for `--mode fixed`.
- Random selection uses `StdRng::seed_from_u64(seed)` when provided.
- `benchmark_runs` has nullable `seed INTEGER`.
- CLI JSON/text output, benchmark reports, and latest benchmark reads include seed.
- Tests cover repeatable selection, CLI output, DB storage, and invalid fixed+seed usage.

## Verification

- Focused tests passed locally.
- Full workspace gate passed locally.

## Next

After merge, continue roadmap item 12: `Embedding Tx Atomicity`.
