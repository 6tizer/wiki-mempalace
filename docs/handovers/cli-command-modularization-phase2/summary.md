# Handoff: CLI Command Modularization phase 2

## Status

Complete in PR #79. GitHub quick CI passed.

## Scope

PR #79 finishes the staged CLI command modularization item by extracting
no-engine dispatch and shared runtime setup. Clap definitions and the
engine-backed business command match remain in `main.rs`.

## Changed

- Added `commands/dispatch.rs` for early commands that do not need the loaded engine.
- Added `commands/runtime.rs` for viewer/wiki/repo/schema/engine/writer-lease setup.
- Added `crates/wiki-cli/tests/cli_smoke.rs` for global arg ordering smoke coverage.
- `main.rs` shrank from 5647 to 5483 lines.

## Verification

- `cargo test -p wiki-cli`
- `cargo test -p wiki-cli --test cli_smoke -- --nocapture`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## Next

Continue roadmap item 21: `Time Library Unification`.
