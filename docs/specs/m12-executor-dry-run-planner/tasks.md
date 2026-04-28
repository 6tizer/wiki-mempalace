# Tasks: M12 Executor Dry-Run Planner

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Branch: `codex/m12-executor-dry-run-planner`
- [x] Add dry-run plan model
- [x] Add planner mapping
- [x] Add CLI `--executor-plan`
- [x] Add JSON/text/Markdown rendering
- [x] Tests added
- [x] Handoff written
- [x] PR #81 opened
- [x] PR #81 quick CI green
- [x] PR #81 merged
- [x] Roadmap / PRD final status updated

## Verification

- `cargo test -p wiki-core strategy -- --nocapture`
- `cargo test -p wiki-cli --test suggest -- --nocapture`
- `cargo test -p wiki-cli`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## Next

Continue with M12 executor guarded apply: allowlist, dry-run-first, and
explicit apply flag.
