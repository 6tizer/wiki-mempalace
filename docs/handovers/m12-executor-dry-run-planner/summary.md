# Handoff: M12 Executor Dry-Run Planner

## Status

Complete in PR #81. GitHub quick CI passed.

## Scope

PR #81 adds an opt-in dry-run executor planner after `wiki-cli suggest`. It
does not execute writes.

## Changed

- Added `StrategyExecutionPlan` and typed dry-run actions in `wiki-core`.
- Added `wiki-cli suggest --executor-plan`.
- Added text, JSON envelope, and Markdown plan renderers.
- Added plan report siblings under `--report-dir`.

## Safety

- Default `wiki-cli suggest` output and JSON remain unchanged.
- `command_preview` is audit display only.
- Future apply must use typed `action_kind` plus allowlist, not shell parsing.

## Verification

- `cargo test -p wiki-core strategy -- --nocapture`
- `cargo test -p wiki-cli --test suggest -- --nocapture`
- `cargo test -p wiki-cli`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## Next

Implement guarded apply as the final roadmap item.
