# Handoff: J14 Semantic Fusion Benchmark

## Status

Complete in PR #75. GitHub quick passed; merge pending.

## Scope

PR #75 adds side-by-side LongMemEval comparison for query baseline vs local
semantic fusion.

## Changed

- `scripts/longmemeval_run.py` accepts `--compare-semantic-fusion`.
- Comparison runs write `metrics_by_variant` and per-case `variant_results`.
- Markdown reports include a Variant Metrics section.
- Workflow dispatch/scheduled runs can enable semantic fusion comparison.
- Fixture tests cover the comparison artifact contract.

## Verification

- `python3 -m py_compile scripts/longmemeval_run.py`
- `python3 tests/longmemeval_runner_test.py`
- `bash -n scripts/longmemeval_fetch.sh`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Next

After PR #75 merges, continue roadmap item 17: `Row-level Wiki State Storage migration/dual-write`.
