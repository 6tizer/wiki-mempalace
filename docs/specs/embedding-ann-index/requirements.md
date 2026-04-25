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
- **R3 (opt-in)**: New behavior is behind a **Cargo feature** and/or
  `PRAGMA`/runtime flag so that default `rusqlite` builds remain portable
  (bundled links may need extra linking story — see design).
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

- [ ] Design doc lists chosen technology (e.g. `sqlite-vec`, `sqlite-vss`, or
      acceptable alternative) and failure modes.
- [ ] `cargo test --workspace` with feature off: behavior unchanged.
- [ ] With feature on: benchmark or test proves sub-linear growth (e.g. large
      fixture) or documented cap.
- [ ] `docs/roadmap` / PRD cross-links updated when implemented.

## Checklist

- [ ] Public API contract unchanged
- [ ] Documented default vs. fast path
- [ ] Migrations and rollback story
