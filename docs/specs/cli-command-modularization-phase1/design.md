# Design: CLI Command Modularization phase 1

## Summary

Phase 1 creates `crates/wiki-cli/src/commands/` and moves only low-risk command
handler code. The clap surface stays in `main.rs`, so global argument ordering
and subcommand parsing remain unchanged.

## Extracted Domains

- `commands/schema.rs`
  - owns `schema-validate` execution.
- `commands/llm_smoke.rs`
  - owns `llm-smoke` execution.
- `commands/outbox.rs`
  - owns outbox export/ack helpers and the consumer cursor floor calculation.

## Main Boundaries

`main.rs` still owns:

- `Cli` / `Cmd` definitions;
- writer-lease classification;
- engine/repository setup;
- the large dispatcher match.

This keeps phase 1 mechanical and lowers risk. Phase 2 can move dispatcher
context once this module shell is stable.

## Test Strategy

- focused outbox cursor tests;
- existing schema validate integration tests;
- `cargo test -p wiki-cli`;
- `cargo test --workspace`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo fmt --all -- --check`;
- `git diff --check`.
