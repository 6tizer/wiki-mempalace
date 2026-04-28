# Tasks: Contradiction Scan Scaling

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Branch: `codex/contradiction-scan-scaling`
- [x] Implement stale prefilter
- [x] Implement scope buckets
- [x] Implement sparse signal index and candidate cap
- [x] Focused tests
- [x] Handoff: `docs/handovers/contradiction-scan-scaling/summary.md`
- [x] PR + CI green
- [x] PRD / roadmap updated

## Verification

- `cargo test -p wiki-kernel contradiction_scan -- --nocapture`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
