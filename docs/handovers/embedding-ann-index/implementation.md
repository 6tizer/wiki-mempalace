# Handoff: Embedding ANN Index Implementation

## Status

Complete in PR #73. GitHub quick passed; merge pending.

## Scope

PR #73 completes the C16B bounded search implementation without adding a native
SQLite extension.

## Changed

- `wiki-storage` creates `wiki_embedding_ann(doc_id, dim, bucket, updated_at)`.
- `upsert_embedding` and `delete_embedding` maintain blob + ANN metadata in one
  SQLite transaction.
- `save_snapshot_and_append_outbox_with_embeddings` keeps embedding blob and ANN
  metadata inside the existing snapshot/outbox transaction.
- Feature-enabled `search_embeddings_cosine` uses
  `EmbeddingSearchBackend::AnnLocalityBuckets`.
- The ANN path probes the query locality bucket plus one-bit neighbors, scores at
  most `clamp(limit * 64, 64, 4096)` candidates, then re-ranks by exact cosine.
- If buckets are empty or under-populated, the query warns and falls back to the
  full scan for compatibility.
- CI quick runs the feature-enabled storage embedding test slice.

## Verification

- `cargo test -p wiki-storage embedding_ -- --nocapture`
- `cargo test -p wiki-storage --features ann-embed embedding_ -- --nocapture`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test --workspace`
- `cargo test -p wiki-storage --features ann-embed embedding_`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Next

After PR #73 merges, continue roadmap item 15: `Contradiction Scan Scaling`.
