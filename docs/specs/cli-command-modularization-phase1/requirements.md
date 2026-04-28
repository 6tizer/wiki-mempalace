# Requirements: CLI Command Modularization phase 1

## Functional Requirements

- **R1 module shell**: Add a `commands/` module tree for low-risk command handlers.
- **R2 no clap drift**: Do not move or change the `Cli` / `Cmd` clap definitions.
- **R3 low-risk domains**: Extract read-only or simple DB commands first:
  `schema-validate`, `llm-smoke`, outbox export/ack helpers.
- **R4 output compatibility**: Existing stdout/stderr text remains unchanged.
- **R5 test compatibility**: Existing focused tests keep passing after move.

## Acceptance Criteria

- [x] `commands/schema.rs` handles `schema-validate`.
- [x] `commands/llm_smoke.rs` handles `llm-smoke`.
- [x] `commands/outbox.rs` handles outbox export/ack helpers.
- [x] Main still owns clap enum and dispatcher.
- [x] PR #78 + quick CI green.

## Checklist

- [x] No command/flag rename.
- [x] No business logic rewrite.
- [x] Phase 2 remains separate.
