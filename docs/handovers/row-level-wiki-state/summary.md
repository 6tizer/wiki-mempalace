# Handoff: Row-level Wiki State migration/dual-write

## Status

Complete in PR #76. GitHub quick passed; merge pending.

## Scope

PR #76 adds row-level snapshot storage beside the existing `wiki_state` blob.
The blob remains the primary read path.

## Changed

- `wiki_state_row` schema added with collection/item key/position/payload columns.
- Snapshot saves now dual-write `wiki_state` and `wiki_state_row`.
- `load_snapshot` falls back to row-level reconstruction only when the blob is
  absent.
- Storage tests cover dual-write, stale row cleanup, and transaction rollback.

## Verification

- `cargo test -p wiki-storage row_level_state -- --nocapture`
- `cargo test -p wiki-storage`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`
- Production read-only duplicate-id check:
  `sources=1275/dups=0`, `claims=1574/dups=0`, `pages=4983/dups=0`,
  `entities=815/dups=0`, `audits=3874/dups=0`, `edges=549`

## Next

After PR #76 merges, continue roadmap item 18: `Row-level Wiki State Storage cutover/cleanup`.
