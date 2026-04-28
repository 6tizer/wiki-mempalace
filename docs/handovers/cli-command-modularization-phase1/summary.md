# Handoff: CLI Command Modularization phase 1

## Status

Complete in PR #78. GitHub quick CI passed.

## Scope

PR #78 starts CLI modularization with low-risk command domains only. Clap
definitions and the main dispatcher remain in `main.rs`.

## Changed

- Added `crates/wiki-cli/src/commands/`.
- Moved `schema-validate` execution to `commands/schema.rs`.
- Moved `llm-smoke` execution to `commands/llm_smoke.rs`.
- Moved outbox export/ack helpers to `commands/outbox.rs`.
- `main.rs` shrank from 5698 to 5647 lines.

## Verification

- `cargo test -p wiki-cli outbox -- --nocapture`
- `cargo test -p wiki-cli schema_validate -- --nocapture`
- `cargo test -p wiki-cli`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## Next

Continue roadmap item 20: `CLI Command Modularization phase 2`.
