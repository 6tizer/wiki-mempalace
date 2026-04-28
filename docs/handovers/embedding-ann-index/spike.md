# Handoff: Embedding ANN Index Spike / Feature Gate

## Status

Complete in PR #72. GitHub quick passed; merge pending.

## Scope

PR #72 locks the C16B feature-gate shape without adding a native ANN extension.

## Decision

- Preferred implementation target remains `sqlite-vec`.
- Default builds keep exact `wiki_embedding` full scan.
- New Cargo feature: `ann-embed`.
- With `ann-embed` enabled today, `EmbeddingSearchBackend::AnnFeatureFallback` is reported and the exact full scan still runs.
- Real ANN DDL/loading/search is deferred to the next implementation PR.

## Changed

- `wiki-storage` has `ann-embed` feature.
- `SqliteRepository::embedding_search_backend()` reports `FullScan` or `AnnFeatureFallback`.
- `search_embeddings_cosine` dispatches through the backend enum.
- CI quick adds a small `ann-embed` feature-gate smoke.

## Verification

- `cargo test -p wiki-storage embedding_ann_feature_gate_reports_backend_and_falls_back_to_scan -- --nocapture`
- `cargo test -p wiki-storage --features ann-embed embedding_ann_feature_gate_reports_backend_and_falls_back_to_scan -- --nocapture`
- `cargo test -p wiki-storage embedding_cosine_ranking -- --nocapture`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test --workspace`
- `cargo test -p wiki-storage --features ann-embed embedding_ann_feature_gate_reports_backend_and_falls_back_to_scan`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Next

PR #73 should implement the real bounded vector search path behind the existing `ann-embed` gate, preserving full-scan fallback.
