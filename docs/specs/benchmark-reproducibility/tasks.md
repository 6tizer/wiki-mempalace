# Tasks: Benchmark Reproducibility

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/benchmark-reproducibility`
- [x] Add CLI `--seed`
- [x] Add deterministic random selection
- [x] Add `benchmark_runs.seed` migration
- [x] Add output/report/latest seed field
- [x] Add focused tests
- [x] Handoff
- [x] Local gate
- [x] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| CLI seed flag | Script | Main | `crates/rust-mempalace/src/cli.rs`, `src/main.rs` | Complete |
| Deterministic selection | Script | Main | `crates/rust-mempalace/src/service.rs` | Complete |
| Seed persistence | Script | Main | `crates/rust-mempalace/src/db.rs`, `src/service.rs` | Complete |
| Regression tests | Script | Main | `crates/rust-mempalace/src/service.rs`, `tests/e2e_core.rs` | Complete |
| Docs and roadmap | Script | Main | `docs/` | Complete |

## Verification

- `cargo test -p rust-mempalace seeded_random_benchmark_selection_is_repeatable -- --nocapture`
- `cargo test -p rust-mempalace --test e2e_core e2e_bench_fixed_vs_random -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Merge

- PR #70 merged on 2026-04-28.
