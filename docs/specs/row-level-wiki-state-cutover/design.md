# Design: Row-level Wiki State cutover/cleanup

## Summary

This PR changes the snapshot read priority from blob-first to row-first while
preserving the old blob as rollback state.

## Read Order

`load_snapshot`:

1. count `wiki_state_row`;
2. if rows exist, rebuild `StorageSnapshot` from rows;
3. otherwise read `wiki_state`;
4. if neither exists, return an empty snapshot.

Malformed row-level state should fail fast when rows are present. Recovery is to
clear `wiki_state_row` and load from the still-maintained blob.

## Write Order

The PR keeps the PR #76 dual-write path:

1. upsert `wiki_state`;
2. replace `wiki_state_row`;
3. append outbox events.

The blob stays in place until production verification proves rows are stable.

## Verification API

`SqliteRepository::verify_row_state_matches_blob()` returns:

- whether the blob exists;
- row count;
- per-collection row counts;
- `matches_blob = Some(true/false)` when both blob and rows exist;
- `matches_blob = None` when one side is absent.

This is read-only and does not mutate production DBs.

## Recovery

If row-level state is suspected corrupt before compatibility cleanup:

1. back up the DB;
2. run `DELETE FROM wiki_state_row;`;
3. restart the process so `load_snapshot` falls back to `wiki_state`;
4. run a normal writer command to repopulate rows.

## Test Strategy

- row primary beats a deliberately stale blob;
- empty rows fall back to blob;
- verification returns match/mismatch evidence.
