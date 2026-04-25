# Requirements: Atomic Snapshot + Outbox Persistence

## Goal

- Make **snapshot** (`wiki_state`) and **outbox append** (`wiki_outbox` inserts)
  part of a **single** durability boundary when the engine commits work that
  includes both, so a crash between steps cannot leave inconsistent combined
  state.

## Context

- Post–C15 review, `LlmWikiEngine::save_to_repo` and
  `flush_outbox_to_repo_with_policy` remain separate `WikiRepository` calls; each
  `append_outbox` is its own autocommit unless batched.
- `mark_outbox_processed` was fixed to use an explicit `BEGIN IMMEDIATE` …
  `COMMIT` transaction; this spec addresses the **write** path pairing snapshot
  + outbox, not consumer acks alone.

## Functional Requirements

- **R1 (atomic unit)**: Expose an API (name TBD) that, for a given engine
  “commit” operation, writes the snapshot and **all** outbox events flushed in
  that call inside **one** SQLite transaction (or provably equivalent: one
  `COMMIT` after all related writes).
- **R2 (ordering)**: Within that unit, `wiki_state` must reflect the in-memory
  state **after** the outbox events that are persisted in the same unit are
  logically emitted (same order as in-memory `emit` + flush order).
- **R3 (partial failure)**: If any statement in the unit fails, no partial
  snapshot and no partial subset of the batch outbox rows may become visible
  as committed (rollback or no-op the whole unit).
- **R4 (callers)**: MCP `save_and_flush`, CLI subcommands that both save and
  flush, and any other documented entry points must use the new API; no
  “save then flush in two autocommit passes” in production code paths.
- **R5 (tests)**: Add integration test(s) that assert atomicity, e.g. inject
  failure on second write and verify first write is not visible, or use SQLite
  savepoint/rollback in test double.

## Non-Goals

- Loading/replaying outbox into memory on `load_from_repo` (separate product
  decision).
- Cross-process distributed transactions (single `wiki.db` connection scope
  only).

## Inputs / Outputs

- **Input**: `StorageSnapshot` + ordered list of `WikiEvent` to append (or
  engine reference + callback that performs snapshot serialization inside the
  transaction).
- **Output**: `Result<usize, StorageError>` (count of outbox rows written) or
  unified `Result<(), StorageError>` with metrics elsewhere.

## Acceptance Criteria

- [ ] One public API on `SqliteRepository` (or `WikiRepository` extension) that
      performs snapshot + N outbox inserts in a single `COMMIT`.
- [ ] `wiki-cli` / `mcp` use that API for save+flush; grep shows no
      save-then-separate-flush in those paths.
- [ ] `cargo test --workspace` green; at least one test proves rollback on
      injected failure.
- [ ] `AGENTS.md` or `docs/outbox-and-consumers.md` updated to describe the new
      durability unit.

## Checklist

- [ ] Behavior matches PRD scope
- [ ] Inputs/outputs explicit
- [ ] Error and rollback cases covered
- [ ] No silent partial commit
