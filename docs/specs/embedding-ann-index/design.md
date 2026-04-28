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
| **Locality-bucket shadow index** | No native dependency; deterministic; bounded candidate scoring | Approximate; may fall back to full scan when bucket coverage is sparse |
| **Keep scan + `LIMIT` prefilter in SQL** | No extension | Not true ANN, still O(n) |
| **External index file** (e.g. hnswlib sidecar) | Full control | Two sources of truth, backup pain |

**Chosen path after implementation**: PR #73 uses a local locality-bucket shadow
index behind the existing `ann-embed` Cargo feature. This avoids platform-native
extension packaging while still bounding candidate scoring for the feature path.
`sqlite-vec` remains a possible future replacement if release packaging becomes
worth the complexity.

Reasons:

- default `rusqlite` bundled builds remain portable;
- CI can test the feature path without platform-specific binaries;
- the shadow table lives in the same SQLite DB and is rebuilt from
  `wiki_embedding`, so backup/restore remains DB-local.

## Data Model

- **Existing**: `wiki_embedding(doc_id TEXT PK, dim INT, vec BLOB, updated_at)`.
- **Feature gate PR #72**: no new schema. `EmbeddingSearchBackend` reports
  `AnnFeatureFallback` when `ann-embed` is enabled and still reads
  `wiki_embedding`.
- **Implementation PR #73**: adds `wiki_embedding_ann(doc_id, dim, bucket,
  updated_at)` and `wiki_embedding_ann_dim_bucket_doc_idx`. The bucket is a
  deterministic 16-bit locality signature derived from the vector.

## Query Flow

1. `upsert_embedding`: within the same transaction, update `wiki_embedding`
   and `wiki_embedding_ann`. `delete_embedding` removes both rows in one
   transaction. PR #71 already made source/claim blob writes atomic with
   snapshot/outbox; PR #73 preserves that boundary for ANN metadata.
2. `search_embeddings_cosine`:
   - If `ann-embed` is enabled: compute query bucket + one-bit neighbor buckets,
     read at most `clamp(limit * 64, 64, 4096)` candidates through the indexed
     `(dim, bucket, doc_id)` table, then re-rank by exact cosine.
   - If the index is empty, stale, or yields fewer candidates than requested:
     warn and fall back to the current full scan.
   - Else: current full scan.

## `rusqlite` + bundled SQLite

- Project uses `rusqlite` with `bundled`. Loading extensions may require
  `Connection::load_extension` and shipping a prebuilt extension binary per
  **platform** in release artifacts — a **compatibility and release**
  decision; document in design update before writing code.

## API Surface

- **No** change to `search_embeddings_cosine` signature; internal dispatch only.
- `SqliteRepository::embedding_search_backend()` exposes which path is active:
  `FullScan` by default, `AnnLocalityBuckets` when `ann-embed` is compiled.

## Edge Cases

- **Dimension change**: if `doc_id` row changes `dim`, index row must
  replace, not update in place with wrong size.
- **Empty table**: return `Ok([])` as today; no ANN call.
- **Corrupt / stale index**: on feature-enabled open, rebuild the shadow index
  when row counts differ. During search, warn and fall back to scan when bucket
  candidates are missing or insufficient.

## Test Strategy

- Unit: with ANN off, same golden vectors as current `embedding_cosine_ranking` test.
- With `ann-embed` feature on: run targeted tests for backend selection,
  locality search exact re-rank, fallback, index rebuild, and delete cleanup.
- Default feature set keeps the existing exact full-scan ranking test.

## Spec Sync Rules

- Any change to `upsert_embedding` contract must update
  [requirements](requirements.md) and migration section.

## References

- `crates/wiki-storage/src/lib.rs` — `search_embeddings_cosine`, `upsert_embedding`
- Roadmap J14 / J13 notes on semantic lane budget
