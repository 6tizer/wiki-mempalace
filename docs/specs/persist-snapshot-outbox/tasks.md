# Tasks: Atomic Snapshot + Outbox Persistence

## Checklist

- [ ] Requirements approved
- [ ] Design approved (trait vs `SqliteRepository`-only API)
- [ ] Plan approved
- [ ] Branch: `codex/persist-snapshot-outbox` (or user prefix)
- [ ] Implementation
- [ ] Tests (failure injection or rollback proof)
- [ ] Docs: `AGENTS.md` / `docs/outbox-and-consumers.md`
- [ ] Handoff: `docs/handovers/persist-snapshot-outbox/summary.md`
- [ ] PR + CI green
- [ ] PRD / roadmap / spec status updated

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| `SqliteRepository` batched `BEGIN…COMMIT` API | Agent | TBD | `crates/wiki-storage/` | Design lock | Not started |
| `LlmWikiEngine` + CLI/MCP use batched path | Agent | TBD | `crates/wiki-kernel/`, `crates/wiki-cli/` | Storage API | Not started |
| `WikiRepository` trait update + impls | Agent | TBD | `wiki-storage` | Storage API | Not started |
| Rollback/atomicity integration test | Agent | TBD | `crates/wiki-storage/tests` or `wiki-cli` | API | Not started |
| Doc sync | Script | TBD | `AGENTS.md`, `docs/outbox-and-consumers.md` | Implementation | Not started |

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
