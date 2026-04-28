# Tasks: ANN-Backed wiki_embedding Search

## Checklist

- [x] Requirements approved
- [x] Design approved (extension choice + CI/release story)
- [x] Plan approved
- [x] Branch: `codex/embedding-ann-spike`
- [x] Spike / feature gate: `ann-embed` compiles and falls back to full scan
- [ ] Implement upsert + search dual path
- [ ] Tests: feature on/off, recall or exact match policy
- [x] Docs: operator notes for feature gate fallback
- [x] Handoff: `docs/handovers/embedding-ann-index/spike.md`
- [x] PR + CI (feature gate smoke) green
- [x] PRD / roadmap updated for spike PR scope

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| Technology spike + build matrix | Agent | Main | `crates/wiki-storage/`, `.github/workflows/ci-quick.yml` | Spec approval | Complete |
| Index DDL + migration in `open` | Agent | TBD | `wiki-storage` | Spike | Not started |
| `upsert` / `delete` + index consistency | Agent | TBD | `wiki-storage` | DDL | Not started |
| `search_embeddings_cosine` ANN path | Agent | TBD | `wiki-storage` | Index | Not started |
| Fall back + warn / metrics | Script | Main/TBD | `wiki-storage` | — | Feature fallback complete; observability pending |
| Long fixture / bench (optional) | Script | TBD | `benches/` or `tests/` | ANN path | Not started |

## Review Gates

- No regression with default feature set (CI).
- If ANN is approximate, document tolerance and re-rank strategy.

## Stop Conditions

- If extension cannot be distributed for macOS/linux CI within budget, freeze
  at “chunked scan + `LIMIT`” and revisit — update PRD with user sign-off.

## Verification

- `cargo test -p wiki-storage embedding_ann_feature_gate_reports_backend_and_falls_back_to_scan -- --nocapture`
- `cargo test -p wiki-storage --features ann-embed embedding_ann_feature_gate_reports_backend_and_falls_back_to_scan`
- `cargo test -p wiki-storage embedding_cosine_ranking -- --nocapture`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
