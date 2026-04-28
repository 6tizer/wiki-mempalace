# Tasks: ANN-Backed wiki_embedding Search

## Checklist

- [x] Requirements approved
- [x] Design approved (extension choice + CI/release story)
- [x] Plan approved
- [x] Branch: `codex/embedding-ann-spike`
- [x] Branch: `codex/embedding-ann-implementation`
- [x] Spike / feature gate: `ann-embed` compiles and falls back to full scan
- [x] Implement upsert + search dual path
- [x] Tests: feature on/off, fallback, rebuild, exact candidate re-rank
- [x] Docs: operator notes for feature gate fallback
- [x] Handoff: `docs/handovers/embedding-ann-index/spike.md`
- [x] Handoff: `docs/handovers/embedding-ann-index/implementation.md`
- [x] PR + CI (feature gate smoke) green
- [x] PR + CI (implementation) green
- [x] PRD / roadmap updated for implementation scope

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| Technology spike + build matrix | Agent | Main | `crates/wiki-storage/`, `.github/workflows/ci-quick.yml` | Spec approval | Complete |
| Index DDL + migration in `open` | Agent | Main | `wiki-storage` | Spike | Complete |
| `upsert` / `delete` + index consistency | Agent | Main | `wiki-storage` | DDL | Complete |
| `search_embeddings_cosine` ANN path | Agent | Main | `wiki-storage` | Index | Complete |
| Fall back + warn / metrics | Script | Main | `wiki-storage` | — | Warning fallback complete; metrics deferred |
| Long fixture / bench (optional) | Script | TBD | `benches/` or `tests/` | ANN path | Deferred |

## Review Gates

- No regression with default feature set (CI).
- If ANN is approximate, document tolerance and re-rank strategy.

## Stop Conditions

- If extension cannot be distributed for macOS/linux CI within budget, freeze
  at “chunked scan + `LIMIT`” and revisit — update PRD with user sign-off.

## Verification

- `cargo test -p wiki-storage embedding_ann_feature_gate_reports_backend_and_falls_back_to_scan -- --nocapture`
- `cargo test -p wiki-storage --features ann-embed embedding_ann_feature_gate_reports_backend_and_falls_back_to_scan`
- `cargo test -p wiki-storage --features ann-embed embedding_ann_locality_search_reranks_bounded_candidates`
- `cargo test -p wiki-storage --features ann-embed embedding_ann_index_is_maintained_and_rebuilt`
- `cargo test -p wiki-storage --features ann-embed embedding_ann_falls_back_to_scan_when_index_empty`
- `cargo test -p wiki-storage --features ann-embed embedding_`
- `cargo test -p wiki-storage embedding_cosine_ranking -- --nocapture`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
