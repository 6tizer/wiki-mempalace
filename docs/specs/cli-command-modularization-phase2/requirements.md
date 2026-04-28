# Requirements: CLI Command Modularization phase 2

## Functional Requirements

- **R1 dispatcher extraction**: Move no-engine early command dispatch out of `main.rs`.
- **R2 shared runtime setup**: Move repo/schema/engine/writer-lease setup into a shared runtime module.
- **R3 clap compatibility**: Keep `Cli` / `Cmd` definitions and global argument behavior unchanged.
- **R4 command smoke**: Add integration smoke coverage for global argument ordering.
- **R5 phase boundary**: Do not move individual write-heavy business command bodies in this PR.

## Acceptance Criteria

- [x] `commands/dispatch.rs` handles no-engine early command dispatch.
- [x] `commands/runtime.rs` owns shared runtime setup.
- [x] `main.rs` still owns clap enum and business command match.
- [x] CLI smoke covers valid and invalid global `--db` placement.
- [x] PR #79 + quick CI green.

## Checklist

- [x] No command/flag rename.
- [x] No output shape change.
- [x] No writer lease behavior change.
