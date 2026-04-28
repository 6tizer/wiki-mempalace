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

**Chosen path after spike**: keep `sqlite-vec` as the preferred implementation
target, but do not link or load a native extension in the default build. PR #72
adds the `ann-embed` Cargo feature and backend dispatch first; with the feature
enabled, storage still falls back to the exact full scan until the implementation
PR adds the virtual table / extension loading path.

Reasons:

- default `rusqlite` bundled builds remain portable;
- CI can prove the feature gate compiles without platform-specific binaries;
- the implementation PR can add native extension wiring behind the already
  tested gate instead of changing product semantics and build story together.

## Data Model

- **Existing**: `wiki_embedding(doc_id TEXT PK, dim INT, vec BLOB, updated_at)`.
- **Feature gate PR #72**: no new schema. `EmbeddingSearchBackend` reports
  `AnnFeatureFallback` when `ann-embed` is enabled and still reads
  `wiki_embedding`.
- **Implementation PR**: add a virtual table or shadow table, e.g.
  `wiki_embedding_vvec(doc_id, embedding)` with triggers or same-transaction
  writes from `save_snapshot_and_append_outbox_with_embeddings` /
  `delete_embedding`.

## Query Flow

1. `upsert_embedding`: within same transaction, update blob row **and** index
   row, or `INSERT` into virtual table. PR #71 already made source/claim blob
   writes atomic with snapshot/outbox; the implementation PR must preserve that
   boundary for any ANN secondary structure.
2. `search_embeddings_cosine`:
   - If `ann-embed` + ANN backend available: KNN or range query → at most `limit * k_probe` distance
     computations in extension, return map to `doc_id` + re-score if needed.
   - Else: current full scan (documented C15 “slow path”).

## `rusqlite` + bundled SQLite

- Project uses `rusqlite` with `bundled`. Loading extensions may require
  `Connection::load_extension` and shipping a prebuilt extension binary per
  **platform** in release artifacts — a **compatibility and release**
  decision; document in design update before writing code.

## API Surface

- **No** change to `search_embeddings_cosine` signature; internal dispatch only.
- `SqliteRepository::embedding_search_backend()` exposes which path is active:
  `FullScan` by default, `AnnFeatureFallback` when `ann-embed` is compiled but
  no native backend is active yet. The implementation PR will add a third
  backend variant for the real ANN path.

## Edge Cases

- **Dimension change**: if `doc_id` row changes `dim`, index row must
  replace, not update in place with wrong size.
- **Empty table**: return `Ok([])` as today; no ANN call.
- **Corrupt index**: mark rebuild + fall back to scan, log `warn!` (or
  return error, product choice — lock in requirements when implementing).

## Test Strategy

- Unit: with ANN off, same golden vectors as current `embedding_cosine_ranking` test.
- With `ann-embed` feature on: PR #72 CI runs
  `cargo test -p wiki-storage --features ann-embed
  embedding_ann_feature_gate_reports_backend_and_falls_back_to_scan`.
- With real ANN path on: run same assertions on result **set** (order may differ
  for ANN; if approximate, use recall@k or exact re-rank in SQL).

## Spec Sync Rules

- Any change to `upsert_embedding` contract must update
  [requirements](requirements.md) and migration section.

## References

- `crates/wiki-storage/src/lib.rs` — `search_embeddings_cosine`, `upsert_embedding`
- Roadmap J14 / J13 notes on semantic lane budget
