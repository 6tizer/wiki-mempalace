# Requirements: Outbox Consumer Cursors

## Functional Requirements

- Storage MUST expose a consumer-scoped export API.
- The API MUST derive `start_after_id` from `wiki_outbox_consumer_progress`.
- Missing consumer progress MUST mean `start_after_id=0`.
- The API MUST include `consumer_tag`, `start_after_id`, `head_id`, `event_count`, and `ndjson`.
- `export-outbox-ndjson-from` MUST default to a consumer cursor.
- `consume-to-mempalace` MUST consume from its consumer cursor.
- `--last-id` MUST act only as a legacy/manual floor and MUST NOT rewind consumer progress.

## Non-Functional Requirements

- No schema migration beyond already-existing `wiki_outbox_consumer_progress`.
- No network calls in tests.
- Multiple consumers must remain independent.

## Acceptance

- Unit tests prove consumer A ack does not advance consumer B.
- CLI helper tests prove cursor export, fresh consumer export, and manual floor behavior.
- Existing outbox export/ack tests remain green.
