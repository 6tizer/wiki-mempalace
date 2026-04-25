# Design: ANN-Backed wiki_embedding Search

## Summary

- Add an **optional** ANN (approximate or exact vector index) layer for
  `doc_id` → embedding blob, co-located with the existing `wiki_embedding`
  table in `wiki.db`.

## Options (evaluate before implementation)

| Option | Pros | Cons |
| --- | --- | --- |
| **sqlite-vec** (`vec0` virtual table) | Native SQLite extension, in-process | Build/link complexity with `libsqlite3-sys` + bundle |
| **sqlite-vss** (FAISS-backed) | Mature | Heavier build, FAISS dependency |
| **Keep scan + `LIMIT` prefilter in SQL** | No extension | Not true ANN, still O(n) |
| **External index file** (e.g. hnswlib sidecar) | Full control | Two sources of truth, backup pain |

**Recommendation in spec (non-binding)**: Prototype `sqlite-vec` behind
`--features ann-embed` (name TBD), fall back to current scan when extension not
loaded.

## Data Model

- **Existing**: `wiki_embedding(doc_id TEXT PK, dim INT, vec BLOB, updated_at)`.
- **New** (illustrative): virtual table or shadow table, e.g.
  `wiki_embedding_vvec(doc_id, embedding)` with RLS/triggers to mirror upserts
  and deletes from `delete_embedding`.

## Query Flow

1. `upsert_embedding`: within same transaction, update blob row **and** index
   row, or `INSERT` into virtual table.
2. `search_embeddings_cosine`:
   - If ANN available: KNN or range query → at most `limit * k_probe` distance
     computations in extension, return map to `doc_id` + re-score if needed.
   - Else: current full scan (documented C15 “slow path”).

## `rusqlite` + bundled SQLite

- Project uses `rusqlite` with `bundled`. Loading extensions may require
  `Connection::load_extension` and shipping a prebuilt extension binary per
  **platform** in release artifacts — a **compatibility and release**
  decision; document in design update before writing code.

## API Surface

- **No** change to `search_embeddings_cosine` signature; internal dispatch only.
- Optional: add `ann_enabled: bool` on `SqliteRepository` from config, or
  detect `PRAGMA compile_options` / `try_load` at `open` time.

## Edge Cases

- **Dimension change**: if `doc_id` row changes `dim`, index row must
  replace, not update in place with wrong size.
- **Empty table**: return `Ok([])` as today; no ANN call.
- **Corrupt index**: mark rebuild + fall back to scan, log `warn!` (or
  return error, product choice — lock in requirements when implementing).

## Test Strategy

- Unit: with ANN off, same golden vectors as current `embedding_cosine_ranking` test.
- With ANN on (CI optional job): run same assertions on result **set** (order may
  differ for ANN; if approximate, use recall@k or exact re-rank in SQL).

## Spec Sync Rules

- Any change to `upsert_embedding` contract must update
  [requirements](requirements.md) and migration section.

## References

- `crates/wiki-storage/src/lib.rs` — `search_embeddings_cosine`, `upsert_embedding`
- Roadmap J14 / J13 notes on semantic lane budget
