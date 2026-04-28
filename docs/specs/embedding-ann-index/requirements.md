# Requirements: ANN-Backed wiki_embedding Search

## Goal

- Replace the default **O(n) full table scan** in `search_embeddings_cosine` with
  a **bounded-work per query** path suitable for large `wiki_embedding` tables,
  while keeping a **fallback** for environments without the optional index.

## Context

- Current implementation loads all rows and scores in Rust (acceptable for
  small DBs, not for high-ingest or large corpora).
- C15 follow-up: architecture change (extension / ANN) is out of the hotfix PR.

## Functional Requirements

- **R1 (bounded work)**: Default production path (when feature enabled) must
  document and enforce an upper bound on work per query (e.g. index top-k
  or graph-limited search), not linear in table size.
- **R2 (correctness)**: For the same query vector and stored rows, results must
  be **deterministic** and rank by cosine (or an admitted approximation, with
  explicit tolerance, if the index is approximate).
- **R3 (opt-in)**: New behavior is behind the `ann-embed` Cargo feature and a
  runtime backend dispatch. The default build remains portable and uses the
  current full-scan fallback.
- **R4 (write path)**: `upsert_embedding` must keep the **index/secondary
  structure** consistent (same transaction as blob write, or post-commit
  rebuild job — document which).
- **R5 (observability)**: Log or counter when falling back to full scan, or
  when index is empty/missing, so operators can tell “slow path” in production.
- **R6 (compatibility)**: Existing `wiki_embedding` schema remains valid;
  new tables/indices are additive; migration script or `CREATE IF NOT EXISTS`
  in `SqliteRepository::open`.

## Non-Goals

- Re-implementing `rust-mempalace` internal vector search in this workstream
  (link from design only if shared crate later).
- Training custom embedding models or changing default dimensions.

## Inputs / Outputs

- **Input**: query `&[f32]`, `limit: usize` (as today).
- **Output**: `Vec<(String, f32)>` sorted by score descending, same as current
  public contract.

## Acceptance Criteria

- [x] Design doc lists chosen technology direction, feature gate, fallback, and
      failure modes.
- [x] `cargo test --workspace` with feature off: behavior unchanged.
- [x] With feature on: CI smoke proves the feature gate compiles and falls back
      to full scan. Sub-linear ANN proof remains implementation PR scope.
- [ ] `docs/roadmap` / PRD cross-links updated when implemented.

## Checklist

- [x] Public API contract unchanged
- [x] Documented default vs. fast path
- [x] Migrations and rollback story for the spike / feature gate
