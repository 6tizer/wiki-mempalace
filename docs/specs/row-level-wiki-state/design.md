# Design: Row-level Wiki State migration/dual-write

## Summary

This PR adds row-level storage as a compatibility layer beside the existing
single-row `wiki_state` JSON blob.

The blob remains the primary read path. Row-level state is written in the same
transaction so the next PR can cut reads over with real migration evidence.

## Schema

`wiki_state_row`:

- `collection TEXT`
- `item_key TEXT`
- `position INTEGER`
- `payload_json TEXT`
- `updated_at TEXT`
- primary key: `(collection, item_key)`

Collections:

- `sources`
- `claims`
- `pages`
- `entities`
- `edges`
- `audits`

ID-bearing records use their UUID as `item_key`. Edges have no stable id, so
they use deterministic ordinal keys and preserve order through `position`.

## Write Path

`save_snapshot_and_append_outbox_inner` now:

1. serializes and upserts `wiki_state`;
2. deletes previous `wiki_state_row` rows;
3. inserts current snapshot rows;
4. appends outbox events.

The caller transaction remains the durability boundary for snapshot, outbox,
embeddings, aliases, and Notion index changes.

## Read Path

`load_snapshot` keeps the old contract:

1. read `wiki_state` blob when present;
2. if missing, rebuild `StorageSnapshot` from `wiki_state_row`;
3. if both are absent, return an empty snapshot.

## Cutover Boundary

The next PR can make row-level state primary after validating production
backfill and recovery. This PR intentionally keeps the old blob for rollback.

## Test Strategy

- dual-write all collections and fallback reconstruction;
- stale rows removed on subsequent save;
- forced row-level insert failure rolls back blob and outbox.
