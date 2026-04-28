# Design: Outbox Consumer Cursors

## Phase 1: Storage API

Add `OutboxConsumerCursorExport` and `export_outbox_ndjson_for_consumer(consumer_tag)` in `wiki-storage`.

The method:

1. Reads `wiki_outbox_consumer_progress` for the consumer.
2. Uses `acked_up_to_id.unwrap_or(0)` as `start_after_id`.
3. Calls the existing ID-based export internally.
4. Returns NDJSON plus cursor metadata.

## Phase 2: Cutover

The next PR will wire CLI and consumers to this API. Legacy `--last-id` stays as an explicit manual override until all callers are migrated.

## Compatibility

The existing `export_outbox_ndjson()` and `export_outbox_ndjson_from_id(last_id)` APIs stay unchanged in Phase 1.
