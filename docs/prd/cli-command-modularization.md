# PRD: CLI Command Modularization

## Goal

Reduce `wiki-cli/src/main.rs` size and coupling without changing CLI arguments
or behavior.

## Scope

- Split low-risk command handlers into `crates/wiki-cli/src/commands/`.
- Keep clap enum definitions stable in phase 1.
- Preserve command output and exit semantics.
- Add focused tests around moved command domains.
- Continue with dispatcher/shared-config extraction in phase 2.

## Non-Goals

- Renaming commands or flags.
- Moving all subcommands in one PR.
- Changing writer lease, global args, or clap parse behavior.
- Reworking command business logic.

## Success Criteria

- Phase 1 extracts low-risk commands with no behavior drift.
- Phase 2 extracts dispatcher/shared context and adds command-level smoke.
- `cargo test -p wiki-cli` and quick CI remain green.

## Status

- **Complete in PR #78/#79** — extracted low-risk command handlers, no-engine dispatch, shared runtime setup, and command smoke coverage.
