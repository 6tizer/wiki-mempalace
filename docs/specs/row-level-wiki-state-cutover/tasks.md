# Tasks: Row-level Wiki State cutover/cleanup

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Branch: `codex/row-level-wiki-state-cutover`
- [x] Row-primary `load_snapshot`
- [x] Blob fallback when rows are absent
- [x] Read-only row/blob verification API
- [x] Recovery notes in handoff
- [x] Tests for primary/fallback/verification
- [x] Handoff
- [x] PR + CI green
- [x] PRD / roadmap updated

## Verification

- `cargo test -p wiki-storage row_level_state -- --nocapture`
- `cargo test -p wiki-storage`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`
