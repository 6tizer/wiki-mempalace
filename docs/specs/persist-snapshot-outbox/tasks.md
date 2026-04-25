# Tasks: Atomic Snapshot + Outbox Persistence

## Checklist

- [x] Requirements approved
- [x] Design approved: option A, `BEGIN IMMEDIATE` API on `WikiRepository`
- [x] Plan approved
- [x] Branch: `codex/persist-snapshot-outbox`
- [x] Implementation
- [x] Tests (failure injection or rollback proof)
- [x] Docs: `AGENTS.md` / `docs/outbox-and-consumers.md`
- [x] Handoff: `docs/handovers/persist-snapshot-outbox/summary.md`
- [x] PR + CI green
- [x] PRD / roadmap / spec status updated

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| `SqliteRepository` batched `BEGIN…COMMIT` API | Agent | Main | `crates/wiki-storage/` | Design lock | Complete |
| `LlmWikiEngine` + CLI/MCP use batched path | Agent | Main | `crates/wiki-kernel/`, `crates/wiki-cli/` | Storage API | Complete |
| `WikiRepository` trait update + impls | Agent | Main | `wiki-storage` | Storage API | Complete |
| Rollback/atomicity integration test | Agent | Main | `crates/wiki-storage/src/lib.rs` | API | Complete |
| Doc sync | Script | Main | `AGENTS.md`, `docs/outbox-and-consumers.md` | Implementation | Complete |

## Review Gates

- No partial commit on simulated failure.
- Grep: no `save_to_repo` immediately followed by `flush_outbox` in MCP/CLI
  without the new API (allow listed legacy tests if any).

## Stop Conditions

- If `&self` + `transaction()` is blocking, document chosen approach (A/B/C in
  design) and re-review before coding.

## Verification

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- Manual: `wiki-cli ingest` + power-cut simulation (optional)

Focused verification run during implementation:

- `cargo test -p wiki-storage --quiet`
- `cargo test -p wiki-kernel --quiet`
- `cargo test -p wiki-cli --test vault_backfill --quiet`
- `cargo test -p wiki-cli --test automation_run_daily --quiet`
- `cargo test -p wiki-cli --quiet`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

PR #25 merged on 2026-04-25.
