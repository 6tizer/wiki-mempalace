# Design: Audit v2 PR 08 Production SearchPorts Default

## Storage-backed Wiki Port

`wiki_storage::SqliteSearchPorts` implements `wiki_core::SearchPorts` and opens
from `SqliteRepository::load_snapshot()`.

It uses the same lexical scoring contract as the previous in-memory wiki port:

- claims/pages feed BM25 and lexical vector fallback streams;
- entities feed graph stream;
- stale claims are skipped;
- viewer scope is enforced with `document_visible_to_viewer`.

The implementation intentionally does not add new SQL indexes or FTS tables in
this PR. It moves the production default boundary from live engine memory to
persisted `wiki.db` state. PR 09 owns retrieval token quality.

## CLI Wiring

`build_wiki_search_ports(repo, eng, viewer)` tries `SqliteSearchPorts` first.
If storage port creation fails, it logs a warning and falls back to
`InMemorySearchPorts`.

`run_fusion_query(...)` now receives the repository and builds:

- wiki-only storage port when `--palace-db` is absent;
- `CompositeSearchPorts(SqliteSearchPorts, MempalaceSearchPorts)` when
  `--palace-db` opens successfully;
- wiki-only storage port when `--palace-db` fails.

`--graph-extras-file` is merged after the active graph stream is built. With a
valid `--palace-db`, that active stream is the composite wiki + mempalace graph
stream, so extras do not replace mempalace graph candidates.

`explain` uses the same storage-backed wiki port for the displayed wiki streams.

## Compatibility

- `--vectors` still supplies a query-vector override from `wiki_embedding`.
- `--graph-extras-file` rejects externally injected `mp_*` ids and merges
  viewer-filtered extras into the active graph stream.
- `InMemorySearchPorts` remains public and unchanged for tests/fallback.
