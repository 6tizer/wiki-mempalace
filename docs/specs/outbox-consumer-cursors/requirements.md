# Requirements: Outbox Consumer Cursors

## Functional Requirements

- Storage MUST expose a consumer-scoped export API.
- The API MUST derive `start_after_id` from `wiki_outbox_consumer_progress`.
- Missing consumer progress MUST mean `start_after_id=0`.
- The API MUST include `consumer_tag`, `start_after_id`, `head_id`, `event_count`, and `ndjson`.
- Phase 1 MUST NOT change existing CLI defaults.

## Non-Functional Requirements

- No schema migration beyond already-existing `wiki_outbox_consumer_progress`.
- No network calls in tests.
- Multiple consumers must remain independent.

## Acceptance

- Unit tests prove consumer A ack does not advance consumer B.
- Existing outbox export/ack tests remain green.
