# Tasks: J14 Semantic Fusion Benchmark

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Branch: `codex/j14-semantic-fusion-benchmark`
- [x] Runner comparison flag
- [x] Variant config switching
- [x] JSON / Markdown variant metrics
- [x] Workflow dispatch input + scheduled default
- [x] Fixture regression test
- [x] Handoff: `docs/handovers/semantic-fusion-benchmark/summary.md`
- [x] PR + CI green
- [x] PRD / roadmap updated

## Verification

- `python3 -m py_compile scripts/longmemeval_run.py`
- `python3 tests/longmemeval_runner_test.py`
- `bash -n scripts/longmemeval_fetch.sh`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
