# Audit v2 PR 08 Production SearchPorts Handoff

## Branch

`codex/audit-v2-08-production-searchports`

## Status

Implemented locally. Full verification passed.

## Completed

- Added `wiki_storage::SqliteSearchPorts`.
- Rewired `wiki-cli query` / `explain` to prefer storage-backed wiki ports.
- Preserved mempalace composition through `CompositeSearchPorts`.
- Preserved invalid palace fallback, now to storage-backed wiki ports.
- Fixed graph extras so `mp_*` external ids are rejected and extras no longer
  replace the active mempalace graph stream.
- Added focused tests for persisted snapshot retrieval and CLI helper behavior.
- Updated query truth table docs and roadmap/spec index.

## Modified Files

- `crates/wiki-storage/src/lib.rs`
- `crates/wiki-cli/src/main.rs`
- `docs/mempalace-linkage.md`
- `docs/specs/audit-v2-08-production-searchports/`
- `docs/handovers/audit-v2-08-production-searchports/summary.md`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/LESSONS.md`

## Interfaces

- New public type: `wiki_storage::SqliteSearchPorts`.
- `query` / `explain` production path now prefers storage-backed wiki ports.

## Known Limits

- Lexical tokenization quality is unchanged; PR 09 owns CJK/Unicode retrieval.
- `--vectors` remains an override path; this PR does not embed query text inside
  `SqliteSearchPorts`.

## Verification

Focused tests passed:

- `cargo test -p wiki-storage sqlite_search_ports_read_snapshot_rows_and_filter_scope -- --nocapture`
- `cargo test -p wiki-cli storage_ports -- --nocapture`
- `cargo test -p wiki-cli graph_extras -- --nocapture`

Review:

- Retrieval quality review found P1 graph extras / mempalace graph replacement;
  fixed with `mp_*` rejection and active-graph merge.

Full gate passed:

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo deny --all-features check advisories bans licenses sources`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
