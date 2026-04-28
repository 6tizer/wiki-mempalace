# Tasks: Outbox Consumer Cursors

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/outbox-consumer-cursors-api`
- [x] Phase 1 implementation
- [x] Branch: `codex/outbox-consumer-cursors-cutover`
- [x] Phase 2 CLI/consumer cutover
- [x] Focused tests
- [x] Workspace tests
- [x] Clippy
- [x] Handoff
- [x] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Add cursor export struct | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Add consumer-scoped export API | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Preserve legacy export behavior | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Phase 2 CLI/consumer cutover | Agent | Main | `crates/wiki-cli/src/main.rs` | Complete |

## Verification

- `cargo test -p wiki-storage outbox_export_for_consumer_uses_independent_cursor`
- `cargo test -p wiki-cli cursor -- --nocapture`
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Merge

- Phase 1 PR #62 merged on 2026-04-28.
- Phase 2 PR #63 merged on 2026-04-28.
