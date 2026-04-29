# Design: Audit v2 PR 05 QueryServed Privacy

## Event Contract

`WikiEvent::QueryServed` keeps the existing `query_fingerprint` field so older
readers do not lose the event. New producers populate it with the same value as
`query_hash`.

New fields:

- `query_hash`: `sha256:v1:<hex>` over a fixed schema salt plus query bytes.
- `query_hash_schema_version`: hash contract version.
- `schema_version`: QueryServed payload schema version; missing means legacy
  v1.
- `viewer_scope`: the query viewer scope used at production time.
- `redacted_preview`: optional and omitted by default.

## Flow

- `LlmWikiEngine::record_query` takes the raw query plus optional viewer scope.
- The raw query is hashed immediately via `WikiEvent::query_served(...)`.
- Audit text records the hash, not the raw query.
- CLI `query` passes the active viewer scope into `record_query`.
- Query pipelines using `QueryContext` pass `ctx.viewer_scope`.

## Compatibility

- `WikiEvent::legacy_query_served(...)` creates old-shape events for tests and
  compatibility fixtures.
- Serde defaults keep old NDJSON readable:
  - missing `schema_version` becomes `1`;
  - missing hash/scope/preview fields become `None`.
- M12/suggest:
  - new scoped events must match the current viewer before `top_doc_ids` are
    considered;
  - new scoped events are skipped when a scan has no explicit viewer;
  - legacy scope-less events keep the existing doc-visibility fallback.

## Tests

- `wiki-core` tests serialization and legacy deserialization.
- `wiki-kernel` tests `record_query`, scoped M12 filtering, and raw-query
  omission in suggestions.
- `wiki-cli` integration test runs `query`, exports outbox, and verifies hash
  plus viewer scope without raw query text.
