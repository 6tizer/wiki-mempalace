# Design: CLI Command Modularization phase 2

## Summary

Phase 2 extracts the dispatcher and shared setup boundary created by phase 1.
It keeps clap definitions and the large business command match in `main.rs` so
the CLI surface remains stable.

## Extracted Modules

- `commands/dispatch.rs`
  - handles early commands that do not need the loaded wiki engine;
  - covers automation list/dry-run plan, vault audit, orphan governance, and vault backfill.
- `commands/runtime.rs`
  - owns viewer scope parsing, wiki root/sync flags, writer lease acquisition, repo open, schema load, and engine load;
  - returns a single runtime bundle to the main dispatcher.

## Boundaries

`main.rs` still owns:

- clap `Cli` / `Cmd` definitions;
- command business match for engine-backed commands;
- shared helper functions used by command modules.

Future CLI cleanup can move command bodies one domain at a time. This PR avoids
changing command behavior while reducing setup coupling.

## Test Strategy

- command-level smoke for global `--db` before/after subcommand;
- existing writer lease tests;
- existing vault audit/backfill/orphan governance command tests;
- `cargo test -p wiki-cli`;
- `cargo test --workspace`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo fmt --all -- --check`;
- `git diff --check`.
