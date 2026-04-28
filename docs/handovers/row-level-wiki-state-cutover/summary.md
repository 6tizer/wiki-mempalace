# Handoff: Row-level Wiki State cutover/cleanup

## Status

Complete in PR #77. GitHub quick passed; merge pending.

## Scope

PR #77 makes row-level state the primary snapshot read path when rows exist.
The legacy `wiki_state` blob remains dual-written and available for rollback.

## Changed

- `load_snapshot` now reads from `wiki_state_row` first when row data exists.
- Empty row state still falls back to the legacy `wiki_state` blob.
- `SqliteRepository::verify_row_state_matches_blob()` provides read-only
  row/blob comparison evidence.
- Storage tests cover row-primary read, blob fallback, and verification match /
  mismatch.

## Recovery

Before removing blob compatibility, recovery remains:

1. back up the DB;
2. run `DELETE FROM wiki_state_row;`;
3. restart the process so `load_snapshot` falls back to `wiki_state`;
4. run a normal writer command to repopulate row-level state.

## Verification

- `cargo test -p wiki-storage row_level_state -- --nocapture`
- `cargo test -p wiki-storage`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## Next

After PR #77 merges, continue roadmap item 19: `CLI Command Modularization phase 1`.
