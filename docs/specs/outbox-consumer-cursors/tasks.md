# Tasks: Outbox Consumer Cursors

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/outbox-consumer-cursors-api`
- [x] Phase 1 implementation
- [x] Focused tests
- [x] Workspace tests
- [x] Clippy
- [x] Handoff
- [ ] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Add cursor export struct | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Add consumer-scoped export API | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Preserve legacy export behavior | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Phase 2 CLI/consumer cutover | Agent | Future | `crates/wiki-cli/src/main.rs` | Pending |

## Verification

- `cargo test -p wiki-storage outbox_export_for_consumer_uses_independent_cursor`
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
