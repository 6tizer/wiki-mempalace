# Handover: Audit v2 PR 10 Outbox + SQLite Reliability

## Branch

`codex/audit-v2-10-outbox-sqlite-reliability`

## Scope

Implements roadmap PR 10 / M-1 + M-8:

- Per-consumer outbox ack count no longer depends on global `processed_at`.
- Manual ack above outbox head clamps to current head.
- SQLite repository open sets `busy_timeout`.
- Storage multi-step write paths share a single `immediate_transaction` wrapper.
- Reliability tests cover second consumer ack and short write-lock wait.

## Changed Files

- `crates/wiki-storage/src/lib.rs`
- `docs/outbox-and-consumers.md`
- `docs/specs/audit-v2-10-outbox-sqlite-reliability/*`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/LESSONS.md`

## Verification

Completed:

- `cargo test -p wiki-storage -- --nocapture`
- reliability review; P1 future-cursor and P2 wrapper coverage findings fixed and re-review found no P0/P1/P2
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo deny --all-features check advisories bans licenses sources`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Notes

- No production wiki or palace writes were run.
- `OutboxStats.unprocessed_events` intentionally remains legacy/global; per-consumer backlog remains `wiki_outbox_consumer_progress` based.
- Single statement insert/upsert paths may remain direct rusqlite calls; the wrapper is for multi-step writes needing explicit commit/rollback.
