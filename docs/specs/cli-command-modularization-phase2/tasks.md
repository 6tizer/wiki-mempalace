# Tasks: CLI Command Modularization phase 2

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Branch: `codex/cli-command-modularization-phase2`
- [x] Extract no-engine dispatcher
- [x] Extract shared runtime setup
- [x] Preserve clap enum in `main.rs`
- [x] Add CLI command smoke tests
- [x] Handoff
- [x] PR #79 + quick CI green
- [x] PRD / roadmap updated

## Verification

- `cargo test -p wiki-cli`
- `cargo test -p wiki-cli --test cli_smoke -- --nocapture`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`
