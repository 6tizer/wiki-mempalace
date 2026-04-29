# Audit v2 PR 04 Cross-Bank Drawer Dedupe Handoff

## Summary

Branch: `codex/audit-v2-04-cross-bank-dedupe`

This PR implements Audit Report Follow-up v2 item 4:

- Drawer uniqueness moves from global `content_hash` to
  `(bank_id, content_hash)`.
- Legacy `idx_drawers_hash` is dropped during migration.
- New `idx_drawers_bank_hash` is created idempotently.
- `mine_path`, `mine_path_convos`, and `LiveMempalaceSink` check duplicates
  within the active bank.

## Files Changed

- `crates/rust-mempalace/src/db.rs`
- `crates/rust-mempalace/src/service.rs`
- `crates/wiki-mempalace-bridge/src/live_sink.rs`
- `crates/wiki-mempalace-bridge/tests/live_sink.rs`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/specs/audit-v2-04-cross-bank-dedupe/`
- `docs/handovers/audit-v2-04-cross-bank-dedupe/summary.md`
- `docs/LESSONS.md`

## Verification

Focused checks:

- `cargo test -p rust-mempalace init_schema_migrates_old_bankless_tables_before_bank_indexes`
- `cargo test -p rust-mempalace mine_path_dedupes_by_bank_and_content_hash`
- `cargo test -p rust-mempalace mine_path_convos_dedupes_by_bank_and_content_hash`
- `cargo test -p wiki-mempalace-bridge same_page_content_can_exist_in_different_banks --features live`

Migration/reliability review found one P2 coverage gap for `mine_path_convos`;
the focused test above fixes it.

Required full gates passed locally:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
