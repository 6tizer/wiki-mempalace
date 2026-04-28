# Tasks: M12 Executor Guarded Apply

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Branch: `codex/m12-executor-guarded-apply`
- [x] Add `suggestion_reason` evidence to plan actions
- [x] Add `suggest-executor-apply`
- [x] Enforce dry-run-first / explicit apply / allowlist
- [x] Validate current-state auto fix before apply
- [x] Add execution report text/JSON/Markdown output
- [x] Tests added
- [x] Handoff written
- [x] PR #82 opened
- [x] PR #82 quick CI green
- [x] PR #82 merged
- [x] Roadmap / PRD final status updated

## Verification

- `cargo test -p wiki-core strategy -- --nocapture`
- `cargo test -p wiki-cli --test suggest -- --nocapture`
- `cargo test -p wiki-cli`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`
