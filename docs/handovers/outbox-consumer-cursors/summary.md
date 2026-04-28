# Handoff: Outbox Consumer Cursors

## Branch

- `codex/outbox-consumer-cursors-api`
- `codex/outbox-consumer-cursors-cutover`

## Merge

- Phase 1 PR #62 merged on 2026-04-28.
- Phase 2 PR #63 merged on 2026-04-28.

## Completed

- Added `OutboxConsumerCursorExport`.
- Added `export_outbox_ndjson_for_consumer(consumer_tag)`.
- Cursor export derives `start_after_id` from `wiki_outbox_consumer_progress`.
- `export-outbox-ndjson-from` now defaults to `--consumer-tag mempalace`.
- `consume-to-mempalace` now consumes from the consumer cursor.
- `--last-id` remains available as a legacy/manual floor and cannot rewind progress.

## Verification

- `cargo test -p wiki-storage outbox_export_for_consumer_uses_independent_cursor` passed.
- `cargo test -p wiki-cli cursor -- --nocapture` passed.
- `cargo test --workspace` passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
