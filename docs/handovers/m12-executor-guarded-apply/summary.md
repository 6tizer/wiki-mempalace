# Handoff: M12 Executor Guarded Apply

## Status

Complete in PR #82. GitHub quick CI passed.

## Scope

This PR adds guarded apply for M12 executor plans. It only applies allowlisted
`fix_auto_safe` actions after validating the plan against current auto fixes.

## Changed

- Added `suggestion_reason` evidence to `StrategyExecutionAction`.
- Added `wiki-cli suggest-executor-apply`.
- Added explicit `--apply` and `--allow fix-auto-safe` gates.
- Added execution report text/JSON/Markdown output.

## Safety

- Default is preflight only.
- `--apply` without allowlist fails.
- No shell execution of `command_preview`.
- Stale or unmatched plan actions are skipped.

## Verification

- `cargo test -p wiki-core strategy -- --nocapture`
- `cargo test -p wiki-cli --test suggest -- --nocapture`
- `cargo test -p wiki-cli`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`
