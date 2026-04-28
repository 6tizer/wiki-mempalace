# Requirements: Benchmark Reproducibility

## Functional Requirements

- `rust-mempalace bench` MUST accept `--seed <u64>`.
- `--seed` MUST be valid only when `--mode random`.
- When `--seed` is present, random sample selection MUST use a deterministic RNG initialized from that seed.
- When `--seed` is absent, random sample selection MUST keep the existing non-deterministic behavior.
- `benchmark_runs` MUST store the seed for seeded random runs.
- CLI JSON output and report files MUST include the seed field.
- Latest benchmark reads MUST expose the stored seed.

## Non-Functional Requirements

- The change MUST NOT alter search/ranking semantics.
- The schema migration MUST be additive and compatible with existing `palace.db` files.
- The implementation MUST NOT add new crate dependencies.

## Acceptance

- `cargo test -p rust-mempalace seeded_random_benchmark_selection_is_repeatable`
- `cargo test -p rust-mempalace --test e2e_core e2e_bench_fixed_vs_random`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
