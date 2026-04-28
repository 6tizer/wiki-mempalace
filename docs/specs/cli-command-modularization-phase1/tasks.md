# Tasks: CLI Command Modularization phase 1

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Branch: `codex/cli-command-modularization-phase1`
- [x] Add `commands/mod.rs`
- [x] Extract schema validate handler
- [x] Extract llm smoke handler
- [x] Extract outbox export/ack helpers
- [x] Preserve clap enum in `main.rs`
- [x] Handoff
- [x] PR #78 + quick CI green
- [x] PRD / roadmap updated

## Verification

- `cargo test -p wiki-cli outbox -- --nocapture`
- `cargo test -p wiki-cli schema_validate -- --nocapture`
- `cargo test -p wiki-cli`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`
