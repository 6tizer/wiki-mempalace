# Handoff: Outbox Consumer Cursors

## Branch

`codex/outbox-consumer-cursors-api`

## Merge

Phase 1 PR #62 merged on 2026-04-28.

## Completed

- Added `OutboxConsumerCursorExport`.
- Added `export_outbox_ndjson_for_consumer(consumer_tag)`.
- Cursor export derives `start_after_id` from `wiki_outbox_consumer_progress`.
- Existing ID-based export and CLI behavior are unchanged.

## Deferred

- CLI/consumer cutover remains the next PR.
- Legacy `--last-id` remains available.

## Verification

- `cargo test -p wiki-storage outbox_export_for_consumer_uses_independent_cursor` passed.
- `cargo test --workspace` passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
