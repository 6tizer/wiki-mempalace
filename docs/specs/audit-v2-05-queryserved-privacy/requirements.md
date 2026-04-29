# Requirements: Audit v2 PR 05 QueryServed Privacy

## Goal

Stop storing raw query text in `WikiEvent::QueryServed` while keeping old
outbox events readable and M12 query-history suggestions scope-safe.

## Functional Requirements

- New `QueryServed` events must not serialize the raw query.
- New events must include:
  - `query_hash`
  - `query_hash_schema_version`
  - `schema_version`
  - `viewer_scope`
  - optional `redacted_preview`
- `query_fingerprint` remains for compatibility, but new events must store the
  hash value there, not raw query text.
- Legacy events that only contain `query_fingerprint`, `top_doc_ids`, and `at`
  must still deserialize.
- CLI `query` must record the active `--viewer-scope` in the event.
- M12/suggest must reject scoped query-history events whose event scope is not
  visible to the current scan viewer.
- M12/suggest must continue to support legacy events through the existing
  `top_doc_ids` visibility fallback.

## Non-Goals

- Do not encrypt query history.
- Do not add a user-visible query-history report.
- Do not change mempalace consumption; `QueryServed` remains ignored by the
  bridge.
- Do not run production wiki or palace writes.

## Acceptance

- Serialized new query events contain no raw query text.
- Old `QueryServed` JSON parses with defaulted schema/privacy fields.
- `wiki-cli query` outbox events include hash and viewer scope.
- M12/suggest does not generate query-history suggestions from mismatched
  event scopes.
- Focused tests and workspace fmt/test/clippy pass.
