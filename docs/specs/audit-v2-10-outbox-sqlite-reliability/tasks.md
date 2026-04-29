# Tasks: Audit v2 PR 10 Outbox + SQLite Reliability

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-v2-10-outbox-sqlite-reliability/` | Complete |
| Per-consumer ack count | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| SQLite busy timeout | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Transaction wrapper cleanup | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Focused reliability tests | Agent | Main | `crates/wiki-storage/src/lib.rs` tests | Complete |
| Reliability review + gates | Main/Subagent | workspace | Complete |

## Checklist

- [x] Behavior matches roadmap scope
- [x] second consumer ack count ignores legacy `processed_at`
- [x] manual ack above current head clamps to current head
- [x] repeat ack for same consumer returns `0`
- [x] legacy `processed_at` remains populated for old observability
- [x] `SqliteRepository::open` sets `busy_timeout`
- [x] short write lock test passes through wrapped snapshot/outbox write path
- [x] `cargo test -p wiki-storage -- --nocapture`
- [x] reliability review complete; P1 future-cursor and P2 wrapper coverage findings fixed
- [x] `cargo fmt --all -- --check`
- [x] `git diff --check`
- [x] `cargo deny --all-features check advisories bans licenses sources`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
