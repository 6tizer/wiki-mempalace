# Requirements: Audit v2 PR 08 Production SearchPorts Default

## Goal

Make production `query` / `explain` use storage-backed wiki search ports by
default, while preserving mempalace fusion and keeping `InMemorySearchPorts` as
test/fallback plumbing.

## Functional Requirements

- `wiki-cli query` must build wiki BM25/vector/graph candidates from `wiki.db`
  storage state by default.
- `wiki-cli explain` must show wiki streams from the same storage-backed port.
- `--palace-db` must still compose wiki candidates with `MempalaceSearchPorts`.
- Invalid `--palace-db` must skip mempalace and continue with wiki candidates.
- `InMemorySearchPorts` must remain available for unit tests and as an explicit
  fallback when storage-backed port creation fails.
- Existing `--vectors` and `--graph-extras-file` override behavior must remain
  compatible.
- Docs must include a query/explain truth table.

## Non-Goals

- Do not rewrite retrieval quality scoring or tokenization; CJK/Unicode quality
  is PR 09.
- Do not require a live production palace DB in tests.
- Do not mutate production `/Users/mac-mini/Documents/wiki`.
- Do not remove `InMemorySearchPorts`.

## Acceptance

- Storage-backed port tests prove persisted snapshot rows are searchable and
  scope-filtered.
- CLI helper tests prove query/explain can retrieve from storage even if the
  engine's in-memory store is cleared.
- Invalid palace path fallback still returns wiki results.
- Query truth table docs are updated.
- Full workspace fmt/test/clippy and cargo-deny pass.
