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

- **R1 (bounded work)**: Feature-enabled production path must document and
  enforce an upper bound on scored candidates per query. Fallback full scan is
  allowed only when the additive index is empty, stale, or under-populated for
  the requested limit.
- **R2 (correctness)**: For the same query vector and stored rows, results must
  be deterministic. The bounded path may be approximate, but it must re-rank
  candidates by exact cosine and fall back to full scan when candidate coverage
  is insufficient for compatibility.
- **R3 (opt-in)**: New behavior is behind the `ann-embed` Cargo feature and a
  runtime backend dispatch. The default build remains portable and uses the
  current full-scan fallback.
- **R4 (write path)**: `upsert_embedding` / `delete_embedding` must keep the
  index table consistent in the same SQLite transaction as the blob row.
- **R5 (observability)**: Emit a warning when falling back to full scan, or
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
- [x] With feature on: tests cover locality-bucket search, exact re-rank,
      fallback, rebuild, and default full-scan compatibility.
- [x] `docs/roadmap` / PRD cross-links updated when implemented.

## Checklist

- [x] Public API contract unchanged
- [x] Documented default vs. fast path
- [x] Migrations and rollback story
